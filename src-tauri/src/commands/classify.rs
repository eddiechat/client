
use crate::adapters::sqlite;
use crate::services::sync;
use crate::error::EddieError;
use crate::services::logger;
use crate::SharedClassifier;

#[tauri::command]
pub async fn reclassify(
    pool: tauri::State<'_, sqlite::DbPool>,
    classifier: tauri::State<'_, SharedClassifier>,
    app: tauri::AppHandle,
    account_id: String,
) -> Result<String, EddieError> {
    logger::info(&format!("Reclassifying all messages: account_id={}", account_id));

    let resolved = classifier.read().await
        .as_ref()
        .cloned()
        .ok_or_else(|| EddieError::Backend("Classifier not loaded yet".to_string()))?;

    sqlite::messages::reset_classifications(&pool, &account_id)?;
    sync::worker::process_changes(&app, &pool, &account_id, &resolved)?;
    logger::info(&format!("Reclassification complete: account_id={}", account_id));
    Ok("Reclassification complete".to_string())
}
