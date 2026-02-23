use crate::adapters::sqlite;
use crate::adapters::sqlite::{accounts, onboarding_tasks, DbPool};
use crate::adapters::imap::connection;
use crate::services::sync::{helpers, tasks};
use crate::error::EddieError;

use crate::services::logger;

/// Run one unit of work. Returns true if work was done (onboarding in progress).
pub async fn tick(
    app: &tauri::AppHandle,
    pool: &DbPool,
) -> Result<bool, EddieError> {
    logger::debug("Engine tick");

    // Step 1: Always fetch latest messages for all onboarded accounts.
    // This runs even during onboarding so new mail keeps arriving.
    let _ = tasks::run_incremental_sync_all(app, pool).await;
    let _ = tasks::run_flag_resync_all(app, pool).await;

    // Step 2: Find an account that needs onboarding
    let account_id = match accounts::find_account_for_onboarding(pool)? {
        Some(id) => id,
        None => return Ok(false),
    };

    // Step 3: Get tasks for this account, seed if missing
    let tasks = onboarding_tasks::get_tasks(pool, &account_id)?;
    if tasks.is_empty() {
        onboarding_tasks::seed_tasks(pool, &account_id)?;
        return Ok(true);
    }

    // Step 4: Find first non-done task
    let task = match tasks.iter().find(|t| t.status != "done") {
        Some(t) => t,
        None => return Ok(false), // all tasks done, incremental sync already ran above
    };

    // Step 5: Run it
    match task.name.as_str() {
        "trust_network" => tasks::run_trust_network(app, pool, &account_id, &task).await?,
        "historical_fetch" => tasks::run_historical_fetch(app, pool, &account_id, &task).await?,
        "connection_history" => {
            tasks::run_connection_history(app, pool, &account_id, &task).await?;
        }
        _ => {
            logger::warn(&format!("Unknown task: {}", task.name));
            onboarding_tasks::mark_task_done(pool, &account_id, &task.name)?;
        }
    }

    Ok(true)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) async fn connect_account(
    pool: &DbPool,
    account_id: &str,
) -> Result<(accounts::Credentials, Vec<String>, connection::ImapConnection), EddieError> {
    let creds = sqlite::accounts::get_credentials(pool, account_id)?
        .ok_or(EddieError::AccountNotFound(account_id.to_string()))?;

    let conn = connection::connect_with_tls(&creds.host, creds.port, creds.tls, &creds.email, &creds.password).await?;

    let self_emails = sqlite::entities::get_self_emails(pool, account_id)?;

    Ok((creds, self_emails, conn))
}

pub fn process_changes(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
) -> Result<(), EddieError> {
    // Update trust network from new sent messages (before classify sets processed_at)
    let start = std::time::Instant::now();
    let extracted = helpers::entity_extraction::extract_entities_from_new_messages(pool, account_id)?;
    if extracted > 0 {
        logger::debug(&format!("Extracted {} connections in {}", extracted, logger::fmt_ms(start.elapsed())));
    }

    helpers::status_emit::emit_status(app, "classifying", "Identifying Points & Circles...");
    let start = std::time::Instant::now();
    let classified = helpers::message_classification::classify_messages(pool, account_id)?;
    logger::debug(&format!("Classified {} messages in {}", classified, logger::fmt_ms(start.elapsed())));

    helpers::status_emit::emit_status(app, "distilling", "Classifying Lines with AI...");
    let start = std::time::Instant::now();
    let distilled = helpers::message_distillation::distill_messages(pool, account_id)?;
    logger::debug(&format!("Distilled {} messages in {}", distilled, logger::fmt_ms(start.elapsed())));

    helpers::status_emit::emit_status(app, "rebuilding", "Organizing conversations...");
    let start = std::time::Instant::now();
    let conv_count = sqlite::conversations::rebuild_conversations(pool, account_id)?;
    logger::debug(&format!("Rebuilt {} conversations in {}", conv_count, logger::fmt_ms(start.elapsed())));

    helpers::status_emit::emit_conversations_updated(app, account_id, conv_count);
    Ok(())
}
