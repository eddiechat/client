use crate::adapters::sqlite;
use crate::adapters::sqlite::DbPool;
use crate::adapters::imap::folders;
use crate::services::sync::worker;
use crate::error::EddieError;

use tracing::{info, error};
use async_imap::types::Fetch;
use futures::TryStreamExt;
use std::collections::HashMap;

const BATCH_SIZE: usize = 500;

/// Run flag resync for all onboarded accounts.
pub async fn run_flag_resync_all(
    app: &tauri::AppHandle,
    pool: &DbPool,
) -> Result<(), EddieError> {
    let account_ids = sqlite::accounts::list_onboarded_account_ids(pool)?;
    for account_id in &account_ids {
        if let Err(e) = run_flag_resync(app, pool, account_id).await {
            error!("Flag resync error for {}: {}", account_id, e);
        }
    }
    Ok(())
}

/// Fetch current flags (and Gmail labels) from IMAP for all locally-cached messages
/// and update any that changed. Rebuilds conversations once at the end if anything changed.
pub async fn run_flag_resync(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
) -> Result<(), EddieError> {
    let (_creds, _self_emails, mut conn) = worker::connect_account(pool, account_id).await?;
    let is_gmail = conn.has_gmail_ext;

    let folder_list = folders::list_folders(&mut conn.session).await?;
    let sync_folders = folders::folders_to_sync(&folder_list, is_gmail);

    let mut any_changed = false;

    for folder_info in &sync_folders {
        let state = match sqlite::folder_sync::get_folder(pool, account_id, &folder_info.name)? {
            Some(s) => s,
            None => continue,
        };

        if state.highest_uid == 0 {
            continue; // never synced
        }

        let folder_start = std::time::Instant::now();

        conn.select_folder(&folder_info.name).await?;

        let mut total_changed: usize = 0;

        if is_gmail {
            // Gmail: resync both FLAGS and X-GM-LABELS
            let local = sqlite::messages::get_uids_flags_and_labels_for_folder(
                pool, account_id, &folder_info.name
            )?;
            if local.is_empty() {
                continue;
            }

            let total_messages = local.len();
            let local_lookup: HashMap<u32, (&str, &str)> = local.iter()
                .map(|(uid, flags, labels)| (*uid, (flags.as_str(), labels.as_str())))
                .collect();

            for batch in local.chunks(BATCH_SIZE) {
                let uid_list: String = batch.iter()
                    .map(|(uid, _, _)| uid.to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                let fetches: Vec<Fetch> = conn.session
                    .uid_fetch(&uid_list, "(UID FLAGS X-GM-LABELS)")
                    .await
                    .map_err(|e| EddieError::Backend(format!("FETCH flags failed: {}", e)))?
                    .try_collect()
                    .await
                    .map_err(|e| EddieError::Backend(format!("Collect flags failed: {}", e)))?;

                let mut updates: Vec<(u32, String, String)> = Vec::new();

                for fetch in &fetches {
                    if let Some(uid) = fetch.uid {
                        let mut new_flags: Vec<String> = fetch.flags()
                            .map(|f| format!("{:?}", f))
                            .collect();
                        new_flags.sort();
                        let new_flags_json = serde_json::to_string(&new_flags)
                            .unwrap_or_else(|_| "[]".to_string());

                        let mut new_labels: Vec<String> = fetch.gmail_labels()
                            .map(|labels| labels.iter()
                                .map(|l| l.trim_start_matches('\\').to_string())
                                .collect())
                            .unwrap_or_default();
                        new_labels.sort();
                        let new_labels_json = serde_json::to_string(&new_labels)
                            .unwrap_or_else(|_| "[]".to_string());

                        if let Some(&(old_flags, old_labels)) = local_lookup.get(&uid) {
                            if old_flags != new_flags_json || old_labels != new_labels_json {
                                updates.push((uid, new_flags_json, new_labels_json));
                            }
                        }
                    }
                }

                if !updates.is_empty() {
                    total_changed += updates.len();
                    sqlite::messages::update_flags_and_labels_batch(
                        pool, account_id, &folder_info.name, &updates
                    )?;
                }
            }

            if total_changed > 0 {
                any_changed = true;
                info!(
                    "Flag resync for {}: {} changed out of {} messages in {:?}",
                    folder_info.name, total_changed, total_messages, folder_start.elapsed()
                );
            }
        } else {
            // Non-Gmail: resync FLAGS only
            let local = sqlite::messages::get_uids_and_flags_for_folder(
                pool, account_id, &folder_info.name
            )?;
            if local.is_empty() {
                continue;
            }

            let total_messages = local.len();
            let local_flags: HashMap<u32, &str> = local.iter()
                .map(|(uid, flags)| (*uid, flags.as_str()))
                .collect();

            for batch in local.chunks(BATCH_SIZE) {
                let uid_list: String = batch.iter()
                    .map(|(uid, _)| uid.to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                let fetches: Vec<Fetch> = conn.session
                    .uid_fetch(&uid_list, "(UID FLAGS)")
                    .await
                    .map_err(|e| EddieError::Backend(format!("FETCH flags failed: {}", e)))?
                    .try_collect()
                    .await
                    .map_err(|e| EddieError::Backend(format!("Collect flags failed: {}", e)))?;

                let mut updates: Vec<(u32, String)> = Vec::new();

                for fetch in &fetches {
                    if let Some(uid) = fetch.uid {
                        let new_flags: Vec<String> = fetch.flags()
                            .map(|f| format!("{:?}", f))
                            .collect();
                        let new_flags_json = serde_json::to_string(&new_flags)
                            .unwrap_or_else(|_| "[]".to_string());

                        if let Some(&old_flags) = local_flags.get(&uid) {
                            if old_flags != new_flags_json {
                                updates.push((uid, new_flags_json));
                            }
                        }
                    }
                }

                if !updates.is_empty() {
                    total_changed += updates.len();
                    sqlite::messages::update_flags_batch(
                        pool, account_id, &folder_info.name, &updates
                    )?;
                }
            }

            if total_changed > 0 {
                any_changed = true;
                info!(
                    "Flag resync for {}: {} changed out of {} messages in {:?}",
                    folder_info.name, total_changed, total_messages, folder_start.elapsed()
                );
            }
        }
    }

    // Rebuild conversations once at the end if anything changed
    if any_changed {
        let conv_count = sqlite::conversations::rebuild_conversations(pool, account_id)?;
        crate::services::sync::helpers::status_emit::emit_conversations_updated(app, account_id, conv_count);
    }

    Ok(())
}
