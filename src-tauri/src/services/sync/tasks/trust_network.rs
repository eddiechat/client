use crate::adapters::sqlite::{onboarding_tasks, DbPool};
use crate::adapters::sqlite::entities::{upsert_entities, NewEntity};

use crate::adapters::imap::folders;
use crate::adapters::imap::sent_scan::fetch_sent_recipients_batch;

use crate::services::sync::{helpers, worker};
use crate::services::sync::helpers::email_normalization::normalize_email;
use crate::error::EddieError;

use crate::services::logger;

/// Onboarding phase 2: Build trust network from sent folder.
///
/// Processes one batch of 500 sent messages per tick:
/// - First tick (no cursor): inserts user + alias entities, then fetches the first batch
/// - Subsequent ticks: continues from the persisted UID cursor
/// - Final tick (no more UIDs): runs process_changes and marks the task done
///
/// Sent folder discovery uses a 3-tier strategy:
/// 1. IMAP attribute match (\Sent from RFC 6154)
/// 2. Name-based fallback (known Sent folder names across languages)
/// 3. Scan all syncable folders for messages FROM the user's email
pub async fn run_trust_network(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
    task: &onboarding_tasks::Task,
) -> Result<(), EddieError> {
    let (creds, self_emails, mut conn) = worker::connect_account(pool, account_id).await?;

    // Parse cursor as the last processed UID (0 = first tick)
    let cursor_uid: u32 = task.cursor
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // First tick: insert user + alias entities
    if cursor_uid == 0 {
        seed_self_entities(pool, account_id, &creds.email, &self_emails)?;
    }

    // Discover Sent folder (attribute match → name fallback → FROM-user scan)
    let folder_list = folders::list_folders(&mut conn.session).await?;
    let (scan_folder, from_filter): (String, Option<String>) = match folders::find_sent_folder(&folder_list) {
        Some(f) => {
            // Tier 1/2 matched: scan the Sent folder directly (all messages are from the user)
            (f, None)
        }
        None => {
            // Tier 3: no Sent folder found — scan syncable folders for FROM user messages
            let sync_folders = folders::folders_to_sync(&folder_list, conn.has_gmail_ext);
            if sync_folders.is_empty() {
                logger::info("No syncable folders found, skipping trust network task");
                onboarding_tasks::mark_task_done(pool, account_id, &task.name)?;
                return Ok(());
            }

            // Scan each syncable folder for FROM-user messages in one pass
            logger::info(&format!(
                "No Sent folder found, scanning {} folders for FROM-user messages",
                sync_folders.len()
            ));
            return scan_folders_for_sent(
                app, pool, account_id, task, &creds.email, &self_emails,
                &mut conn, &sync_folders,
            ).await;
        }
    };

    let mailbox = conn.select_folder(&scan_folder).await?;
    let server_count = mailbox.exists;
    logger::info(&format!("{} messages found in {}", server_count, scan_folder));

    let above_uid = if cursor_uid > 0 { Some(cursor_uid) } else { None };

    let start = std::time::Instant::now();
    let (recipient_counts, max_uid, remaining) =
        fetch_sent_recipients_batch(&mut conn, 500, above_uid, from_filter.as_deref()).await?;
    logger::debug(&format!("fetch_sent_recipients_batch took: {}", logger::fmt_ms(start.elapsed())));

    let scanned = server_count as usize - remaining;
    helpers::status_emit::emit_status(
        app, "trust_network",
        &format!("{}/{} from {} scanned", scanned, server_count, scan_folder),
    );

    match max_uid {
        Some(new_cursor) => {
            // Build entity records for this batch
            let self_normalized: Vec<String> = std::iter::once(normalize_email(&creds.email))
                .chain(self_emails.iter().map(|a| normalize_email(a)))
                .collect();

            let now = chrono::Utc::now().timestamp_millis();
            let entities: Vec<NewEntity> = recipient_counts
                .iter()
                .filter(|(email, _)| !self_normalized.contains(&normalize_email(email)))
                .map(|(email, count)| NewEntity {
                    account_id: account_id.to_string(),
                    email: normalize_email(email),
                    display_name: None,
                    trust_level: "connection".to_string(),
                    source: Some("sent_scan".to_string()),
                    first_seen: now,
                    last_seen: None,
                    sent_count: Some(*count as i32),
                    metadata: None,
                })
                .collect();

            if !entities.is_empty() {
                upsert_entities(pool, &entities)?;
            }

            // Persist cursor for next tick
            onboarding_tasks::update_cursor(pool, account_id, &task.name, &new_cursor.to_string())?;
            logger::debug(&format!("Trust network: batch done, cursor at UID {}", new_cursor));
        }
        None => {
            // All UIDs processed — finalize
            logger::debug("Trust network: all sent messages scanned");
            worker::process_changes(app, pool, account_id)?;
            onboarding_tasks::mark_task_done(pool, account_id, &task.name)?;
        }
    }

    Ok(())
}

/// Fallback: scan all syncable folders for messages FROM the user's email.
/// Runs as a single-tick operation (no cursor) since the FROM filter typically
/// yields far fewer messages than scanning the entire Sent folder.
async fn scan_folders_for_sent(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
    task: &onboarding_tasks::Task,
    user_email: &str,
    self_emails: &[String],
    conn: &mut crate::adapters::imap::connection::ImapConnection,
    sync_folders: &[&folders::FolderInfo],
) -> Result<(), EddieError> {
    let self_normalized: Vec<String> = std::iter::once(normalize_email(user_email))
        .chain(self_emails.iter().map(|a| normalize_email(a)))
        .collect();

    let mut all_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (i, folder_info) in sync_folders.iter().enumerate() {
        helpers::status_emit::emit_status(
            app, "trust_network",
            &format!("Scanning {} for sent messages ({}/{})", folder_info.name, i + 1, sync_folders.len()),
        );

        if let Err(e) = conn.select_folder(&folder_info.name).await {
            logger::warn(&format!("Could not select {}: {}", folder_info.name, e));
            continue;
        }

        // Fetch all FROM-user messages in this folder (no cursor, single pass)
        let (counts, _, _) = fetch_sent_recipients_batch(conn, 5000, None, Some(user_email)).await?;

        for (email, count) in counts {
            *all_counts.entry(email).or_insert(0) += count;
        }
    }

    let now = chrono::Utc::now().timestamp_millis();
    let entities: Vec<NewEntity> = all_counts
        .iter()
        .filter(|(email, _)| !self_normalized.contains(&normalize_email(email)))
        .map(|(email, count)| NewEntity {
            account_id: account_id.to_string(),
            email: normalize_email(email),
            display_name: None,
            trust_level: "connection".to_string(),
            source: Some("sent_scan".to_string()),
            first_seen: now,
            last_seen: None,
            sent_count: Some(*count as i32),
            metadata: None,
        })
        .collect();

    if !entities.is_empty() {
        upsert_entities(pool, &entities)?;
        logger::info(&format!("Trust network fallback: found {} connections from FROM-user scan", entities.len()));
    } else {
        logger::info("Trust network fallback: no sent messages found in syncable folders");
    }

    worker::process_changes(app, pool, account_id)?;
    onboarding_tasks::mark_task_done(pool, account_id, &task.name)?;
    Ok(())
}

fn seed_self_entities(
    pool: &DbPool,
    account_id: &str,
    user_email: &str,
    aliases: &[String],
) -> Result<(), EddieError> {
    let now = chrono::Utc::now().timestamp_millis();

    let mut entities = vec![NewEntity {
        account_id: account_id.to_string(),
        email: normalize_email(user_email),
        display_name: None,
        trust_level: "user".to_string(),
        source: Some("self".to_string()),
        first_seen: now,
        last_seen: Some(now),
        sent_count: None,
        metadata: None,
    }];

    for alias in aliases {
        entities.push(NewEntity {
            account_id: account_id.to_string(),
            email: normalize_email(alias),
            display_name: None,
            trust_level: "alias".to_string(),
            source: Some("self".to_string()),
            first_seen: now,
            last_seen: Some(now),
            sent_count: None,
            metadata: None,
        });
    }

    upsert_entities(pool, &entities)?;
    logger::debug(&format!("Seeded {} self entities", entities.len()));
    Ok(())
}
