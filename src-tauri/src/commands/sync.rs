//! Sync engine Tauri commands
//!
//! Thin command wrappers that delegate to the sync engine.
//! Business logic is handled by the SyncEngine itself.

use std::collections::HashMap;
use tauri::State;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::services::resolve_account_id_string;
use crate::state::SyncManager;
use crate::sync::action_queue::ActionType;
use crate::sync::engine::SyncEvent;
use crate::types::responses::{CachedChatMessageResponse, ConversationResponse, EntityResponse, SyncStatusResponse};
use crate::types::EddieError;

// ========== Tauri Commands ==========

/// Initialize sync engine for an account and start syncing
#[tauri::command]
pub async fn init_sync_engine(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<SyncStatusResponse, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Initializing sync engine for: {}", account_id);

    let engine = manager.get_or_create(&account_id).await?;

    // Start full sync and monitoring in background
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        // Perform initial sync
        {
            let engine_guard = engine_clone.read().await;
            if let Err(e) = engine_guard.full_sync().await {
                error!("Background sync failed: {}", e);
                return;
            }
        }

        // Start monitoring after successful sync
        info!("Initial sync complete, starting monitoring...");
        {
            let mut engine_guard = engine_clone.write().await;
            if let Err(e) = engine_guard.start_monitoring().await {
                error!("Failed to start monitoring: {}", e);
            }
        }

        // Run the notification processing loop
        run_monitor_loop(engine_clone).await;
    });

    let status = engine.read().await.status().await;
    Ok(status.into())
}

/// Run the monitoring notification loop
async fn run_monitor_loop(engine: std::sync::Arc<RwLock<crate::sync::engine::SyncEngine>>) {
    info!("Starting notification processing loop...");
    loop {
        let engine_guard = engine.read().await;
        if !engine_guard.is_monitoring() {
            debug!("Monitoring stopped, exiting notification loop");
            break;
        }

        // Process any pending notifications
        if !engine_guard.process_monitor_notification().await {
            // No notification, sleep briefly
            drop(engine_guard);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}

/// Get sync status
#[tauri::command]
pub async fn get_sync_status(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<SyncStatusResponse, EddieError> {
    let account_id = resolve_account_id_string(account)?;

    if let Some(engine) = manager.get(&account_id).await {
        let status = engine.read().await.status().await;
        Ok(status.into())
    } else {
        Ok(SyncStatusResponse::idle(account_id))
    }
}

/// Trigger a sync for a folder
#[tauri::command]
pub async fn sync_folder(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    folder: Option<String>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    let folder = folder.unwrap_or_else(|| "INBOX".to_string());

    let engine = manager.get_or_create(&account_id).await?;
    engine
        .read()
        .await
        .sync_folder(&folder)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(())
}

/// Perform full sync for an account
#[tauri::command]
pub async fn initial_sync(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Starting initial sync for: {}", account_id);

    let engine = manager.get_or_create(&account_id).await?;
    engine
        .read()
        .await
        .full_sync()
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(())
}

/// Get cached conversations from SQLite
///
/// Tab parameter:
/// - "connections": Only show conversations classified as 'chat'
/// - "all": Show all conversations regardless of classification
/// - "others": Not implemented yet (returns empty list)
#[tauri::command]
pub async fn get_cached_conversations(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    tab: Option<String>,
) -> Result<Vec<ConversationResponse>, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    let tab = tab.as_deref().unwrap_or("connections");

    // Determine classification filter based on tab
    let classification_filter = match tab {
        "connections" => Some("chat"),
        "all" => None,
        "others" => {
            // Not implemented yet - return empty list
            return Ok(vec![]);
        }
        _ => Some("chat"), // Default to connections
    };

    let engine = manager.get_or_create(&account_id).await?;
    let conversations = engine
        .read()
        .await
        .get_conversations(classification_filter)
        .map_err(|e| EddieError::Database(e.to_string()))?;

    tracing::info!(
        "get_cached_conversations: tab={}, filter={:?}, found {} conversations",
        tab,
        classification_filter,
        conversations.len()
    );

    Ok(conversations.into_iter().map(|c| c.into()).collect())
}

/// Get messages for a cached conversation
#[tauri::command]
pub async fn get_cached_conversation_messages(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    conversation_id: i64,
) -> Result<Vec<CachedChatMessageResponse>, EddieError> {
    let account_id = resolve_account_id_string(account)?;

    let engine = manager.get_or_create(&account_id).await?;
    let messages = engine
        .read()
        .await
        .get_conversation_messages(conversation_id)
        .map_err(|e| EddieError::Database(e.to_string()))?;

    Ok(messages.into_iter().map(|m| m.into()).collect())
}

/// Fetch message body (on-demand, if not cached)
#[tauri::command]
pub async fn fetch_message_body(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    message_id: i64,
) -> Result<CachedChatMessageResponse, EddieError> {
    let account_id = resolve_account_id_string(account)?;

    let engine = manager.get_or_create(&account_id).await?;
    let message = engine
        .read()
        .await
        .fetch_message_body(message_id)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(message.into())
}

/// Rebuild all conversations from cached messages
///
/// This regenerates conversation participant keys from cached messages,
/// which is useful after adding CC support or fixing participant grouping.
/// Returns the number of conversations rebuilt.
#[tauri::command]
pub async fn rebuild_conversations(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    user_email: String,
) -> Result<u32, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Rebuilding conversations for account: {}", account_id);

    let engine = manager.get_or_create(&account_id).await?;
    let count = engine
        .read()
        .await
        .rebuild_all_conversations(&account_id, &user_email)?;

    info!("Rebuilt {} conversations", count);
    Ok(count)
}

/// Drop the sync database and re-fetch all messages
///
/// This deletes the local cache database and triggers a full re-sync.
/// Useful for testing or recovering from database corruption.
#[tauri::command]
pub async fn drop_and_resync(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Dropping database and re-syncing for account: {}", account_id);

    // Shutdown the sync engine to close database connections
    manager.remove(&account_id).await;

    // Delete the database file
    let db_path = if cfg!(debug_assertions) {
        std::path::PathBuf::from("../.sqlite/eddie_sync.db")
    } else {
        std::path::PathBuf::from("eddie_sync.db")
    };

    if db_path.exists() {
        std::fs::remove_file(&db_path)
            .map_err(|e| EddieError::Backend(format!("Failed to delete database: {}", e)))?;
        info!("Deleted database file: {:?}", db_path);
    }

    // Re-initialize and trigger full sync
    let engine = manager.get_or_create(&account_id).await?;
    engine
        .read()
        .await
        .full_sync()
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    info!("Database dropped and full sync initiated for account: {}", account_id);
    Ok(())
}

/// Queue an action for offline support
#[tauri::command]
pub async fn queue_sync_action(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    action_type: String,
    folder: String,
    uids: Vec<u32>,
    flags: Option<Vec<String>>,
    target_folder: Option<String>,
) -> Result<i64, EddieError> {
    let account_id = resolve_account_id_string(account)?;

    let engine = manager.get_or_create(&account_id).await?;
    let action = parse_action_type(&action_type, folder, uids, flags, target_folder)?;

    let result = engine
        .read()
        .await
        .queue_action(action)
        .map_err(|e| EddieError::Database(e.to_string()))?;
    Ok(result)
}

/// Parse action type from string
fn parse_action_type(
    action_type: &str,
    folder: String,
    uids: Vec<u32>,
    flags: Option<Vec<String>>,
    target_folder: Option<String>,
) -> Result<ActionType, EddieError> {
    match action_type {
        "add_flags" => Ok(ActionType::AddFlags {
            folder,
            uids,
            flags: flags.unwrap_or_default(),
        }),
        "remove_flags" => Ok(ActionType::RemoveFlags {
            folder,
            uids,
            flags: flags.unwrap_or_default(),
        }),
        "delete" => Ok(ActionType::Delete { folder, uids }),
        "move" => Ok(ActionType::Move {
            source_folder: folder,
            target_folder: target_folder
                .ok_or_else(|| EddieError::InvalidInput("target_folder required for move".into()))?,
            uids,
        }),
        "copy" => Ok(ActionType::Copy {
            source_folder: folder,
            target_folder: target_folder
                .ok_or_else(|| EddieError::InvalidInput("target_folder required for copy".into()))?,
            uids,
        }),
        _ => Err(EddieError::InvalidInput(format!(
            "Unknown action type: {}",
            action_type
        ))),
    }
}

/// Set online status
#[tauri::command]
pub async fn set_sync_online(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    online: bool,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;

    if let Some(engine) = manager.get(&account_id).await {
        engine.read().await.set_online(online);
    }
    Ok(())
}

/// Check if there are pending sync actions
#[tauri::command]
pub async fn has_pending_sync_actions(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<bool, EddieError> {
    let account_id = resolve_account_id_string(account)?;

    let engine = manager.get_or_create(&account_id).await?;
    let result = engine
        .read()
        .await
        .action_queue()
        .has_pending(&account_id)
        .map_err(|e| EddieError::Database(e.to_string()))?;
    Ok(result)
}

/// Start monitoring for mailbox changes
#[tauri::command]
pub async fn start_monitoring(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Starting monitoring for: {}", account_id);

    let engine = manager.get_or_create(&account_id).await?;

    // Start monitoring
    {
        let mut engine_guard = engine.write().await;
        engine_guard
            .start_monitoring()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;
    }

    // Spawn the notification processing loop
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        run_monitor_loop(engine_clone).await;
    });

    Ok(())
}

/// Stop monitoring for mailbox changes
#[tauri::command]
pub async fn stop_monitoring(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Stopping monitoring for: {}", account_id);

    if let Some(engine) = manager.get(&account_id).await {
        engine.read().await.stop_monitoring();
    }

    Ok(())
}

/// Shutdown sync engine for an account
#[tauri::command]
pub async fn shutdown_sync_engine(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    info!("Shutting down sync engine for: {}", account_id);
    manager.remove(&account_id).await;
    Ok(())
}

/// Mark all unread messages in a conversation as read
#[tauri::command]
pub async fn mark_conversation_read(
    manager: State<'_, SyncManager>,
    app_handle: tauri::AppHandle,
    account: Option<String>,
    conversation_id: i64,
) -> Result<(), EddieError> {
    use tauri::Emitter;

    let account_id = resolve_account_id_string(account)?;
    debug!(
        "Marking conversation {} as read for account: {}",
        conversation_id, account_id
    );

    let engine = manager.get_or_create(&account_id).await?;
    let engine_guard = engine.read().await;
    let db = engine_guard.database();

    // Get all messages in the conversation
    let messages = db
        .get_conversation_messages(conversation_id)
        .map_err(|e| EddieError::Database(e.to_string()))?;

    // Find unread messages (those without \Seen flag)
    let mut unread_by_folder: HashMap<String, Vec<u32>> = HashMap::new();
    let mut total_unread = 0i32;

    for msg in &messages {
        let flags: Vec<String> = serde_json::from_str(&msg.flags).unwrap_or_default();
        if !flags.iter().any(|f| f == "\\Seen") {
            unread_by_folder
                .entry(msg.folder_name.clone())
                .or_default()
                .push(msg.uid);
            total_unread += 1;
        }
    }

    if unread_by_folder.is_empty() {
        debug!("No unread messages in conversation {}", conversation_id);
        return Ok(());
    }

    info!(
        "Marking {} unread messages as read in conversation {}",
        total_unread, conversation_id
    );

    // Update local database flags and queue actions for each folder
    for (folder, uids) in &unread_by_folder {
        // Update local database flags
        for uid in uids {
            db.add_message_flags(&account_id, folder, *uid, &["\\Seen".to_string()])
                .map_err(|e| EddieError::Database(e.to_string()))?;
        }

        // Queue the action for IMAP server sync
        let action = ActionType::AddFlags {
            folder: folder.clone(),
            uids: uids.clone(),
            flags: vec!["\\Seen".to_string()],
        };
        engine_guard
            .queue_action(action)
            .map_err(|e| EddieError::Database(e.to_string()))?;
    }

    // Update conversation unread count
    db.adjust_conversation_unread_count(conversation_id, -total_unread)
        .map_err(|e| EddieError::Database(e.to_string()))?;

    // Emit sync event so UI refreshes
    let _ = app_handle.emit(
        "sync-event",
        SyncEvent::ConversationsUpdated {
            conversation_ids: vec![conversation_id],
        },
    );

    Ok(())
}

/// Search entities for autocomplete suggestions
/// Returns up to 5 entities matching the query, prioritizing connections and recent contacts
#[tauri::command]
pub async fn search_entities(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<EntityResponse>, EddieError> {
    let account_id = resolve_account_id_string(account)?;
    let limit = limit.unwrap_or(5).min(10); // Default to 5, max 10

    if query.is_empty() {
        return Ok(vec![]);
    }

    let engine = manager.get_or_create(&account_id).await?;

    let entities = engine
        .read()
        .await
        .search_entities(&query, limit)?;

    Ok(entities.into_iter().map(|e| e.into()).collect())
}
