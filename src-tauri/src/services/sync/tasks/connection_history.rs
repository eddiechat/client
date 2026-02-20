use crate::adapters::sqlite;
use crate::adapters::sqlite::{onboarding_tasks, DbPool};
use crate::adapters::imap::{envelopes, folders, historical};
use crate::services::sync::{helpers, worker};
use crate::error::EddieError;

use tracing::warn;
use std::collections::HashMap;
use async_imap::types::Fetch;
use futures::TryStreamExt;

/// Onboarding phase 4: Fetch full history for conversations of type "connections"
pub async fn run_connection_history(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
    task: &onboarding_tasks::Task,
) -> Result<(), EddieError> {
    // Step 1: Get connection emails from connections conversations
    let connection_emails = sqlite::conversations::get_connection_emails(pool, account_id)?;

    if connection_emails.is_empty() {
        onboarding_tasks::mark_task_done(pool, account_id, &task.name)?;
        return Ok(());
    }

    let (_creds, self_emails, mut conn) = worker::connect_account(pool, account_id).await?;
    let folder_list = folders::list_folders(&mut conn.session).await?;
    let sync_folders = folders::folders_to_sync(&folder_list, conn.has_gmail_ext);

    for folder_info in &sync_folders {
        conn.select_folder(&folder_info.name).await?;

        // Get existing UIDs for this folder so we can skip them
        let existing_uids = sqlite::messages::get_uids_for_folder(
            pool, account_id, &folder_info.name
        )?;

        for (conn_idx, email) in connection_emails.iter().enumerate() {
            // Search for messages from or to this connection (no date limit)
            let search_query = format!(
                "OR FROM \"{}\" TO \"{}\"",
                email, email
            );

            let uid_set = conn.session
                .uid_search(&search_query)
                .await
                .map_err(|e| EddieError::Backend(format!("SEARCH failed: {}", e)))?;

            let new_uids: Vec<u32> = uid_set.into_iter()
                .filter(|uid| !existing_uids.contains(uid))
                .collect();

            if new_uids.is_empty() {
                continue;
            }

            println!(
                "Found {} old messages with {} in {}",
                new_uids.len(), email, folder_info.name
            );

            // Fetch in batches of 200
            let total = new_uids.len();
            let mut fetched = 0usize;
            for chunk in new_uids.chunks(200) {
                fetched += chunk.len();
                helpers::status_emit::emit_status(app, "connection_history",
                    &format!("Expanding thread {} with {}/{}",
                        conn_idx + 1, fetched, total));
                let uid_list: String = chunk.iter()
                    .map(|u| u.to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                // Fetch envelopes
                let fetch_query = if conn.has_gmail_ext {
                    "(UID FLAGS ENVELOPE BODYSTRUCTURE X-GM-LABELS)"
                } else {
                    "(UID FLAGS ENVELOPE BODYSTRUCTURE)"
                };

                let fetches: Vec<Fetch> = conn.session
                    .uid_fetch(&uid_list, fetch_query)
                    .await
                    .map_err(|e| EddieError::Backend(format!("FETCH failed: {}", e)))?
                    .try_collect()
                    .await
                    .map_err(|e| EddieError::Backend(format!("Collect failed: {}", e)))?;

                let mut envelopes: Vec<envelopes::Envelope> = Vec::new();
                let mut text_parts: Vec<(u32, Vec<u32>, bool, String)> = Vec::new();

                for fetch in &fetches {
                    if let Some(env) = envelopes::parse_envelope(fetch) {
                        envelopes.push(env);
                    }
                    if let (Some(uid), Some(bs)) = (fetch.uid, fetch.bodystructure()) {
                        if let Some((part, encoding)) = historical::find_mime_part(bs, &[], "plain") {
                            text_parts.push((uid, part, false, historical::encoding_to_string(encoding)));
                        } else if let Some((part, encoding)) = historical::find_mime_part(bs, &[], "html") {
                            text_parts.push((uid, part, true, historical::encoding_to_string(encoding)));
                        }
                    }
                }

                // Fetch references
                let refs_fetches: Vec<Fetch> = conn.session
                    .uid_fetch(&uid_list, "(UID BODY.PEEK[HEADER.FIELDS (References)])")
                    .await
                    .map_err(|e| EddieError::Backend(format!("FETCH refs failed: {}", e)))?
                    .try_collect()
                    .await
                    .map_err(|e| EddieError::Backend(format!("Collect refs failed: {}", e)))?;

                for fetch in &refs_fetches {
                    if let Some(uid) = fetch.uid {
                        let refs = envelopes::parse_references_value(
                            &String::from_utf8_lossy(fetch.header().unwrap_or(&[]))
                        );
                        if let Some(env) = envelopes.iter_mut().find(|e| e.uid == uid) {
                            env.references = refs;
                        }
                    }
                }

                // Fetch bodies
                let mut bodies: Vec<(u32, String, bool)> = Vec::new();
                let mut uid_is_html: HashMap<u32, bool> = HashMap::new();
                let mut uid_encoding: HashMap<u32, String> = HashMap::new();

                if !text_parts.is_empty() {
                    let mut by_part: HashMap<Vec<u32>, Vec<u32>> = HashMap::new();
                    for (uid, part, is_html, encoding) in &text_parts {
                        by_part.entry(part.clone()).or_default().push(*uid);
                        uid_is_html.insert(*uid, *is_html);
                        uid_encoding.insert(*uid, encoding.clone());
                    }

                    for (part, part_uids) in &by_part {
                        let part_uid_list: String = part_uids.iter()
                            .map(|u| u.to_string())
                            .collect::<Vec<_>>()
                            .join(",");

                        let body_query = format!("(UID BODY.PEEK[{}])", historical::part_to_string(part));

                        let body_fetches: Vec<Fetch> = conn.session
                            .uid_fetch(&part_uid_list, &body_query)
                            .await
                            .map_err(|e| EddieError::Backend(format!("FETCH body failed: {}", e)))?
                            .try_collect()
                            .await
                            .map_err(|e| EddieError::Backend(format!("Collect body failed: {}", e)))?;

                        let path = historical::part_to_section_path(part);

                        for fetch in &body_fetches {
                            if let Some(uid) = fetch.uid {
                                if let Some(section_data) = fetch.section(&path) {
                                    let encoding = uid_encoding.get(&uid).cloned().unwrap_or_default();
                                    let decoded = historical::decode_body(section_data, &encoding)
                                        .map_err(|e| EddieError::Backend(format!("Decode body failed: {}", e)))?;
                                    let is_html = uid_is_html.get(&uid).copied().unwrap_or(false);
                                    bodies.push((uid, decoded, is_html));
                                }
                            }
                        }
                    }
                }

                // Insert
                let messages = helpers::message_builder::prepare_messages(
                    account_id, &folder_info.name, &envelopes, &self_emails,
                );
                sqlite::messages::insert_messages(pool, &messages)?;

                for (uid, text, is_html) in &bodies {
                    let clean_text = if *is_html {
                        html2text::from_read(text.as_bytes(), 80)
                            .unwrap_or_else(|_| text.clone())
                    } else {
                        text.clone()
                    };
                    if let Err(e) = sqlite::messages::update_body_by_uid(
                        pool, account_id, *uid, &clean_text
                    ) {
                        warn!("Failed to store body for UID {}: {}", uid, e);
                    }
                }

                worker::process_changes(app, pool, account_id)?;
            }
        }
    }

    onboarding_tasks::mark_task_done(pool, account_id, &task.name)?;
    Ok(())
}
