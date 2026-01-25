//! IMAP Sync Engine
//!
//! Maintains a local SQLite cache of email messages synchronized with IMAP servers.
//! The local database is a cache of server state, not the source of truth.
//!
//! Key principles:
//! - UI renders exclusively from SQLite, never directly from IMAP responses
//! - All user actions execute on the IMAP/SMTP server first
//! - UI updates only after the next sync confirms the server state changed
//! - Server wins all conflicts

use chrono::{DateTime, Duration, Utc};
use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::sync::action_queue::{ActionQueue, ActionType, QueuedAction, ReplayResult};
use crate::sync::capability::{CapabilityInfo, ServerCapability};
use crate::sync::classifier::MessageClassifier;
use crate::sync::conversation::ConversationGrouper;
use crate::sync::db::{
    CachedConversation, CachedMessage, FolderSyncState, SyncDatabase, SyncProgress,
};
use crate::sync::idle::{IdleConfig, QuickCheckState};
use crate::types::error::HimalayaError;

/// Sync engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Database file path
    pub db_path: PathBuf,
    /// Initial sync: number of days to fetch immediately
    pub initial_sync_days: u32,
    /// Initial sync: batch size for background sync
    pub background_sync_batch_size: u32,
    /// Maximum message age to keep in cache (days)
    pub max_cache_age_days: u32,
    /// IDLE/polling configuration
    pub idle_config: IdleConfig,
    /// Auto-classify messages
    pub auto_classify: bool,
    /// Folders to sync (empty = INBOX only)
    pub sync_folders: Vec<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("eddie_sync.db"),
            initial_sync_days: 30,
            background_sync_batch_size: 500,
            max_cache_age_days: 365,
            idle_config: IdleConfig::default(),
            auto_classify: true,
            sync_folders: vec![], // Empty means INBOX + Sent
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
    BackgroundSync,
    ReplayingActions,
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
    /// Sync status changed
    StatusChanged(SyncStatus),
    /// New messages arrived
    NewMessages { folder: String, count: u32 },
    /// Messages deleted
    MessagesDeleted { folder: String, uids: Vec<u32> },
    /// Flags changed
    FlagsChanged { folder: String, uids: Vec<u32> },
    /// Conversations updated
    ConversationsUpdated { conversation_ids: Vec<i64> },
    /// Error occurred
    Error { message: String },
    /// Online status changed
    OnlineStatusChanged { is_online: bool },
}

/// The main sync engine
pub struct SyncEngine {
    account_id: String,
    user_email: String,
    config: SyncConfig,
    db: Arc<SyncDatabase>,
    action_queue: Arc<ActionQueue>,
    conversation_grouper: Arc<ConversationGrouper>,
    classifier: Arc<MessageClassifier>,
    capabilities: Arc<RwLock<Option<CapabilityInfo>>>,
    status: Arc<RwLock<SyncStatus>>,
    is_online: Arc<AtomicBool>,
    event_tx: Sender<SyncEvent>,
    shutdown: Arc<AtomicBool>,
    quick_check_state: Arc<RwLock<HashMap<String, QuickCheckState>>>,
}

impl SyncEngine {
    /// Create a new sync engine
    pub fn new(
        account_id: String,
        user_email: String,
        config: SyncConfig,
    ) -> Result<(Self, Receiver<SyncEvent>), HimalayaError> {
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
            config,
            db,
            action_queue,
            conversation_grouper,
            classifier,
            capabilities: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(status)),
            is_online: Arc::new(AtomicBool::new(false)),
            event_tx,
            shutdown: Arc::new(AtomicBool::new(false)),
            quick_check_state: Arc::new(RwLock::new(HashMap::new())),
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
        let was_online = self.is_online.swap(online, Ordering::SeqCst);
        if was_online != online {
            let _ = self.event_tx.send(SyncEvent::OnlineStatusChanged { is_online: online });
        }
    }

    /// Get the database
    pub fn database(&self) -> Arc<SyncDatabase> {
        self.db.clone()
    }

    /// Get the action queue
    pub fn action_queue(&self) -> Arc<ActionQueue> {
        self.action_queue.clone()
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

    /// Set server capabilities
    pub async fn set_capabilities(&self, caps: CapabilityInfo) {
        // Store in database
        let _ = self.db.store_capabilities(
            &self.account_id,
            &caps.raw_capabilities,
            caps.sync_capability == ServerCapability::Qresync,
            caps.sync_capability == ServerCapability::Condstore ||
                caps.sync_capability == ServerCapability::Qresync,
            caps.supports_idle,
        );

        let mut capabilities = self.capabilities.write().await;
        *capabilities = Some(caps);
    }

    /// Get cached capabilities
    pub async fn capabilities(&self) -> Option<CapabilityInfo> {
        let caps = self.capabilities.read().await;
        caps.clone()
    }

    /// Perform a full sync for a folder
    pub async fn sync_folder(&self, folder: &str) -> Result<SyncResult, HimalayaError> {
        info!("Starting sync for folder: {}", folder);

        self.update_status(|s| {
            s.state = SyncState::Syncing;
            s.current_folder = Some(folder.to_string());
        }).await;

        let result = self.do_sync_folder(folder).await;

        match &result {
            Ok(sync_result) => {
                info!(
                    "Sync completed: {} new, {} updated, {} deleted",
                    sync_result.new_messages,
                    sync_result.updated_messages,
                    sync_result.deleted_messages
                );

                self.update_status(|s| {
                    s.state = SyncState::Idle;
                    s.current_folder = None;
                    s.last_sync = Some(Utc::now());
                    s.error = None;
                }).await;
            }
            Err(e) => {
                error!("Sync failed: {}", e);

                self.update_status(|s| {
                    s.state = SyncState::Error;
                    s.error = Some(e.to_string());
                }).await;
            }
        }

        result
    }

    /// Internal sync implementation
    async fn do_sync_folder(&self, folder: &str) -> Result<SyncResult, HimalayaError> {
        let caps = self.capabilities.read().await.clone()
            .unwrap_or_else(|| CapabilityInfo::default());

        // Step 1: Check UIDVALIDITY
        let sync_state = self.check_uidvalidity(folder).await?;

        // Step 2: Replay pending actions
        self.replay_pending_actions(folder).await?;

        // Step 3-5: Sync based on capability level
        let result = match caps.sync_capability {
            ServerCapability::Qresync => {
                self.sync_with_qresync(folder, &sync_state).await?
            }
            ServerCapability::Condstore => {
                self.sync_with_condstore(folder, &sync_state).await?
            }
            ServerCapability::Bare => {
                self.sync_bare_imap(folder, &sync_state).await?
            }
        };

        // Step 6: Update sync state
        self.update_sync_state(folder, &result).await?;

        // Step 7: Recompute conversations
        let affected_conversations = self.update_conversations(&result).await?;

        // Step 8: Notify UI
        if !affected_conversations.is_empty() {
            let _ = self.event_tx.send(SyncEvent::ConversationsUpdated {
                conversation_ids: affected_conversations.clone(),
            });
        }

        Ok(SyncResult {
            new_messages: result.new_messages,
            updated_messages: result.updated_messages,
            deleted_messages: result.deleted_messages,
            affected_conversations,
        })
    }

    /// Check UIDVALIDITY and invalidate cache if changed
    async fn check_uidvalidity(&self, folder: &str) -> Result<FolderSyncState, HimalayaError> {
        let existing = self.db.get_folder_sync_state(&self.account_id, folder)?;

        if let Some(state) = existing {
            // UIDVALIDITY will be checked when we get server state
            // If it changes, we'll purge the cache
            Ok(state)
        } else {
            // No existing state - create initial
            let state = FolderSyncState {
                account_id: self.account_id.clone(),
                folder_name: folder.to_string(),
                uidvalidity: None,
                highestmodseq: None,
                last_seen_uid: None,
                last_sync_timestamp: None,
                sync_in_progress: true,
            };
            self.db.upsert_folder_sync_state(&state)?;
            Ok(state)
        }
    }

    /// Handle UIDVALIDITY change
    pub async fn handle_uidvalidity_change(&self, folder: &str, _new_uidvalidity: u32) -> Result<(), HimalayaError> {
        warn!("UIDVALIDITY changed for {}, invalidating cache", folder);

        // Purge entire folder cache
        self.db.invalidate_folder_cache(&self.account_id, folder)?;

        // Notify UI that conversations may have changed
        let _ = self.event_tx.send(SyncEvent::ConversationsUpdated {
            conversation_ids: vec![], // Full refresh needed
        });

        Ok(())
    }

    /// Replay pending actions to server
    async fn replay_pending_actions(&self, _folder: &str) -> Result<(), HimalayaError> {
        let pending = self.action_queue.get_pending(&self.account_id)?;

        if pending.is_empty() {
            return Ok(());
        }

        self.update_status(|s| {
            s.state = SyncState::ReplayingActions;
            s.pending_actions = pending.len() as u32;
        }).await;

        for action in pending {
            if let Some(id) = action.id {
                self.action_queue.mark_processing(id)?;

                // Here we would actually execute the action against IMAP
                // For now, we'll mark as completed (actual implementation would call backend)
                let result = self.execute_action(&action).await;

                match result {
                    ReplayResult::Success => {
                        self.action_queue.mark_completed(id)?;
                    }
                    ReplayResult::Retry(error) => {
                        if self.action_queue.should_retry(&action) {
                            self.action_queue.retry(id)?;
                        } else {
                            self.action_queue.mark_failed(id, &error)?;
                        }
                    }
                    ReplayResult::Discard(reason) => {
                        info!("Discarding action: {}", reason);
                        self.action_queue.mark_completed(id)?;
                    }
                }
            }
        }

        // Clean up completed actions
        self.action_queue.cleanup_completed(&self.account_id)?;

        self.update_status(|s| {
            s.pending_actions = 0;
        }).await;

        Ok(())
    }

    /// Execute a queued action (placeholder - needs IMAP integration)
    async fn execute_action(&self, action: &QueuedAction) -> ReplayResult {
        // This would integrate with the EmailBackend to execute the action
        // For now, return success as a placeholder
        debug!("Executing action: {:?}", action.action);
        ReplayResult::Success
    }

    /// Sync using QRESYNC (best performance)
    async fn sync_with_qresync(
        &self,
        folder: &str,
        sync_state: &FolderSyncState,
    ) -> Result<InternalSyncResult, HimalayaError> {
        info!("Using QRESYNC sync strategy for {}", folder);

        // With QRESYNC, SELECT...QRESYNC gives us:
        // - Flag changes (FETCH responses with FLAGS)
        // - Deletions (VANISHED responses)
        // In one round-trip!

        // This is a placeholder - actual implementation would:
        // 1. SELECT folder with QRESYNC (UIDVALIDITY HIGHESTMODSEQ message-set)
        // 2. Parse VANISHED responses for deleted UIDs
        // 3. Parse FETCH responses for flag changes
        // 4. Fetch new messages (UID > last_seen_uid)

        // For now, fall back to CONDSTORE behavior
        self.sync_with_condstore(folder, sync_state).await
    }

    /// Sync using CONDSTORE
    async fn sync_with_condstore(
        &self,
        folder: &str,
        sync_state: &FolderSyncState,
    ) -> Result<InternalSyncResult, HimalayaError> {
        info!("Using CONDSTORE sync strategy for {}", folder);

        // Step 1: Fetch new messages
        let new_messages = self.fetch_new_messages(folder, sync_state.last_seen_uid).await?;

        // Step 2: Detect flag changes using CHANGEDSINCE
        let flag_changes = if let Some(modseq) = sync_state.highestmodseq {
            self.fetch_flag_changes(folder, modseq).await?
        } else {
            Vec::new()
        };

        // Step 3: Detect deletions via UID SEARCH
        let deletions = self.detect_deletions(folder, sync_state.last_seen_uid).await?;

        Ok(InternalSyncResult {
            new_messages: new_messages.len() as u32,
            updated_messages: flag_changes.len() as u32,
            deleted_messages: deletions.len() as u32,
            new_message_ids: new_messages.iter().map(|m| m.id).collect(),
            updated_message_ids: flag_changes,
            deleted_uids: deletions,
        })
    }

    /// Sync using bare IMAP (full comparison)
    async fn sync_bare_imap(
        &self,
        folder: &str,
        sync_state: &FolderSyncState,
    ) -> Result<InternalSyncResult, HimalayaError> {
        info!("Using bare IMAP sync strategy for {}", folder);

        // Step 1: Fetch new messages
        let new_messages = self.fetch_new_messages(folder, sync_state.last_seen_uid).await?;

        // Step 2: Full flag comparison
        let flag_changes = self.full_flag_comparison(folder).await?;

        // Step 3: Detect deletions via UID SEARCH
        let deletions = self.detect_deletions(folder, sync_state.last_seen_uid).await?;

        Ok(InternalSyncResult {
            new_messages: new_messages.len() as u32,
            updated_messages: flag_changes.len() as u32,
            deleted_messages: deletions.len() as u32,
            new_message_ids: new_messages.iter().map(|m| m.id).collect(),
            updated_message_ids: flag_changes,
            deleted_uids: deletions,
        })
    }

    /// Fetch new messages (UID > last_seen_uid)
    async fn fetch_new_messages(
        &self,
        _folder: &str,
        _last_seen_uid: Option<u32>,
    ) -> Result<Vec<CachedMessage>, HimalayaError> {
        // This is a placeholder - actual implementation would:
        // 1. UID FETCH <last_seen_uid+1>:* (ENVELOPE FLAGS BODYSTRUCTURE)
        // 2. Parse responses into CachedMessage
        // 3. Store in database

        // For now, return empty (integration with EmailBackend needed)
        Ok(Vec::new())
    }

    /// Fetch flag changes using CHANGEDSINCE
    async fn fetch_flag_changes(
        &self,
        _folder: &str,
        _since_modseq: u64,
    ) -> Result<Vec<i64>, HimalayaError> {
        // This is a placeholder - actual implementation would:
        // 1. UID FETCH 1:* (FLAGS) (CHANGEDSINCE <modseq>)
        // 2. Update flags in database
        // 3. Return IDs of updated messages

        Ok(Vec::new())
    }

    /// Full flag comparison against cache
    async fn full_flag_comparison(&self, _folder: &str) -> Result<Vec<i64>, HimalayaError> {
        // This is a placeholder - actual implementation would:
        // 1. UID FETCH 1:* FLAGS
        // 2. Compare with cached flags
        // 3. Update database for differences
        // 4. Return IDs of updated messages

        Ok(Vec::new())
    }

    /// Detect deleted messages
    async fn detect_deletions(
        &self,
        folder: &str,
        _last_seen_uid: Option<u32>,
    ) -> Result<Vec<u32>, HimalayaError> {
        // Get cached UIDs
        let cached_uids = self.db.get_folder_uids(&self.account_id, folder)?;

        if cached_uids.is_empty() {
            return Ok(Vec::new());
        }

        // This is a placeholder - actual implementation would:
        // 1. UID SEARCH UID 1:<max_uid>
        // 2. Compare with cached UIDs
        // 3. Delete missing UIDs from database

        // For now, return empty
        Ok(Vec::new())
    }

    /// Update sync state after successful sync
    async fn update_sync_state(
        &self,
        folder: &str,
        _result: &InternalSyncResult,
    ) -> Result<(), HimalayaError> {
        let mut state = self.db.get_folder_sync_state(&self.account_id, folder)?
            .unwrap_or(FolderSyncState {
                account_id: self.account_id.clone(),
                folder_name: folder.to_string(),
                uidvalidity: None,
                highestmodseq: None,
                last_seen_uid: None,
                last_sync_timestamp: None,
                sync_in_progress: false,
            });

        state.last_sync_timestamp = Some(Utc::now());
        state.sync_in_progress = false;

        // Update would include new UIDVALIDITY, HIGHESTMODSEQ, etc. from server response

        self.db.upsert_folder_sync_state(&state)?;

        Ok(())
    }

    /// Update conversations for synced messages
    async fn update_conversations(
        &self,
        result: &InternalSyncResult,
    ) -> Result<Vec<i64>, HimalayaError> {
        let mut affected: HashSet<i64> = HashSet::new();

        // Process new messages
        for msg_id in &result.new_message_ids {
            if let Some(message) = self.db.get_message_by_id(*msg_id)? {
                // Classify if enabled
                if self.config.auto_classify {
                    let _ = self.classifier.classify_and_store(&message);
                }

                // Assign to conversation
                if let Ok(conv_id) = self.conversation_grouper.assign_to_conversation(
                    &self.account_id,
                    &self.user_email,
                    &message,
                ) {
                    affected.insert(conv_id);
                }
            }
        }

        // Process deleted messages
        for _uid in &result.deleted_uids {
            // Conversation links are auto-deleted via CASCADE
            // We'll clean up empty conversations
        }

        // Clean up empty conversations
        self.db.delete_empty_conversations(&self.account_id)?;

        Ok(affected.into_iter().collect())
    }

    /// Perform initial sync for a new account or folder
    pub async fn initial_sync(&self, folder: &str) -> Result<(), HimalayaError> {
        info!("Starting initial sync for {}", folder);

        self.update_status(|s| {
            s.state = SyncState::InitialSync;
            s.current_folder = Some(folder.to_string());
            s.progress = Some(SyncProgressInfo {
                phase: "initial".to_string(),
                current: 0,
                total: None,
                message: "Fetching recent messages...".to_string(),
            });
        }).await;

        // Phase 1: Fetch last N days immediately
        let cutoff_date = Utc::now() - Duration::days(self.config.initial_sync_days as i64);
        self.sync_messages_since(folder, cutoff_date).await?;

        // Phase 2: Background sync older messages
        self.start_background_sync(folder).await?;

        self.update_status(|s| {
            s.state = SyncState::Idle;
            s.progress = None;
        }).await;

        Ok(())
    }

    /// Sync messages since a date
    async fn sync_messages_since(
        &self,
        _folder: &str,
        _since: DateTime<Utc>,
    ) -> Result<u32, HimalayaError> {
        // Placeholder - actual implementation would:
        // 1. SEARCH SINCE <date>
        // 2. Fetch matching messages
        // 3. Store in database

        Ok(0)
    }

    /// Start background sync for older messages
    async fn start_background_sync(&self, folder: &str) -> Result<(), HimalayaError> {
        // Update progress tracking
        let progress = SyncProgress {
            account_id: self.account_id.clone(),
            folder_name: folder.to_string(),
            phase: "background".to_string(),
            total_messages: None,
            synced_messages: 0,
            oldest_synced_date: None,
            last_batch_uid: None,
            started_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.update_sync_progress(&progress)?;

        // Background sync would run in a separate task
        // Fetching messages in batches of background_sync_batch_size

        Ok(())
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

    /// Perform quick check optimization
    pub async fn quick_check(&self, folder: &str, current: QuickCheckState) -> bool {
        let states = self.quick_check_state.read().await;
        if let Some(cached) = states.get(folder) {
            !cached.changed(&current)
        } else {
            false // No cached state, need full sync
        }
    }

    /// Update quick check state
    pub async fn update_quick_check_state(&self, folder: &str, state: QuickCheckState) {
        let mut states = self.quick_check_state.write().await;
        states.insert(folder.to_string(), state);
    }

    /// Shutdown the sync engine
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Check if shutdown requested
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
}

/// Internal sync result (before conversation processing)
struct InternalSyncResult {
    new_messages: u32,
    updated_messages: u32,
    deleted_messages: u32,
    new_message_ids: Vec<i64>,
    updated_message_ids: Vec<i64>,
    deleted_uids: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sync_engine_creation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let config = SyncConfig {
            db_path,
            ..Default::default()
        };

        let result = SyncEngine::new(
            "test@example.com".to_string(),
            "test@example.com".to_string(),
            config,
        );

        assert!(result.is_ok());
        let (engine, _rx) = result.unwrap();
        assert!(!engine.is_online());
    }

    #[tokio::test]
    async fn test_status_updates() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let config = SyncConfig {
            db_path,
            ..Default::default()
        };

        let (engine, rx) = SyncEngine::new(
            "test@example.com".to_string(),
            "test@example.com".to_string(),
            config,
        ).unwrap();

        engine.update_status(|s| {
            s.state = SyncState::Syncing;
        }).await;

        let status = engine.status().await;
        assert_eq!(status.state, SyncState::Syncing);

        // Check event was emitted
        let event = rx.try_recv();
        assert!(event.is_ok());
    }
}
