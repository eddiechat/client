//! Sync engine Tauri commands
//!
//! Thin command wrappers that delegate to the sync/adapter layer.
//! The background worker handles actual IMAP synchronization.

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::adapters::sqlite::{self, DbPool};
use crate::engine;
use crate::services::resolve_account_id_string;
use crate::sync::db::is_read_only_mode;
use crate::types::EddieError;

// ========== Tauri Commands ==========

/// Initialize sync engine for an account and start syncing.
/// Seeds onboarding tasks and wakes the background worker.
#[tauri::command]
pub async fn init_sync_engine(
    pool: State<'_, DbPool>,
    wake_tx: State<'_, mpsc::Sender<()>>,
    account: Option<String>,
) -> Result<SyncStatusResponse, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Initializing sync for: {}", account_id);

    // Ensure account exists in sync DB
    sqlite::accounts::ensure_account(&pool, &account_id)?;

    // Seed onboarding tasks (idempotent - INSERT OR IGNORE)
    sqlite::onboarding_tasks::seed_tasks(&pool, &account_id)?;

    // Register user entity
    sqlite::entities::insert_entity(&pool, &account_id, &account_id, "account", "user")?;

    // Wake the background worker
    let _ = wake_tx.send(()).await;

    // Return current status
    get_sync_status_inner(&pool, &account_id)
}

/// Get sync status for an account
#[tauri::command]
pub async fn get_sync_status(
    pool: State<'_, DbPool>,
    account: Option<String>,
) -> Result<SyncStatusResponse, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    get_sync_status_inner(&pool, &account_id)
}

fn get_sync_status_inner(
    pool: &DbPool,
    account_id: &str,
) -> Result<SyncStatusResponse, EddieError> {
    let tasks = sqlite::onboarding_tasks::get_tasks(pool, account_id)?;

    let state = if tasks.is_empty() {
        "idle".to_string()
    } else if tasks.iter().all(|t| t.status == "done") {
        "synced".to_string()
    } else if tasks.iter().any(|t| t.status == "in_progress") {
        "syncing".to_string()
    } else {
        "pending".to_string()
    };

    let current_task = tasks
        .iter()
        .find(|t| t.status != "done")
        .map(|t| t.name.clone());
    let done_count = tasks.iter().filter(|t| t.status == "done").count() as u32;
    let total_count = tasks.len() as u32;

    Ok(SyncStatusResponse {
        state,
        account_id: account_id.to_string(),
        current_folder: current_task,
        progress_current: Some(done_count),
        progress_total: Some(total_count),
        progress_message: None,
        last_sync: None,
        error: None,
        is_online: true,
        pending_actions: 0,
        monitor_mode: None,
    })
}

/// Trigger a manual sync cycle
#[tauri::command]
pub async fn sync_now(
    wake_tx: State<'_, mpsc::Sender<()>>,
) -> Result<String, EddieError> {
    info!("Manual sync triggered");
    let _ = wake_tx.send(()).await;
    Ok("Sync triggered".to_string())
}

/// Get cached conversations from SQLite
#[tauri::command]
pub async fn get_cached_conversations(
    pool: State<'_, DbPool>,
    account: Option<String>,
    tab: Option<String>,
) -> Result<Vec<sqlite::conversations::Conversation>, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    let tab = tab.as_deref().unwrap_or("connections");

    let mut conversations = sqlite::conversations::fetch_conversations(&pool, &account_id)?;

    // Filter by tab/classification
    match tab {
        "connections" => {
            conversations.retain(|c| c.classification == "connections");
        }
        "others" => {
            conversations.retain(|c| c.classification == "others");
        }
        "all" => {} // No filter
        _ => {
            conversations.retain(|c| c.classification == "connections");
        }
    }

    info!(
        "get_cached_conversations: tab={}, found {} conversations",
        tab,
        conversations.len()
    );

    Ok(conversations)
}

/// Get messages for a cached conversation
#[tauri::command]
pub async fn get_cached_conversation_messages(
    pool: State<'_, DbPool>,
    account: Option<String>,
    conversation_id: String,
) -> Result<Vec<sqlite::messages::Message>, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    sqlite::messages::fetch_conversation_messages(&pool, &account_id, &conversation_id)
}

/// Fetch message body - currently reads from cache only
#[tauri::command]
pub async fn fetch_message_body(
    pool: State<'_, DbPool>,
    account: Option<String>,
    message_id: String,
) -> Result<Option<sqlite::messages::Message>, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, date, from_address, from_name, to_addresses, cc_addresses,
                subject, body_text, body_html, has_attachments, imap_flags, distilled_text
         FROM messages
         WHERE account_id = ?1 AND id = ?2",
    )?;

    let message = stmt
        .query_row(rusqlite::params![account_id, message_id], |row| {
            Ok(sqlite::messages::Message {
                id: row.get(0)?,
                date: row.get(1)?,
                from_address: row.get(2)?,
                from_name: row.get(3)?,
                to_addresses: row.get(4)?,
                cc_addresses: row.get(5)?,
                subject: row.get(6)?,
                body_text: row.get(7)?,
                body_html: row.get(8)?,
                has_attachments: row.get::<_, i32>(9)? != 0,
                imap_flags: row.get(10)?,
                distilled_text: row.get(11)?,
            })
        })
        .ok();

    Ok(message)
}

/// Rebuild all conversations from cached messages
#[tauri::command]
pub async fn rebuild_conversations(
    pool: State<'_, DbPool>,
    app: tauri::AppHandle,
    account: Option<String>,
) -> Result<u32, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Rebuilding conversations for account: {}", account_id);

    let count = sqlite::conversations::rebuild_conversations(&pool, &account_id)? as u32;

    // Also reprocess classifications
    engine::worker::process_changes(&app, &pool, &account_id)?;

    info!("Rebuilt {} conversations", count);
    Ok(count)
}

/// Reclassify all messages for an account
#[tauri::command]
pub async fn reclassify(
    pool: State<'_, DbPool>,
    app: tauri::AppHandle,
    account: Option<String>,
) -> Result<String, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Reclassifying all messages for account: {}", account_id);
    sqlite::messages::reset_classifications(&pool, &account_id)?;
    engine::worker::process_changes(&app, &pool, &account_id)?;
    info!("Reclassification complete for account: {}", account_id);
    Ok("Reclassification complete".to_string())
}

/// Drop the sync database contents for an account and re-fetch
#[tauri::command]
pub async fn drop_and_resync(
    pool: State<'_, DbPool>,
    wake_tx: State<'_, mpsc::Sender<()>>,
    account: Option<String>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!(
        "Dropping sync data and re-syncing for account: {}",
        account_id
    );

    // Delete account row — ON DELETE CASCADE removes all child data
    // (messages, conversations, entities, action_queue, sync_state, folder_sync, onboarding_tasks)
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM accounts WHERE id = ?1",
        rusqlite::params![&account_id],
    )?;
    drop(conn);

    // Recreate account row and re-seed for fresh sync
    sqlite::accounts::ensure_account(&pool, &account_id)?;
    sqlite::entities::insert_entity(&pool, &account_id, &account_id, "account", "user")?;
    sqlite::onboarding_tasks::seed_tasks(&pool, &account_id)?;

    // Wake the worker
    let _ = wake_tx.send(()).await;

    info!(
        "Sync data dropped and re-sync initiated for account: {}",
        account_id
    );
    Ok(())
}

/// Mark all unread messages in a conversation as read (local cache only)
#[tauri::command]
pub async fn mark_conversation_read(
    pool: State<'_, DbPool>,
    app_handle: tauri::AppHandle,
    account: Option<String>,
    conversation_id: String,
) -> Result<(), EddieError> {
    use tauri::Emitter;

    // Check read-only mode
    if is_read_only_mode()? {
        info!(
            "Read-only mode: Blocked mark_conversation_read - conversation_id: {}",
            conversation_id
        );
        return Err(EddieError::ReadOnlyMode);
    }

    let account_id = resolve_account_id_string(account)?;
    debug!(
        "Marking conversation {} as read for account: {}",
        conversation_id, account_id
    );

    let conn = pool.get()?;

    // Update all unseen messages in this conversation to include \Seen flag
    conn.execute(
        "UPDATE messages SET imap_flags =
            CASE
                WHEN imap_flags NOT LIKE '%Seen%' THEN
                    CASE WHEN imap_flags = '[]' OR imap_flags = ''
                        THEN '[\"\\\\Seen\"]'
                        ELSE SUBSTR(imap_flags, 1, LENGTH(imap_flags)-1) || ',\"\\\\Seen\"]'
                    END
                ELSE imap_flags
            END
        WHERE account_id = ?1 AND conversation_id = ?2 AND imap_flags NOT LIKE '%Seen%'",
        rusqlite::params![account_id, conversation_id],
    )?;

    // Update conversation unread count
    conn.execute(
        "UPDATE conversations SET unread_count = 0 WHERE account_id = ?1 AND id = ?2",
        rusqlite::params![account_id, conversation_id],
    )?;

    // Emit event so UI refreshes
    let _ = app_handle.emit("sync:conversations-updated", &conversation_id);

    Ok(())
}

/// Search entities for autocomplete suggestions
#[tauri::command]
pub async fn search_entities(
    pool: State<'_, DbPool>,
    account: Option<String>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<EntityResult>, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    let limit = limit.unwrap_or(5).min(10);

    if query.is_empty() {
        return Ok(vec![]);
    }

    let conn = pool.get()?;
    let pattern = format!("%{}%", query);

    let mut stmt = conn.prepare(
        "SELECT id, email, display_name, trust_level, last_seen
         FROM entities
         WHERE account_id = ?1 AND (email LIKE ?2 OR display_name LIKE ?2)
           AND trust_level NOT IN ('user', 'alias')
         ORDER BY
            CASE trust_level WHEN 'connection' THEN 0 ELSE 1 END,
            COALESCE(last_seen, 0) DESC
         LIMIT ?3",
    )?;

    let rows = stmt.query_map(rusqlite::params![account_id, pattern, limit], |row| {
        Ok(EntityResult {
            id: row.get(0)?,
            email: row.get(1)?,
            name: row.get(2)?,
            trust_level: row.get(3)?,
            last_seen: row.get(4)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Shutdown sync engine for an account (no-op with worker loop)
#[tauri::command]
pub async fn shutdown_sync_engine(_account: Option<String>) -> Result<(), EddieError> {
    // No-op: the background worker loop handles its own lifecycle
    Ok(())
}

// ========== Response types ==========

/// Response for sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusResponse {
    pub state: String,
    pub account_id: String,
    pub current_folder: Option<String>,
    pub progress_current: Option<u32>,
    pub progress_total: Option<u32>,
    pub progress_message: Option<String>,
    pub last_sync: Option<String>,
    pub error: Option<String>,
    pub is_online: bool,
    pub pending_actions: u32,
    pub monitor_mode: Option<String>,
}

impl Default for SyncStatusResponse {
    fn default() -> Self {
        Self {
            state: "idle".to_string(),
            account_id: String::new(),
            current_folder: None,
            progress_current: None,
            progress_total: None,
            progress_message: None,
            last_sync: None,
            error: None,
            is_online: false,
            pending_actions: 0,
            monitor_mode: None,
        }
    }
}

impl SyncStatusResponse {
    pub fn idle(account_id: String) -> Self {
        Self {
            account_id,
            ..Default::default()
        }
    }
}

/// Entity search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResult {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub trust_level: String,
    pub last_seen: Option<i64>,
}
