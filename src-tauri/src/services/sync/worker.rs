use crate::adapters::sqlite;
use crate::adapters::sqlite::{accounts, onboarding_tasks, DbPool};
use crate::adapters::imap::connection;
use crate::services::sync::helpers;
use crate::services::sync::helpers::message_classification::ClassifierState;
use crate::services::sync::tasks;
use crate::error::EddieError;
use crate::services::logger;
use crate::SharedClassifier;
use std::sync::Arc;
use tauri::Manager;

/// Ensure the ONNX model is downloaded and the classifier is loaded.
/// Returns the classifier, loading it on first call.
async fn ensure_classifier(
    app: &tauri::AppHandle,
    shared: &SharedClassifier,
) -> Result<Arc<ClassifierState>, EddieError> {
    // Fast path: already loaded
    {
        let guard = shared.read().await;
        if let Some(ref c) = *guard {
            return Ok(c.clone());
        }
    }

    // Slow path: download model if needed, then load
    let model_path = helpers::model_download::ensure_model(app)
        .await
        .map_err(|e| EddieError::Backend(format!("Model download failed: {}", e)))?;

    // Tokenizer is still bundled as a resource
    let tokenizer_path = {
        let resource_dir = app.path().resource_dir()
            .expect("Failed to resolve resource directory");
        let bundled = resource_dir.join("resources/tokenizer.json");
        if bundled.exists() {
            bundled
        } else {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("tokenizer.json")
        }
    };

    let classifier = Arc::new(
        ClassifierState::load(&model_path, &tokenizer_path)
            .map_err(|e| EddieError::Backend(format!("Failed to load classifier: {}", e)))?,
    );
    logger::info("ONNX classifier loaded");

    let mut guard = shared.write().await;
    *guard = Some(classifier.clone());
    Ok(classifier)
}

/// Run one unit of work. Returns true if work was done (onboarding or skill classification).
pub async fn tick(
    app: &tauri::AppHandle,
    pool: &DbPool,
    classifier: &SharedClassifier,
) -> Result<bool, EddieError> {
    logger::debug("Engine tick");

    // Ensure model is downloaded and classifier is ready
    let resolved = ensure_classifier(app, classifier).await?;

    // Step 0: Replay any pending actions (mark_read, send, etc.)
    if let Err(e) = tasks::replay_pending_actions(pool).await {
        logger::warn(&format!("Action replay error: {}", e));
    }

    // Step 1: Always fetch latest messages for all onboarded accounts.
    // This runs even during onboarding so new mail keeps arriving.
    let _ = tasks::run_incremental_sync_all(app, pool, &resolved).await;
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
        "trust_network" => tasks::run_trust_network(app, pool, &account_id, &task, &resolved).await?,
        "historical_fetch" => tasks::run_historical_fetch(app, pool, &account_id, &task, &resolved).await?,
        "connection_history" => {
            tasks::run_connection_history(app, pool, &account_id, &task, &resolved).await?;
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

    let read_only = sqlite::settings::get_setting(pool, "read_only")?
        .map(|v| v != "false")
        .unwrap_or(true);

    let conn = connection::connect_with_tls(&creds.host, creds.port, creds.tls, &creds.email, &creds.password, read_only).await?;

    let self_emails = sqlite::entities::get_self_emails(pool, account_id)?;

    Ok((creds, self_emails, conn))
}

pub fn process_changes(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
    classifier: &Arc<ClassifierState>,
) -> Result<(), EddieError> {
    // Update trust network from new sent messages (before classify sets processed_at)
    let start = std::time::Instant::now();
    let extracted = helpers::entity_extraction::extract_entities_from_new_messages(pool, account_id)?;
    if extracted > 0 {
        logger::debug(&format!("Extracted {} connections in {}", extracted, logger::fmt_ms(start.elapsed())));
    }

    // Populate display_name on entities from message from_name headers
    let names_updated = sqlite::entities::update_display_names_from_messages(pool, account_id)?;
    if names_updated > 0 {
        logger::debug(&format!("Updated {} entity display names", names_updated));
    }

    helpers::status_emit::emit_status(app, "classifying", "Identifying Points & Circles...");
    let start = std::time::Instant::now();
    let stats = helpers::message_classification::classify_messages(pool, account_id, classifier)?;
    logger::debug(&format!(
        "Classified {} messages in {} (rules={}, model={})",
        stats.total, logger::fmt_ms(start.elapsed()), stats.rules, stats.model
    ));

    helpers::status_emit::emit_status(app, "distilling", "Classifying Requests with AI...");
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
