
use crate::adapters::sqlite;
use crate::services::sync;
use crate::error::EddieError;
use tracing::info;

#[tauri::command]
pub async fn reclassify(
    pool: tauri::State<'_, sqlite::DbPool>,
    app: tauri::AppHandle,
    account_id: String,
) -> Result<String, EddieError> {
    info!(account_id = %account_id, "Reclassifying all messages");
    sqlite::messages::reset_classifications(&pool, &account_id)?;
    sync::worker::process_changes(&app, &pool, &account_id)?;
    info!(account_id = %account_id, "Reclassification complete");
    Ok("Reclassification complete".to_string())
}