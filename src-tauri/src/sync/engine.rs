//! IMAP Sync Engine
//!
//! Maintains a local SQLite cache of email messages synchronized with IMAP servers.
//! The local database is a cache of server state, not the source of truth.

use chrono::{DateTime, Duration, Utc};
use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::backend::EmailBackend;
use crate::config::{self, AccountConfig};
use crate::sync::action_queue::{ActionQueue, ActionType, QueuedAction, ReplayResult};
use crate::sync::capability::{CapabilityDetector, CapabilityInfo, ServerCapability};
use crate::sync::classifier::MessageClassifier;
use crate::sync::conversation::ConversationGrouper;
use crate::sync::db::{
    CachedConversation, CachedMessage, FolderSyncState, SyncDatabase, SyncProgress,
};
use crate::types::error::HimalayaError;
use crate::types::Envelope;

/// Sync engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Database file path
    pub db_path: PathBuf,
    /// Initial sync: number of days to fetch immediately
    pub initial_sync_days: u32,
    /// Maximum message age to keep in cache (days)
    pub max_cache_age_days: u32,
    /// Auto-classify messages
    pub auto_classify: bool,
    /// Folders to sync (empty = INBOX + Sent)
    pub sync_folders: Vec<String>,
    /// Page size for fetching envelopes
    pub fetch_page_size: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        // In dev mode, use .sqlite relative to project root for easier debugging
        let db_path = if cfg!(debug_assertions) {
            PathBuf::from("../.sqlite/eddie_sync.db")
        } else {
            PathBuf::from("eddie_sync.db")
        };

        Self {
            db_path,
            initial_sync_days: 365, // 1 year
            max_cache_age_days: 365,
            auto_classify: true,
            sync_folders: vec![],
            fetch_page_size: 500,
        }
    }
}

/// Sync status for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub state: SyncState,
    pub account_id: String,
    pub current_folder: Option<String>,
    pub progress: Option<SyncProgressInfo>,
    pub last_sync: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub is_online: bool,
    pub pending_actions: u32,
}

/// Sync state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncState {
    Idle,
    Connecting,
    Syncing,
    InitialSync,
    Error,
}

/// Progress information for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgressInfo {
    pub phase: String,
    pub current: u32,
    pub total: Option<u32>,
    pub message: String,
}

/// Sync result
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub new_messages: u32,
    pub updated_messages: u32,
    pub deleted_messages: u32,
    pub affected_conversations: Vec<i64>,
}

/// Event emitted by the sync engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncEvent {
    StatusChanged(SyncStatus),
    NewMessages { folder: String, count: u32 },
    MessagesDeleted { folder: String, uids: Vec<u32> },
    FlagsChanged { folder: String, uids: Vec<u32> },
    ConversationsUpdated { conversation_ids: Vec<i64> },
    Error { message: String },
    SyncComplete,
}

/// The main sync engine
pub struct SyncEngine {
    account_id: String,
    user_email: String,
    account_config: AccountConfig,
    config: SyncConfig,
    db: Arc<SyncDatabase>,
    action_queue: Arc<ActionQueue>,
    conversation_grouper: Arc<ConversationGrouper>,
    classifier: Arc<MessageClassifier>,
    status: Arc<RwLock<SyncStatus>>,
    is_online: Arc<AtomicBool>,
    event_tx: Sender<SyncEvent>,
    shutdown: Arc<AtomicBool>,
}

impl SyncEngine {
    /// Create a new sync engine
    pub fn new(
        account_id: String,
        user_email: String,
        account_config: AccountConfig,
        config: SyncConfig,
    ) -> Result<(Self, Receiver<SyncEvent>), HimalayaError> {
        // Ensure parent directory exists
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Initialize database
        let db = Arc::new(SyncDatabase::new(&config.db_path)?);
        let action_queue = Arc::new(ActionQueue::new(db.clone()));
        let conversation_grouper = Arc::new(ConversationGrouper::new(db.clone()));
        let classifier = Arc::new(MessageClassifier::new(db.clone()));

        let (event_tx, event_rx) = flume::unbounded();

        let status = SyncStatus {
            state: SyncState::Idle,
            account_id: account_id.clone(),
            current_folder: None,
            progress: None,
            last_sync: None,
            error: None,
            is_online: false,
            pending_actions: 0,
        };

        let engine = Self {
            account_id,
            user_email,
            account_config,
            config,
            db,
            action_queue,
            conversation_grouper,
            classifier,
            status: Arc::new(RwLock::new(status)),
            is_online: Arc::new(AtomicBool::new(false)),
            event_tx,
            shutdown: Arc::new(AtomicBool::new(false)),
        };

        Ok((engine, event_rx))
    }

    /// Get current sync status
    pub async fn status(&self) -> SyncStatus {
        self.status.read().await.clone()
    }

    /// Check if online
    pub fn is_online(&self) -> bool {
        self.is_online.load(Ordering::SeqCst)
    }

    /// Set online status
    pub fn set_online(&self, online: bool) {
        self.is_online.store(online, Ordering::SeqCst);
    }

    /// Get the database
    pub fn database(&self) -> Arc<SyncDatabase> {
        self.db.clone()
    }

    /// Get the action queue
    pub fn action_queue(&self) -> Arc<ActionQueue> {
        self.action_queue.clone()
    }

    /// Get the conversation grouper
    pub fn conversation_grouper(&self) -> Arc<ConversationGrouper> {
        self.conversation_grouper.clone()
    }

    /// Update and broadcast status
    async fn update_status<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut SyncStatus),
    {
        let mut status = self.status.write().await;
        update_fn(&mut status);
        let _ = self.event_tx.send(SyncEvent::StatusChanged(status.clone()));
    }

    /// Create an EmailBackend for this account
    async fn create_backend(&self) -> Result<EmailBackend, HimalayaError> {
        EmailBackend::new(&self.account_id).await
    }

    /// Perform a full sync - fetches all messages and rebuilds cache
    pub async fn full_sync(&self) -> Result<SyncResult, HimalayaError> {
        info!("Starting full sync for account: {}", self.account_id);

        self.update_status(|s| {
            s.state = SyncState::Syncing;
            s.error = None;
        })
        .await;

        let result = self.do_full_sync().await;

        match &result {
            Ok(sync_result) => {
                info!(
                    "Full sync completed: {} new messages, {} conversations affected",
                    sync_result.new_messages,
                    sync_result.affected_conversations.len()
                );

                self.update_status(|s| {
                    s.state = SyncState::Idle;
                    s.current_folder = None;
                    s.last_sync = Some(Utc::now());
                    s.error = None;
                    s.is_online = true;
                })
                .await;

                self.set_online(true);
                let _ = self.event_tx.send(SyncEvent::SyncComplete);
            }
            Err(e) => {
                error!("Full sync failed: {}", e);

                self.update_status(|s| {
                    s.state = SyncState::Error;
                    s.error = Some(e.to_string());
                })
                .await;
            }
        }

        result
    }

    /// Internal full sync implementation
    async fn do_full_sync(&self) -> Result<SyncResult, HimalayaError> {
        let backend = self.create_backend().await?;

        // Get folders to sync
        let folders_to_sync = self.get_folders_to_sync(&backend).await?;
        info!("Will sync folders: {:?}", folders_to_sync);

        let mut total_new = 0u32;
        let mut all_message_ids: Vec<i64> = Vec::new();

        // Sync each folder
        for (idx, folder) in folders_to_sync.iter().enumerate() {
            self.update_status(|s| {
                s.current_folder = Some(folder.clone());
                s.progress = Some(SyncProgressInfo {
                    phase: "syncing".to_string(),
                    current: idx as u32,
                    total: Some(folders_to_sync.len() as u32),
                    message: format!("Syncing {}...", folder),
                });
            })
            .await;

            match self.sync_folder_from_imap(&backend, folder).await {
                Ok(message_ids) => {
                    total_new += message_ids.len() as u32;
                    all_message_ids.extend(message_ids);
                }
                Err(e) => {
                    warn!("Failed to sync folder {}: {}", folder, e);
                }
            }
        }

        // Rebuild conversations from all cached messages
        self.update_status(|s| {
            s.progress = Some(SyncProgressInfo {
                phase: "conversations".to_string(),
                current: 0,
                total: None,
                message: "Building conversations...".to_string(),
            });
        })
        .await;

        let conv_count = self
            .conversation_grouper
            .rebuild_conversations(&self.account_id, &self.user_email)?;

        info!("Rebuilt {} conversations", conv_count);

        // Get affected conversation IDs
        let conversations = self.db.get_conversations(&self.account_id, true)?;
        let affected_conversations: Vec<i64> = conversations.iter().map(|c| c.id).collect();

        let _ = self.event_tx.send(SyncEvent::ConversationsUpdated {
            conversation_ids: affected_conversations.clone(),
        });

        Ok(SyncResult {
            new_messages: total_new,
            updated_messages: 0,
            deleted_messages: 0,
            affected_conversations,
        })
    }

    /// Get list of folders to sync
    async fn get_folders_to_sync(&self, backend: &EmailBackend) -> Result<Vec<String>, HimalayaError> {
        if !self.config.sync_folders.is_empty() {
            return Ok(self.config.sync_folders.clone());
        }

        // Default: INBOX + Sent folder
        let mut folders = vec!["INBOX".to_string()];

        // Find sent folder
        let all_folders = backend.list_folders().await?;
        for folder in all_folders {
            let name_lower = folder.name.to_lowercase();
            if name_lower.contains("sent")
                || name_lower.contains("envoy")
                || name_lower.contains("gesendet")
                || name_lower.contains("enviados")
                || name_lower.contains("inviati")
            {
                folders.push(folder.name);
                break;
            }
        }

        Ok(folders)
    }

    /// Sync a single folder from IMAP into the database
    async fn sync_folder_from_imap(
        &self,
        backend: &EmailBackend,
        folder: &str,
    ) -> Result<Vec<i64>, HimalayaError> {
        info!("Syncing folder: {}", folder);

        // Fetch envelopes from IMAP
        let envelopes = backend
            .list_envelopes(Some(folder), 0, self.config.fetch_page_size)
            .await?;

        info!("Fetched {} envelopes from {}", envelopes.len(), folder);

        // Filter by date
        let cutoff = Utc::now() - Duration::days(self.config.initial_sync_days as i64);
        let mut message_ids: Vec<i64> = Vec::new();

        for envelope in envelopes {
            // Parse date and filter
            let msg_date = chrono::DateTime::parse_from_rfc3339(&envelope.date)
                .ok()
                .map(|d| d.with_timezone(&Utc));

            if let Some(date) = msg_date {
                if date < cutoff {
                    continue; // Skip old messages
                }
            }

            // Convert envelope to cached message
            let cached_msg = self.envelope_to_cached_message(folder, &envelope)?;

            // Upsert into database
            let msg_id = self.db.upsert_message(&cached_msg)?;
            message_ids.push(msg_id);

            // Classify if enabled
            if self.config.auto_classify {
                if let Ok(Some(msg)) = self.db.get_message_by_id(msg_id) {
                    let _ = self.classifier.classify_and_store(&msg);
                }
            }
        }

        info!("Stored {} messages from {}", message_ids.len(), folder);

        Ok(message_ids)
    }

    /// Convert an Envelope to a CachedMessage
    fn envelope_to_cached_message(
        &self,
        folder: &str,
        envelope: &Envelope,
    ) -> Result<CachedMessage, HimalayaError> {
        // Parse the UID from the envelope ID (assuming it's a string representation)
        let uid: u32 = envelope.id.parse().unwrap_or(0);

        // Parse date
        let date = chrono::DateTime::parse_from_rfc3339(&envelope.date)
            .ok()
            .map(|d| d.with_timezone(&Utc));

        // Extract from name and address
        let (from_name, from_address) = parse_email_address(&envelope.from);

        // Serialize to/flags as JSON
        let to_json = serde_json::to_string(&envelope.to).unwrap_or_else(|_| "[]".to_string());
        let flags_json = serde_json::to_string(&envelope.flags).unwrap_or_else(|_| "[]".to_string());

        Ok(CachedMessage {
            id: 0, // Will be set by database
            account_id: self.account_id.clone(),
            folder_name: folder.to_string(),
            uid,
            message_id: envelope.message_id.clone(),
            in_reply_to: envelope.in_reply_to.clone(),
            references: None,
            from_address,
            from_name,
            to_addresses: to_json,
            cc_addresses: None,
            subject: Some(envelope.subject.clone()),
            date,
            flags: flags_json,
            has_attachment: envelope.has_attachment,
            body_cached: false,
            text_body: None,
            html_body: None,
            raw_size: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Sync a specific folder
    pub async fn sync_folder(&self, folder: &str) -> Result<SyncResult, HimalayaError> {
        info!("Syncing folder: {}", folder);

        self.update_status(|s| {
            s.state = SyncState::Syncing;
            s.current_folder = Some(folder.to_string());
        })
        .await;

        let backend = self.create_backend().await?;
        let message_ids = self.sync_folder_from_imap(&backend, folder).await?;

        // Update conversations for new messages
        let mut affected_conversations: HashSet<i64> = HashSet::new();
        for msg_id in &message_ids {
            if let Ok(Some(message)) = self.db.get_message_by_id(*msg_id) {
                if let Ok(conv_id) = self.conversation_grouper.assign_to_conversation(
                    &self.account_id,
                    &self.user_email,
                    &message,
                ) {
                    affected_conversations.insert(conv_id);
                }
            }
        }

        self.update_status(|s| {
            s.state = SyncState::Idle;
            s.current_folder = None;
            s.last_sync = Some(Utc::now());
        })
        .await;

        let affected: Vec<i64> = affected_conversations.into_iter().collect();

        if !affected.is_empty() {
            let _ = self.event_tx.send(SyncEvent::ConversationsUpdated {
                conversation_ids: affected.clone(),
            });
        }

        let _ = self.event_tx.send(SyncEvent::SyncComplete);

        Ok(SyncResult {
            new_messages: message_ids.len() as u32,
            updated_messages: 0,
            deleted_messages: 0,
            affected_conversations: affected,
        })
    }

    /// Get conversations from cache
    pub fn get_conversations(&self, include_hidden: bool) -> Result<Vec<CachedConversation>, HimalayaError> {
        self.db.get_conversations(&self.account_id, include_hidden)
    }

    /// Get messages for a conversation
    pub fn get_conversation_messages(&self, conversation_id: i64) -> Result<Vec<CachedMessage>, HimalayaError> {
        self.db.get_conversation_messages(conversation_id)
    }

    /// Queue a user action
    pub fn queue_action(&self, action: ActionType) -> Result<i64, HimalayaError> {
        self.action_queue.queue(&self.account_id, action)
    }

    /// Fetch full message body and cache it
    pub async fn fetch_message_body(&self, message_id: i64) -> Result<CachedMessage, HimalayaError> {
        let message = self.db.get_message_by_id(message_id)?
            .ok_or_else(|| HimalayaError::MessageNotFound(message_id.to_string()))?;

        // If already cached, return it
        if message.body_cached {
            return Ok(message);
        }

        // Fetch from IMAP
        let backend = self.create_backend().await?;
        let full_message = backend
            .get_message(Some(&message.folder_name), &message.uid.to_string(), true)
            .await?;

        // Update cache with body
        let updated = CachedMessage {
            body_cached: true,
            text_body: full_message.text_body,
            html_body: full_message.html_body,
            ..message
        };

        self.db.upsert_message(&updated)?;

        Ok(updated)
    }

    /// Shutdown the sync engine
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

/// Parse an email address string into (name, address)
fn parse_email_address(addr: &str) -> (Option<String>, String) {
    let addr = addr.trim();

    // Handle "Name <email>" format
    if let (Some(name_end), Some(email_start)) = (addr.rfind('<'), addr.rfind('>')) {
        if email_start > name_end {
            let email = addr[name_end + 1..email_start].trim().to_lowercase();
            let name = addr[..name_end].trim().trim_matches('"').trim();
            return (
                if name.is_empty() { None } else { Some(name.to_string()) },
                email,
            );
        }
    }

    // Plain email address
    (None, addr.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_email_address() {
        let (name, email) = parse_email_address("John Doe <john@example.com>");
        assert_eq!(name, Some("John Doe".to_string()));
        assert_eq!(email, "john@example.com");

        let (name, email) = parse_email_address("jane@example.com");
        assert_eq!(name, None);
        assert_eq!(email, "jane@example.com");

        let (name, email) = parse_email_address("\"Jane Doe\" <jane@example.com>");
        assert_eq!(name, Some("Jane Doe".to_string()));
        assert_eq!(email, "jane@example.com");
    }
}
