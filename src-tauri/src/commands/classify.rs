
use crate::adapters::sqlite;
use crate::services::sync;
use crate::services::sync::helpers::message_classification::ClassifierState;
use crate::error::EddieError;
use crate::services::logger;
use std::sync::Arc;

#[tauri::command]
pub async fn reclassify(
    pool: tauri::State<'_, sqlite::DbPool>,
    classifier: tauri::State<'_, Arc<ClassifierState>>,
    app: tauri::AppHandle,
    account_id: String,
) -> Result<String, EddieError> {
    logger::info(&format!("Reclassifying all messages: account_id={}", account_id));
    sqlite::messages::reset_classifications(&pool, &account_id)?;
    sync::worker::process_changes(&app, &pool, &account_id, &classifier)?;
    logger::info(&format!("Reclassification complete: account_id={}", account_id));
    Ok("Reclassification complete".to_string())
}
