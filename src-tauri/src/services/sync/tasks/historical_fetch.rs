use crate::adapters::sqlite;
use crate::adapters::sqlite::{onboarding_tasks, DbPool};
use crate::adapters::imap::{folders, historical};
use crate::services::sync::{helpers, worker};
use crate::error::EddieError;

use crate::services::logger;

/// Onboarding phase 3: Fetch 12 months with text body
pub async fn run_historical_fetch(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
    task: &onboarding_tasks::Task,
) -> Result<(), EddieError> {
    // Discover and seed folders first
    let (_creds, self_emails, mut conn) = worker::connect_account(pool, account_id).await?;
    let folder_list = folders::list_folders(&mut conn.session).await?;
    let sync_folders = folders::folders_to_sync(&folder_list, conn.has_gmail_ext);

    for folder in &sync_folders {
        sqlite::folder_sync::ensure_folder(pool, account_id, &folder.name)?;
    }

    // Now check for pending work
    let next_folder = sqlite::folder_sync::next_pending_folder(pool, account_id)?;

    let folder = match next_folder {
        Some(f) => f,
        None => {
            logger::debug("Historical fetch: all folders done");
            onboarding_tasks::mark_task_done(pool, account_id, &task.name)?;
            return Ok(());
        }
    };

    logger::debug(&format!("Historical fetch: starting {}", folder.name));

    let since = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(365))
        .ok_or_else(|| EddieError::Config("Date arithmetic overflow".into()))?
        .format("%d-%b-%Y")
        .to_string();

    let local_count = sqlite::messages::get_uids_for_folder(pool, account_id, &folder.name)?
        .len();

    let mailbox = conn.select_folder(&folder.name).await?;
    let server_count = mailbox.exists;

    helpers::status_emit::emit_status(app, "historical_fetch",
        &format!("{}/{} from {} ingested", local_count, server_count, folder.name));

    let below_uid = if folder.lowest_uid > 0 {
        Some(folder.lowest_uid)
    } else {
        None
    };

    let fetch_start = std::time::Instant::now();
    let total = historical::fetch_historical(
        &mut conn,
        &folder.name,
        &since,
        200,
        Some(1),
        below_uid,
        |envelopes, bodies| -> Result<(), String> {
            let messages = helpers::message_builder::prepare_messages(
                account_id, &folder.name, &envelopes, &self_emails,
            );
            sqlite::messages::insert_messages(pool, &messages)
                .map_err(|e| e.to_string())?;

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
                    logger::warn(&format!("Failed to store body for UID {}: {}", uid, e));
                }
            }

            if let Some(min_uid) = envelopes.iter().map(|e| e.uid).min() {
                sqlite::folder_sync::update_lowest_uid(
                    pool, account_id, &folder.name, min_uid
                ).map_err(|e| e.to_string())?;
            }
            if let Some(max_uid) = envelopes.iter().map(|e| e.uid).max() {
                sqlite::folder_sync::update_lowest_uid(pool, account_id, &folder.name, max_uid)
                    .map_err(|e| e.to_string())?;
                sqlite::folder_sync::update_highest_uid(pool, account_id, &folder.name, max_uid)
                    .map_err(|e| e.to_string())?;
            }

            worker::process_changes(app, pool, account_id)
                .map_err(|e| e.to_string())?;
            Ok(())
        },
    ).await?;

    logger::debug(&format!(
        "Historical fetch: {} fetched {} messages in {}",
        folder.name, total, logger::fmt_ms(fetch_start.elapsed())
    ));

    // Update last_sync so round-robin picks another folder next
    sqlite::folder_sync::set_status(pool, account_id, &folder.name, "in_progress")?;

    if total == 0 {
        // No more UIDs to process — folder is done
        logger::debug(&format!("Historical fetch: {} complete", folder.name));
        sqlite::folder_sync::set_status(pool, account_id, &folder.name, "done")?;
    }

    // Don't mark task done — next tick checks for more folders
    Ok(())
}
