use serde::Serialize;

use crate::adapters::sqlite::{self, DbPool};
use crate::error::EddieError;

#[derive(Debug, Serialize)]
pub struct EntityResult {
    pub email: String,
    pub display_name: Option<String>,
    pub trust_level: String,
}

#[tauri::command]
pub async fn search_entities(
    pool: tauri::State<'_, DbPool>,
    account_id: String,
    query: String,
) -> Result<Vec<EntityResult>, EddieError> {
    let results = sqlite::entities::search_entities(&pool, &account_id, &query)?;
    Ok(results)
}

#[derive(Debug, Serialize)]
pub struct AliasInfo {
    pub email: String,
    pub is_primary: bool,
}

#[tauri::command]
pub async fn get_user_aliases(
    pool: tauri::State<'_, DbPool>,
    account_id: String,
) -> Result<Vec<AliasInfo>, EddieError> {
    let aliases = sqlite::entities::get_user_aliases(&pool, &account_id)?;
    Ok(aliases)
}
