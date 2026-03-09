use crate::adapters::sqlite;
use crate::error::EddieError;

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
