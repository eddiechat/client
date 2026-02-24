use crate::adapters::sqlite;
use crate::adapters::sqlite::conversations::{Conversation, Cluster, Thread};
use crate::adapters::sqlite::messages::Message;
use crate::error::EddieError;
use crate::services::logger;

#[tauri::command]
pub async fn fetch_conversations(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
) -> Result<Vec<Conversation>, EddieError> {
    sqlite::conversations::fetch_conversations(&pool, &account_id)
}

#[tauri::command]
pub async fn fetch_conversation_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    conversation_id: String,
) -> Result<Vec<Message>, EddieError> {
    sqlite::messages::fetch_conversation_messages(&pool, &account_id, &conversation_id)
}

#[tauri::command]
pub async fn fetch_cluster_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    cluster_id: String,
) -> Result<Vec<Message>, EddieError> {
    if let Some(skill_id) = cluster_id.strip_prefix("skill:") {
        sqlite::messages::fetch_skill_match_messages(&pool, &account_id, skill_id)
    } else {
        sqlite::messages::fetch_cluster_messages(&pool, &account_id, &cluster_id)
    }
}

#[tauri::command]
pub async fn fetch_clusters(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
) -> Result<Vec<Cluster>, EddieError> {
    sqlite::conversations::fetch_clusters(&pool, &account_id)
}

#[tauri::command]
pub async fn fetch_cluster_threads(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    cluster_id: String,
) -> Result<Vec<Thread>, EddieError> {
    sqlite::conversations::fetch_cluster_threads(&pool, &account_id, &cluster_id)
}

#[tauri::command]
pub async fn fetch_thread_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    thread_id: String,
) -> Result<Vec<Message>, EddieError> {
    sqlite::messages::fetch_thread_messages(&pool, &account_id, &thread_id)
}

#[tauri::command]
pub async fn group_domains(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    name: String,
    domains: Vec<String>,
) -> Result<String, EddieError> {
    sqlite::line_groups::group_domains(&pool, &account_id, &name, &domains)
}

#[tauri::command]
pub async fn ungroup_domains(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    group_id: String,
) -> Result<(), EddieError> {
    sqlite::line_groups::ungroup_domains(&pool, &account_id, &group_id)
}

#[tauri::command]
pub async fn move_to_lines(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    emails: Vec<String>,
) -> Result<(), EddieError> {
    for email in &emails {
        sqlite::entities::delete_entity(&pool, &account_id, email)?;
    }
    logger::info(&format!("Deleted entities, rebuilding conversations: account_id={}", account_id));
    sqlite::conversations::rebuild_conversations(&pool, &account_id)?;
    Ok(())
}

#[tauri::command]
pub async fn fetch_recent_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    limit: u32,
) -> Result<Vec<Message>, EddieError> {
    sqlite::messages::fetch_recent_messages(&pool, &account_id, limit)
}