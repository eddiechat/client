//! IMAP Sync Engine
//!
//! Maintains a local SQLite cache of email messages synchronized with IMAP servers.
//! The local database is a cache of server state, not the source of truth.
//!
//! Features:
//! - Background monitoring via polling (IDLE support ready for future)
//! - Automatic sync on detected changes
//! - Offline action queue with replay

use chrono::{DateTime, Duration, Utc};
use flume::Receiver;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::backend::EmailBackend;
use crate::config::EmailAccountConfig;
use crate::sync::action_queue::{ActionQueue, ActionType, ReplayResult};
use crate::sync::classifier::MessageClassifier;
use crate::sync::conversation::ConversationGrouper;
use crate::sync::db::{CachedConversation, CachedChatMessage, SyncDatabase};
use crate::sync::idle::{ChangeNotification, MailboxMonitor, MonitorConfig, MonitorMode};
use crate::types::error::EddieError;
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
    /// Enable background monitoring for changes
    pub enable_monitoring: bool,
    /// Monitoring configuration
    pub monitor_config: MonitorConfig,
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
            enable_monitoring: true,
            monitor_config: MonitorConfig::default(),
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
    pub monitor_mode: Option<String>,
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
    SyncComplete {},
}

/// The main sync engine
pub struct SyncEngine {
    account_id: String,
    user_email: String,
    user_aliases: Vec<String>, // List of email aliases for this account
    account_config: EmailAccountConfig,
    config: SyncConfig,
    db: Arc<SyncDatabase>,
    action_queue: Arc<ActionQueue>,
    conversation_grouper: Arc<ConversationGrouper>,
    classifier: Arc<MessageClassifier>,
    status: Arc<RwLock<SyncStatus>>,
    is_online: Arc<AtomicBool>,
    app_handle: Option<tauri::AppHandle>,
    shutdown: Arc<AtomicBool>,
    /// Mailbox monitor for detecting changes
    monitor: Option<Arc<MailboxMonitor>>,
    /// Receiver for monitor notifications
    monitor_rx: Option<Receiver<ChangeNotification>>,
}

impl SyncEngine {
    /// Create a new sync engine
    pub fn new(
        account_id: String,
        user_email: String,
        user_aliases: Vec<String>,
        account_config: EmailAccountConfig,
        config: SyncConfig,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<Self, EddieError> {
        // Ensure parent directory exists
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Initialize database
        let db = Arc::new(SyncDatabase::new(&config.db_path)?);
        let action_queue = Arc::new(ActionQueue::new(db.clone()));
        let conversation_grouper = Arc::new(ConversationGrouper::new(db.clone()));
        let classifier = Arc::new(MessageClassifier::new(db.clone()));

        let status = SyncStatus {
            state: SyncState::Idle,
            account_id: account_id.clone(),
            current_folder: None,
            progress: None,
            last_sync: None,
            error: None,
            is_online: false,
            pending_actions: 0,
            monitor_mode: None,
        };

        let engine = Self {
            account_id,
            user_email,
            user_aliases,
            account_config,
            config,
            db,
            action_queue,
            conversation_grouper,
            classifier,
            status: Arc::new(RwLock::new(status)),
            is_online: Arc::new(AtomicBool::new(false)),
            app_handle,
            shutdown: Arc::new(AtomicBool::new(false)),
            monitor: None,
            monitor_rx: None,
        };

        Ok(engine)
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
        self.emit_event(SyncEvent::StatusChanged(status.clone()));
    }

    /// Emit an event to the frontend via Tauri
    fn emit_event(&self, event: SyncEvent) {
        if let Some(handle) = &self.app_handle {
            if let Err(e) = handle.emit("sync-event", &event) {
                warn!("Failed to emit sync event: {}", e);
            }
        }
    }

    /// Create an EmailBackend for this account
    async fn create_backend(&self) -> Result<EmailBackend, EddieError> {
        EmailBackend::new(&self.account_id).await
    }

    /// Perform a full sync - fetches all messages and rebuilds cache
    pub async fn full_sync(&self) -> Result<SyncResult, EddieError> {
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
                self.emit_event(SyncEvent::SyncComplete {});
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
    async fn do_full_sync(&self) -> Result<SyncResult, EddieError> {
        let backend = self.create_backend().await?;

        // Replay any pending offline actions first
        self.update_status(|s| {
            s.progress = Some(SyncProgressInfo {
                phase: "replaying".to_string(),
                current: 0,
                total: None,
                message: "Replaying pending actions...".to_string(),
            });
        })
        .await;

        match self.action_queue.replay_pending(&self.account_id, &backend).await {
            Ok(results) => {
                let success_count = results.iter().filter(|r| matches!(r, ReplayResult::Success)).count();
                let retry_count = results.iter().filter(|r| matches!(r, ReplayResult::Retry(_))).count();
                let discard_count = results.iter().filter(|r| matches!(r, ReplayResult::Discard(_))).count();
                info!(
                    "Replayed {} actions: {} success, {} retry, {} discarded",
                    results.len(), success_count, retry_count, discard_count
                );
            }
            Err(e) => {
                warn!("Failed to replay pending actions: {}", e);
                // Continue with sync even if replay fails
            }
        }

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
            .rebuild_conversations(&self.account_id, &self.user_email, &self.user_aliases)?;

        info!("Rebuilt {} conversations", conv_count);

        // Update conversation classifications based on existing message classifications
        self.db.update_conversation_classifications(&self.account_id)?;

        // Get affected conversation IDs (all conversations)
        let conversations = self.db.get_conversations(&self.account_id, None)?;
        let affected_conversations: Vec<i64> = conversations.iter().map(|c| c.id).collect();

        self.emit_event(SyncEvent::ConversationsUpdated {
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
    async fn get_folders_to_sync(
        &self,
        backend: &EmailBackend,
    ) -> Result<Vec<String>, EddieError> {
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
    ) -> Result<Vec<i64>, EddieError> {
        info!("Syncing folder: {}", folder);

        // Check UIDVALIDITY before processing envelopes
        // TODO: The EmailBackend does not currently expose UIDVALIDITY from the IMAP server.
        // When the backend is updated to provide UIDVALIDITY (e.g., via a get_folder_status method),
        // uncomment and update the following code:
        //
        // if let Some(server_uidvalidity) = backend.get_folder_uidvalidity(folder).await? {
        //     self.check_uidvalidity(folder, server_uidvalidity)?;
        // }
        //
        // For now, the UIDVALIDITY checking infrastructure is in place and ready to use
        // once the backend provides this information.

        // Fetch envelopes from IMAP
        let envelopes = backend
            .list_envelopes(Some(folder), 0, self.config.fetch_page_size)
            .await?;

        info!("Fetched {} envelopes from {}", envelopes.len(), folder);

        // Filter by date
        let cutoff = Utc::now() - Duration::days(self.config.initial_sync_days as i64);
        // Epoch date indicates parsing failure - don't filter these out
        let epoch = chrono::DateTime::from_timestamp(0, 0).unwrap().with_timezone(&Utc);
        let mut message_ids: Vec<i64> = Vec::new();

        for envelope in envelopes {
            // Parse date and filter
            let msg_date = chrono::DateTime::parse_from_rfc3339(&envelope.date)
                .ok()
                .map(|d| d.with_timezone(&Utc));

            if let Some(date) = msg_date {
                // Don't filter out epoch dates (indicates date parsing failure upstream)
                if date != epoch && date < cutoff {
                    continue; // Skip old messages
                }
            }

            // Convert envelope to cached message
            let cached_msg = self.envelope_to_cached_message(folder, &envelope)?;

            // Upsert into database
            let msg_id = self.db.upsert_message(&cached_msg)?;
            message_ids.push(msg_id);

            // Extract entities (participants) from the message
            extract_entities_from_message(&self.db, &self.account_id, &self.user_email, &self.user_aliases, &cached_msg);

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

    /// Check UIDVALIDITY and invalidate cache if it has changed.
    ///
    /// IMAP UIDs are only valid within a UIDVALIDITY epoch. If UIDVALIDITY changes
    /// (e.g., after mailbox reconstruction), all cached UIDs are invalid and must
    /// be discarded.
    ///
    /// Returns Ok(true) if the cache was invalidated, Ok(false) if UIDVALIDITY matches.
    fn check_uidvalidity(&self, folder: &str, server_uidvalidity: u32) -> Result<bool, EddieError> {
        let stored_uidvalidity = self.db.get_folder_uidvalidity(&self.account_id, folder)?;

        match stored_uidvalidity {
            Some(stored) if stored != server_uidvalidity => {
                // UIDVALIDITY changed - all cached UIDs are now invalid
                warn!(
                    "UIDVALIDITY changed for folder '{}': {} -> {}. Invalidating cache.",
                    folder, stored, server_uidvalidity
                );
                self.db.invalidate_folder_cache(&self.account_id, folder)?;
                // Store the new UIDVALIDITY
                self.db.set_folder_uidvalidity(&self.account_id, folder, server_uidvalidity)?;
                Ok(true)
            }
            Some(_) => {
                // UIDVALIDITY matches - cache is still valid
                Ok(false)
            }
            None => {
                // First sync for this folder - store the UIDVALIDITY
                info!(
                    "Storing initial UIDVALIDITY {} for folder '{}'",
                    server_uidvalidity, folder
                );
                self.db.set_folder_uidvalidity(&self.account_id, folder, server_uidvalidity)?;
                Ok(false)
            }
        }
    }

    /// Convert an Envelope to a CachedChatMessage
    fn envelope_to_cached_message(
        &self,
        folder: &str,
        envelope: &Envelope,
    ) -> Result<CachedChatMessage, EddieError> {
        // Parse the UID from the envelope ID (assuming it's a string representation)
        let uid: u32 = envelope.id.parse().unwrap_or(0);

        // Parse date
        let date = chrono::DateTime::parse_from_rfc3339(&envelope.date)
            .ok()
            .map(|d| d.with_timezone(&Utc));

        // Extract from name and address
        let (from_name, from_address) = parse_email_address(&envelope.from);

        // Serialize to/cc/flags as JSON
        let to_json = serde_json::to_string(&envelope.to).unwrap_or_else(|_| "[]".to_string());
        let cc_json = if envelope.cc.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&envelope.cc).unwrap_or_else(|_| "[]".to_string()))
        };
        let flags_json =
            serde_json::to_string(&envelope.flags).unwrap_or_else(|_| "[]".to_string());

        Ok(CachedChatMessage {
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
            cc_addresses: cc_json,
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
    pub async fn sync_folder(&self, folder: &str) -> Result<SyncResult, EddieError> {
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
                    &self.user_aliases,
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
            self.emit_event(SyncEvent::ConversationsUpdated {
                conversation_ids: affected.clone(),
            });
        }

        self.emit_event(SyncEvent::SyncComplete {});

        Ok(SyncResult {
            new_messages: message_ids.len() as u32,
            updated_messages: 0,
            deleted_messages: 0,
            affected_conversations: affected,
        })
    }

    /// Get conversations from cache
    pub fn get_conversations(
        &self,
        classification_filter: Option<&str>,
    ) -> Result<Vec<CachedConversation>, EddieError> {
        self.db.get_conversations(&self.account_id, classification_filter)
    }

    /// Get conversations from cache with connection filtering
    ///
    /// connection_filter options:
    /// - None: No connection filtering
    /// - Some("connections"): Only conversations where at least one participant is a connection
    /// - Some("others"): Only conversations where NO participants are connections
    pub fn get_conversations_with_connection_filter(
        &self,
        classification_filter: Option<&str>,
        connection_filter: Option<&str>,
    ) -> Result<Vec<CachedConversation>, EddieError> {
        self.db.get_conversations_with_connection_filter(
            &self.account_id,
            classification_filter,
            connection_filter,
        )
    }

    /// Get messages for a conversation
    pub fn get_conversation_messages(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<CachedChatMessage>, EddieError> {
        self.db.get_conversation_messages(conversation_id)
    }

    /// Rebuild all conversations from cached messages
    ///
    /// This regenerates conversation participant keys from all cached messages,
    /// which includes CC addresses. Useful after enabling CC support.
    pub fn rebuild_all_conversations(
        &self,
        account_id: &str,
        user_email: &str,
    ) -> Result<u32, EddieError> {
        self.conversation_grouper.rebuild_conversations(account_id, user_email, &self.user_aliases)
    }

    /// Queue a user action
    pub fn queue_action(&self, action: ActionType) -> Result<i64, EddieError> {
        self.action_queue.queue(&self.account_id, action)
    }

    /// Search entities for autocomplete suggestions
    /// Returns up to `limit` entities matching the query, prioritizing connections and recent contacts
    pub fn search_entities(&self, query: &str, limit: u32) -> Result<Vec<crate::sync::db::Entity>, EddieError> {
        self.db.search_entities(&self.account_id, query, limit)
    }

    /// Reprocess all messages to update entity connections based on current aliases
    ///
    /// This should be called after aliases are added/updated to retroactively mark
    /// recipients as connections if the user (or their aliases) have sent messages to them.
    pub fn reprocess_entities_for_aliases(&self) -> Result<u32, EddieError> {
        info!("Reprocessing entities for account: {}", self.account_id);
        info!("User email: {}, Aliases: {:?}", self.user_email, self.user_aliases);

        // Get all messages for this account
        let messages = self.db.get_all_messages_for_account(&self.account_id)?;
        info!("Found {} messages to process", messages.len());

        let mut entities_to_mark: std::collections::HashSet<String> = std::collections::HashSet::new();

        for message in messages {
            // Check if message is from user or any alias
            let from_lower = message.from_address.to_lowercase();
            let user_email_lower = self.user_email.to_lowercase();
            let is_from_user = from_lower == user_email_lower
                || self.user_aliases.iter().any(|alias| alias == &from_lower);

            if is_from_user {
                // This is an outgoing message - collect all recipient emails
                let contact_timestamp = message.date.unwrap_or_else(chrono::Utc::now);

                // Process TO addresses
                let to_addresses: Vec<String> = serde_json::from_str(&message.to_addresses).unwrap_or_default();
                for addr in to_addresses {
                    let (name, email) = parse_email_address(&addr);
                    let email_lower = email.to_lowercase();

                    // Skip self and aliases
                    if email_lower == user_email_lower || self.user_aliases.iter().any(|alias| alias == &email_lower) {
                        continue;
                    }

                    entities_to_mark.insert(email_lower.clone());

                    self.db.upsert_entity(
                        &self.account_id,
                        &email,
                        name.as_deref(),
                        true, // Mark as connection
                        contact_timestamp,
                    )?;
                }

                // Process CC addresses
                if let Some(cc_json) = &message.cc_addresses {
                    let cc_addresses: Vec<String> = serde_json::from_str(cc_json).unwrap_or_default();
                    for addr in cc_addresses {
                        let (name, email) = parse_email_address(&addr);
                        let email_lower = email.to_lowercase();

                        // Skip self and aliases
                        if email_lower == user_email_lower || self.user_aliases.iter().any(|alias| alias == &email_lower) {
                            continue;
                        }

                        entities_to_mark.insert(email_lower.clone());

                        self.db.upsert_entity(
                            &self.account_id,
                            &email,
                            name.as_deref(),
                            true, // Mark as connection
                            contact_timestamp,
                        )?;
                    }
                }
            }
        }

        let count = entities_to_mark.len() as u32;
        info!("Marked {} unique entities as connections for account: {}", count, self.account_id);
        info!("Sample entities: {:?}", entities_to_mark.iter().take(5).collect::<Vec<_>>());

        Ok(count)
    }

    /// Reclassify all messages for this account
    ///
    /// This re-runs the classifier on all messages and updates the message_classifications table,
    /// then updates conversation classifications accordingly. This is useful after adding aliases
    /// to ensure messages from aliases are properly classified as 'chat'.
    pub fn reclassify_all_messages(&self) -> Result<u32, EddieError> {
        info!("Reclassifying all messages for account: {}", self.account_id);

        // Get all messages for this account
        let messages = self.db.get_all_messages_for_account(&self.account_id)?;
        let total_messages = messages.len();

        info!("Found {} messages to reclassify", total_messages);

        // Reclassify each message
        let mut reclassified_count = 0u32;
        for message in messages {
            if let Err(e) = self.classifier.classify_and_store(&message) {
                warn!("Failed to reclassify message {}: {}", message.id, e);
            } else {
                reclassified_count += 1;
            }
        }

        info!("Reclassified {} messages", reclassified_count);

        // Update conversation classifications based on the newly classified messages
        self.db.update_conversation_classifications(&self.account_id)?;

        info!("Updated conversation classifications for account: {}", self.account_id);

        Ok(reclassified_count)
    }

    /// Replay pending actions from the action queue
    ///
    /// Executes all pending actions on the IMAP server. This is typically called
    /// at the start of a sync to flush any offline actions before syncing.
    pub async fn replay_actions(&self) -> Result<Vec<ReplayResult>, EddieError> {
        info!("Replaying pending actions for account: {}", self.account_id);

        let backend = self.create_backend().await?;
        let results = self.action_queue.replay_pending(&self.account_id, &backend).await?;

        // Update pending actions count in status
        let pending_count = self.action_queue.get_pending(&self.account_id)?.len() as u32;
        self.update_status(|s| {
            s.pending_actions = pending_count;
        }).await;

        Ok(results)
    }

    /// Fetch full message body and cache it
    pub async fn fetch_message_body(
        &self,
        message_id: i64,
    ) -> Result<CachedChatMessage, EddieError> {
        let message = self
            .db
            .get_message_by_id(message_id)?
            .ok_or_else(|| EddieError::MessageNotFound(message_id.to_string()))?;

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
        let updated = CachedChatMessage {
            body_cached: true,
            text_body: full_message.text_body,
            html_body: full_message.html_body,
            ..message
        };

        self.db.upsert_message(&updated)?;

        Ok(updated)
    }

    /// Start background monitoring for mailbox changes
    ///
    /// This creates a monitor that polls for changes and triggers syncs when needed.
    /// The monitor runs in a background task and sends notifications via the channel.
    pub async fn start_monitoring(&mut self) -> Result<(), EddieError> {
        if !self.config.enable_monitoring {
            info!("Monitoring disabled in config, skipping");
            return Ok(());
        }

        if self.monitor.is_some() {
            warn!("Monitor already started for account: {}", self.account_id);
            return Ok(());
        }

        info!(
            "Starting mailbox monitoring for account: {} (poll interval: {}s)",
            self.account_id, self.config.monitor_config.poll_interval_seconds
        );

        // Get folders to monitor
        let backend = self.create_backend().await?;
        let folders = self.get_folders_to_sync(&backend).await?;
        info!("Will monitor folders: {:?}", folders);

        // TODO: Detect IDLE capability from server
        // For now, assume no IDLE support until email-lib exposes it
        let supports_idle = false;
        if supports_idle {
            info!("Server supports IDLE - will use push notifications");
        } else {
            info!("Server does not support IDLE - using polling");
        }

        // Create the monitor
        let (monitor, rx) = MailboxMonitor::new(
            self.account_id.clone(),
            folders,
            self.config.monitor_config.clone(),
            supports_idle,
        );

        let monitor = Arc::new(monitor);
        self.monitor = Some(monitor.clone());
        self.monitor_rx = Some(rx);

        // Update status with monitoring mode
        self.update_status(|s| {
            s.monitor_mode = Some("polling".to_string());
        })
        .await;

        // Mark monitor as running BEFORE spawning to avoid race condition
        monitor.mark_running();

        // Start the monitor in a background task
        let monitor_clone = monitor.clone();
        tokio::spawn(async move {
            monitor_clone.start().await;
        });

        info!("Mailbox monitor started for account: {}", self.account_id);
        Ok(())
    }

    /// Process notifications from the monitor
    ///
    /// This should be called in a loop to handle incoming change notifications.
    /// Returns true if a notification was processed, false if the channel is empty/closed.
    pub async fn process_monitor_notification(&self) -> bool {
        let rx = match &self.monitor_rx {
            Some(rx) => rx,
            None => {
                debug!("No monitor receiver available");
                return false;
            }
        };

        // Try to receive a notification (non-blocking)
        match rx.try_recv() {
            Ok(notification) => {
                self.handle_notification(notification).await;
                true
            }
            Err(flume::TryRecvError::Empty) => false,
            Err(flume::TryRecvError::Disconnected) => {
                warn!("Monitor channel disconnected");
                false
            }
        }
    }

    /// Handle a single notification from the monitor
    async fn handle_notification(&self, notification: ChangeNotification) {
        match notification {
            ChangeNotification::PollTrigger => {
                debug!("Poll trigger received, checking for changes...");
                self.handle_poll_trigger().await;
            }
            ChangeNotification::NewMessages { folder } => {
                info!("New messages detected in folder: {}", folder);
                self.emit_event(SyncEvent::NewMessages {
                    folder: folder.clone(),
                    count: 0, // Unknown count
                });
                // Trigger a folder sync
                if let Err(e) = self.sync_folder(&folder).await {
                    error!("Failed to sync folder after new messages: {}", e);
                }
            }
            ChangeNotification::MessagesExpunged { folder } => {
                info!("Messages expunged in folder: {}", folder);
                // Trigger a folder sync
                if let Err(e) = self.sync_folder(&folder).await {
                    error!("Failed to sync folder after expunge: {}", e);
                }
            }
            ChangeNotification::FlagsChanged { folder } => {
                info!("Flags changed in folder: {}", folder);
                // Trigger a folder sync
                if let Err(e) = self.sync_folder(&folder).await {
                    error!("Failed to sync folder after flag change: {}", e);
                }
            }
            ChangeNotification::FolderChanged { folder } => {
                info!("Folder changed: {}", folder);
                if let Err(e) = self.sync_folder(&folder).await {
                    error!("Failed to sync changed folder: {}", e);
                }
            }
            ChangeNotification::ConnectionLost { error } => {
                error!("Monitor connection lost: {}", error);
                self.set_online(false);
                self.update_status(|s| {
                    s.is_online = false;
                    s.error = Some(format!("Connection lost: {}", error));
                })
                .await;
            }
            ChangeNotification::Shutdown => {
                info!("Monitor shutdown notification received");
            }
        }
    }

    /// Handle a poll trigger - check if folders have changed
    async fn handle_poll_trigger(&self) {
        info!("Processing poll trigger for account: {}", self.account_id);

        // Create backend for checking
        let backend = match self.create_backend().await {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to create backend for poll check: {}", e);
                self.set_online(false);
                return;
            }
        };

        // Mark as online since we connected successfully
        if !self.is_online() {
            info!("Connection restored for account: {}", self.account_id);
            self.set_online(true);
            self.update_status(|s| {
                s.is_online = true;
                s.error = None;
            })
            .await;
        }

        // Get folders and check each one
        let folders = match self.get_folders_to_sync(&backend).await {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to get folders for poll check: {}", e);
                return;
            }
        };

        let mut any_changes = false;

        for folder in &folders {
            debug!("Checking folder for changes: {}", folder);

            // Fetch most recent messages to detect changes
            match backend.list_envelopes(Some(folder), 0, 10).await {
                Ok(envelopes) => {
                    // Get the most recent message ID (first in the list, as they're sorted newest-first)
                    // Use and_then to flatten Option<Option<String>> to Option<String>
                    let latest_message_id = envelopes.first().and_then(|e| e.message_id.clone());

                    if let Some(monitor) = &self.monitor {
                        if monitor.check_folder_changes(folder, latest_message_id.as_deref()).await {
                            info!(
                                "Changes detected in folder '{}', triggering sync",
                                folder
                            );
                            any_changes = true;
                            monitor.update_folder_state(folder, latest_message_id).await;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to check folder '{}': {}", folder, e);
                }
            }
        }

        if any_changes {
            info!("Changes detected, triggering full sync");
            if let Err(e) = self.full_sync().await {
                error!("Failed to sync after poll detected changes: {}", e);
            }
        } else {
            debug!("No changes detected in poll check");
        }
    }

    /// Get the current monitoring mode
    pub async fn monitor_mode(&self) -> Option<MonitorMode> {
        if let Some(monitor) = &self.monitor {
            Some(monitor.mode().await)
        } else {
            None
        }
    }

    /// Check if monitoring is active
    pub fn is_monitoring(&self) -> bool {
        self.monitor
            .as_ref()
            .map(|m| m.is_running())
            .unwrap_or(false)
    }

    /// Stop the monitor
    pub fn stop_monitoring(&self) {
        if let Some(monitor) = &self.monitor {
            info!("Stopping monitor for account: {}", self.account_id);
            monitor.stop();
        }
    }

    /// Shutdown the sync engine
    pub fn shutdown(&self) {
        info!("Shutting down sync engine for account: {}", self.account_id);
        self.stop_monitoring();
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
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                },
                email,
            );
        }
    }

    // Plain email address
    (None, addr.to_lowercase())
}

/// Extract and store entities from a cached message
/// If the message is from the user or one of their aliases, recipients are marked as connections
fn extract_entities_from_message(
    db: &SyncDatabase,
    account_id: &str,
    user_email: &str,
    user_aliases: &[String],
    msg: &CachedChatMessage,
) {
    let contact_timestamp = msg.date.unwrap_or_else(Utc::now);
    let user_email_lower = user_email.to_lowercase();
    let from_address_lower = msg.from_address.to_lowercase();

    // Check if the message is from the user or one of their aliases (outgoing)
    let is_from_user = from_address_lower == user_email_lower
        || user_aliases.iter().any(|alias| alias == &from_address_lower);

    // Extract sender (from)
    if !is_from_user {
        // Don't add self as an entity
        if let Err(e) = db.upsert_entity(
            account_id,
            &msg.from_address,
            msg.from_name.as_deref(),
            false, // sender is not a connection unless we've sent to them
            contact_timestamp,
        ) {
            debug!("Failed to upsert entity for from address: {}", e);
        }
    }

    // Extract recipients (to)
    let to_addresses: Vec<String> = serde_json::from_str(&msg.to_addresses).unwrap_or_default();
    for addr in to_addresses {
        let (name, email) = parse_email_address(&addr);
        let email_lower = email.to_lowercase();

        // Skip self and aliases
        if email_lower == user_email_lower || user_aliases.iter().any(|alias| alias == &email_lower) {
            continue;
        }

        if let Err(e) = db.upsert_entity(
            account_id,
            &email,
            name.as_deref(),
            is_from_user, // Mark as connection if user sent the message
            contact_timestamp,
        ) {
            debug!("Failed to upsert entity for to address: {}", e);
        }
    }

    // Extract CC recipients
    if let Some(cc_json) = &msg.cc_addresses {
        let cc_addresses: Vec<String> = serde_json::from_str(cc_json).unwrap_or_default();
        for addr in cc_addresses {
            let (name, email) = parse_email_address(&addr);
            let email_lower = email.to_lowercase();

            // Skip self and aliases
            if email_lower == user_email_lower || user_aliases.iter().any(|alias| alias == &email_lower) {
                continue;
            }

            if let Err(e) = db.upsert_entity(
                account_id,
                &email,
                name.as_deref(),
                is_from_user, // Mark as connection if user sent the message
                contact_timestamp,
            ) {
                debug!("Failed to upsert entity for cc address: {}", e);
            }
        }
    }
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
