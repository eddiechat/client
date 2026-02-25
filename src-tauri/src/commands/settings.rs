use crate::adapters::sqlite;
use crate::error::EddieError;
use crate::services::ollama::{OllamaEntry, OllamaState};

#[tauri::command]
pub async fn get_setting(
    pool: tauri::State<'_, sqlite::DbPool>,
    key: String,
) -> Result<Option<String>, EddieError> {
    sqlite::settings::get_setting(&pool, &key)
}

#[tauri::command]
pub async fn set_setting(
    pool: tauri::State<'_, sqlite::DbPool>,
    key: String,
    value: String,
) -> Result<(), EddieError> {
    sqlite::settings::set_setting(&pool, &key, &value)
}

#[tauri::command]
pub async fn get_ollama_models(
    ollama: tauri::State<'_, OllamaState>,
    key: String,
) -> Result<OllamaEntry, EddieError> {
    let guard = ollama.read().await;
    Ok(guard.get(&key).cloned().unwrap_or(OllamaEntry {
        models: Vec::new(),
        selected_model: None,
    }))
}
