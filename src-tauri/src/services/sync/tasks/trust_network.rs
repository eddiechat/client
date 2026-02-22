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

    // Discover Sent folder
    let folder_list = folders::list_folders(&mut conn.session).await?;
    let sent_folder = folders::find_folder_by_attribute(&folder_list, "Sent")
        .ok_or(EddieError::Backend("No Sent folder found".into()))?;

    // First tick: insert user + alias entities
    if cursor_uid == 0 {
        seed_self_entities(pool, account_id, &creds.email, &self_emails)?;
    }

    let mailbox = conn.select_folder(&sent_folder).await?;
    let server_count = mailbox.exists;

    let above_uid = if cursor_uid > 0 { Some(cursor_uid) } else { None };

    let start = std::time::Instant::now();
    let (recipient_counts, max_uid, remaining) =
        fetch_sent_recipients_batch(&mut conn, 500, above_uid).await?;
    logger::debug(&format!("fetch_sent_recipients_batch took: {}", logger::fmt_ms(start.elapsed())));

    let scanned = server_count as usize - remaining;
    helpers::status_emit::emit_status(
        app, "trust_network",
        &format!("{}/{} from Sent scanned", scanned, server_count),
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
