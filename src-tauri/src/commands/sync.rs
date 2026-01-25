//! Sync engine Tauri commands
//!
//! Provides commands for the frontend to interact with the sync engine.
//! The sync engine is started automatically when accounts are accessed.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::config;
use crate::sync::action_queue::ActionType;
use crate::sync::db::{CachedConversation, CachedMessage};
use crate::sync::engine::{SyncConfig, SyncEngine, SyncState, SyncStatus};
use crate::types::error::HimalayaError;

/// Sync engine manager state - manages engines for multiple accounts
pub struct SyncManager {
    engines: RwLock<HashMap<String, Arc<SyncEngine>>>,
    default_db_dir: PathBuf,
}

impl SyncManager {
    pub fn new() -> Self {
        // In dev mode, use .sqlite relative to project root for easier debugging
        let db_dir = if cfg!(debug_assertions) {
            PathBuf::from("../.sqlite")
        } else {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("eddie.chat")
                .join("sync")
        };

        // Ensure directory exists
        let _ = std::fs::create_dir_all(&db_dir);

        info!("Sync database directory: {:?}", db_dir);

        Self {
            engines: RwLock::new(HashMap::new()),
            default_db_dir: db_dir,
        }
    }

    /// Get or create sync engine for an account
    pub async fn get_or_create(&self, account_id: &str) -> Result<Arc<SyncEngine>, HimalayaError> {
        // Check if engine exists
        {
            let engines = self.engines.read().await;
            if let Some(engine) = engines.get(account_id) {
                return Ok(engine.clone());
            }
        }

        // Create new engine
        info!("Creating sync engine for account: {}", account_id);

        let app_config = config::get_config()?;
        let (name, account) = app_config
            .get_account(Some(account_id))
            .ok_or_else(|| HimalayaError::AccountNotFound(account_id.to_string()))?;

        let db_path = self.default_db_dir.join(format!("{}.db", name));
        info!("Database path: {:?}", db_path);

        let sync_config = SyncConfig {
            db_path,
            initial_sync_days: 365,
            max_cache_age_days: 365,
            auto_classify: true,
            sync_folders: vec![],
            fetch_page_size: 1000,
        };

        let (engine, _event_rx) = SyncEngine::new(
            name.to_string(),
            account.email.clone(),
            account.clone(),
            sync_config,
        )?;

        let engine = Arc::new(engine);

        // Store engine
        {
            let mut engines = self.engines.write().await;
            engines.insert(account_id.to_string(), engine.clone());
        }

        Ok(engine)
    }

    /// Get sync engine for account (if exists)
    pub async fn get(&self, account_id: &str) -> Option<Arc<SyncEngine>> {
        let engines = self.engines.read().await;
        engines.get(account_id).cloned()
    }

    /// Remove sync engine for account
    pub async fn remove(&self, account_id: &str) {
        let mut engines = self.engines.write().await;
        if let Some(engine) = engines.remove(account_id) {
            engine.shutdown();
        }
    }

    /// Get all engine account IDs
    pub async fn get_account_ids(&self) -> Vec<String> {
        let engines = self.engines.read().await;
        engines.keys().cloned().collect()
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

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
}

impl From<SyncStatus> for SyncStatusResponse {
    fn from(s: SyncStatus) -> Self {
        Self {
            state: match s.state {
                SyncState::Idle => "idle".to_string(),
                SyncState::Connecting => "connecting".to_string(),
                SyncState::Syncing => "syncing".to_string(),
                SyncState::InitialSync => "initial_sync".to_string(),
                SyncState::Error => "error".to_string(),
            },
            account_id: s.account_id,
            current_folder: s.current_folder,
            progress_current: s.progress.as_ref().map(|p| p.current),
            progress_total: s.progress.as_ref().and_then(|p| p.total),
            progress_message: s.progress.map(|p| p.message),
            last_sync: s.last_sync.map(|d| d.to_rfc3339()),
            error: s.error,
            is_online: s.is_online,
            pending_actions: s.pending_actions,
        }
    }
}

/// Cached conversation for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationResponse {
    pub id: i64,
    pub participant_key: String,
    pub participants: Vec<ParticipantInfo>,
    pub last_message_date: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_from: Option<String>,
    pub message_count: u32,
    pub unread_count: u32,
    pub is_outgoing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub email: String,
    pub name: Option<String>,
}

impl From<CachedConversation> for ConversationResponse {
    fn from(c: CachedConversation) -> Self {
        let participants: Vec<ParticipantInfo> = serde_json::from_str(&c.participants)
            .unwrap_or_default();

        Self {
            id: c.id,
            participant_key: c.participant_key,
            participants,
            last_message_date: c.last_message_date.map(|d| d.to_rfc3339()),
            last_message_preview: c.last_message_preview,
            last_message_from: c.last_message_from,
            message_count: c.message_count,
            unread_count: c.unread_count,
            is_outgoing: c.is_outgoing,
        }
    }
}

/// Cached message for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMessageResponse {
    pub id: i64,
    pub folder: String,
    pub uid: u32,
    pub message_id: Option<String>,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub subject: Option<String>,
    pub date: Option<String>,
    pub flags: Vec<String>,
    pub has_attachment: bool,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub body_cached: bool,
}

impl From<CachedMessage> for CachedMessageResponse {
    fn from(m: CachedMessage) -> Self {
        Self {
            id: m.id,
            folder: m.folder_name,
            uid: m.uid,
            message_id: m.message_id,
            from_address: m.from_address,
            from_name: m.from_name,
            to_addresses: serde_json::from_str(&m.to_addresses).unwrap_or_default(),
            cc_addresses: m.cc_addresses
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default(),
            subject: m.subject,
            date: m.date.map(|d| d.to_rfc3339()),
            flags: serde_json::from_str(&m.flags).unwrap_or_default(),
            has_attachment: m.has_attachment,
            text_body: m.text_body,
            html_body: m.html_body,
            body_cached: m.body_cached,
        }
    }
}

// ========== Tauri Commands ==========

/// Initialize sync engine for an account and start syncing
#[tauri::command]
pub async fn init_sync_engine(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<SyncStatusResponse, String> {
    let account_id = get_account_id(account)?;
    info!("Initializing sync engine for: {}", account_id);

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    // Start full sync in background
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        if let Err(e) = engine_clone.full_sync().await {
            error!("Background sync failed: {}", e);
        }
    });

    let status = engine.status().await;
    Ok(status.into())
}

/// Get sync status
#[tauri::command]
pub async fn get_sync_status(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<SyncStatusResponse, String> {
    let account_id = get_account_id(account)?;

    if let Some(engine) = manager.get(&account_id).await {
        let status = engine.status().await;
        Ok(status.into())
    } else {
        // Return idle status if not initialized
        Ok(SyncStatusResponse {
            state: "idle".to_string(),
            account_id,
            current_folder: None,
            progress_current: None,
            progress_total: None,
            progress_message: None,
            last_sync: None,
            error: None,
            is_online: false,
            pending_actions: 0,
        })
    }
}

/// Trigger a sync for a folder
#[tauri::command]
pub async fn sync_folder(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    folder: Option<String>,
) -> Result<(), String> {
    let account_id = get_account_id(account)?;
    let folder = folder.unwrap_or_else(|| "INBOX".to_string());

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    engine
        .sync_folder(&folder)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Perform full sync for an account
#[tauri::command]
pub async fn initial_sync(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<(), String> {
    let account_id = get_account_id(account)?;
    info!("Starting initial sync for: {}", account_id);

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    engine.full_sync().await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Get cached conversations from SQLite
#[tauri::command]
pub async fn get_cached_conversations(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    include_hidden: Option<bool>,
) -> Result<Vec<ConversationResponse>, String> {
    let account_id = get_account_id(account)?;
    let include_hidden = include_hidden.unwrap_or(false);

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    let conversations = engine
        .get_conversations(include_hidden)
        .map_err(|e| e.to_string())?;

    Ok(conversations.into_iter().map(|c| c.into()).collect())
}

/// Get messages for a cached conversation
#[tauri::command]
pub async fn get_cached_conversation_messages(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    conversation_id: i64,
) -> Result<Vec<CachedMessageResponse>, String> {
    let account_id = get_account_id(account)?;

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    let messages = engine
        .get_conversation_messages(conversation_id)
        .map_err(|e| e.to_string())?;

    Ok(messages.into_iter().map(|m| m.into()).collect())
}

/// Fetch message body (on-demand, if not cached)
#[tauri::command]
pub async fn fetch_message_body(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    message_id: i64,
) -> Result<CachedMessageResponse, String> {
    let account_id = get_account_id(account)?;

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    let message = engine
        .fetch_message_body(message_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(message.into())
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
) -> Result<i64, String> {
    let account_id = get_account_id(account)?;

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    let action = match action_type.as_str() {
        "add_flags" => ActionType::AddFlags {
            folder,
            uids,
            flags: flags.unwrap_or_default(),
        },
        "remove_flags" => ActionType::RemoveFlags {
            folder,
            uids,
            flags: flags.unwrap_or_default(),
        },
        "delete" => ActionType::Delete { folder, uids },
        "move" => ActionType::Move {
            source_folder: folder,
            target_folder: target_folder.ok_or("target_folder required for move")?,
            uids,
        },
        "copy" => ActionType::Copy {
            source_folder: folder,
            target_folder: target_folder.ok_or("target_folder required for copy")?,
            uids,
        },
        _ => return Err(format!("Unknown action type: {}", action_type)),
    };

    engine.queue_action(action).map_err(|e| e.to_string())
}

/// Set online status
#[tauri::command]
pub async fn set_sync_online(
    manager: State<'_, SyncManager>,
    account: Option<String>,
    online: bool,
) -> Result<(), String> {
    let account_id = get_account_id(account)?;

    if let Some(engine) = manager.get(&account_id).await {
        engine.set_online(online);
    }
    Ok(())
}

/// Check if there are pending sync actions
#[tauri::command]
pub async fn has_pending_sync_actions(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<bool, String> {
    let account_id = get_account_id(account)?;

    let engine = manager
        .get_or_create(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    engine
        .action_queue()
        .has_pending(&account_id)
        .map_err(|e| e.to_string())
}

/// Shutdown sync engine for an account
#[tauri::command]
pub async fn shutdown_sync_engine(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<(), String> {
    let account_id = get_account_id(account)?;
    manager.remove(&account_id).await;
    Ok(())
}

// Helper function to get account ID
fn get_account_id(account: Option<String>) -> Result<String, String> {
    if let Some(id) = account {
        Ok(id)
    } else {
        let app_config = config::get_config().map_err(|e| e.to_string())?;
        app_config
            .default_account_name()
            .map(|s| s.to_string())
            .ok_or_else(|| "No default account configured".to_string())
    }
}
