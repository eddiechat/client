//! IMAP IDLE and Polling for Change Monitoring
//!
//! Desktop: Maintains persistent connection with IDLE command (when available)
//! Mobile/Fallback: Polls at configurable intervals
//!
//! The monitoring system detects mailbox changes and triggers sync operations.

use chrono::{DateTime, Duration, Utc};
use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// IDLE/polling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Whether to use IDLE (if available) vs polling
    pub prefer_idle: bool,
    /// IDLE timeout in minutes (re-issue before NAT timeout)
    pub idle_timeout_minutes: u64,
    /// Poll interval in seconds (when IDLE not available/disabled)
    pub poll_interval_seconds: u64,
    /// Quick-check optimization: skip full sync if message count unchanged
    pub use_quick_check: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            prefer_idle: true,
            idle_timeout_minutes: 20, // Re-issue before 29-minute NAT timeout
            poll_interval_seconds: 60, // 1 minute for polling
            use_quick_check: true,
        }
    }
}

/// Change notification types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeNotification {
    /// New message(s) may have arrived - trigger sync
    NewMessages { folder: String },
    /// Message(s) may have been expunged - trigger sync
    MessagesExpunged { folder: String },
    /// Flags may have changed - trigger sync
    FlagsChanged { folder: String },
    /// General folder state change detected
    FolderChanged { folder: String },
    /// Time for a poll check
    PollTrigger,
    /// Connection lost - need to reconnect
    ConnectionLost { error: String },
    /// Monitor is stopping
    Shutdown,
}

/// Monitoring mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    /// Using IMAP IDLE for push notifications
    Idle,
    /// Using periodic polling
    Polling,
    /// Not monitoring (stopped)
    Stopped,
}

/// State tracked for quick-check optimization
#[derive(Debug, Clone, Default)]
pub struct FolderState {
    pub folder: String,
    pub message_count: Option<u32>,
    pub latest_message_id: Option<String>,
    pub last_check: Option<DateTime<Utc>>,
}

impl FolderState {
    pub fn new(folder: String) -> Self {
        Self {
            folder,
            message_count: None,
            latest_message_id: None,
            last_check: None,
        }
    }

    /// Check if folder has changes by comparing latest message ID
    pub fn has_changes(&self, new_latest_id: Option<&str>) -> bool {
        match (&self.latest_message_id, new_latest_id) {
            (Some(old_id), Some(new_id)) => {
                if old_id != new_id {
                    info!(
                        "Folder '{}' latest message changed: {} -> {}",
                        self.folder, old_id, new_id
                    );
                    true
                } else {
                    debug!(
                        "Folder '{}' latest message unchanged: {}",
                        self.folder, old_id
                    );
                    false
                }
            }
            (None, Some(new_id)) => {
                info!(
                    "Folder '{}' initial latest message: {}",
                    self.folder, new_id
                );
                true // First check, assume changes
            }
            (Some(_), None) => {
                warn!(
                    "Folder '{}' has no messages (was not empty before)",
                    self.folder
                );
                true // Folder became empty, trigger sync
            }
            (None, None) => {
                debug!("Folder '{}' is empty", self.folder);
                false // Both empty, no changes
            }
        }
    }

    /// Update the tracked state
    pub fn update(&mut self, latest_message_id: Option<String>) {
        self.latest_message_id = latest_message_id;
        self.last_check = Some(Utc::now());
    }
}

/// Mailbox monitor that uses polling (with IDLE-ready structure)
///
/// Currently implements polling-based monitoring. The structure is ready
/// for native IMAP IDLE support when/if email-lib exposes it.
pub struct MailboxMonitor {
    account_id: String,
    folders: Vec<String>,
    config: MonitorConfig,
    mode: Arc<RwLock<MonitorMode>>,
    running: Arc<AtomicBool>,
    notification_tx: Sender<ChangeNotification>,
    folder_states: Arc<RwLock<Vec<FolderState>>>,
    supports_idle: bool,
}

impl MailboxMonitor {
    /// Create a new mailbox monitor
    pub fn new(
        account_id: String,
        folders: Vec<String>,
        config: MonitorConfig,
        supports_idle: bool,
    ) -> (Self, Receiver<ChangeNotification>) {
        let (tx, rx) = flume::unbounded();

        let folder_states: Vec<FolderState> = folders
            .iter()
            .map(|f| FolderState::new(f.clone()))
            .collect();

        let monitor = Self {
            account_id,
            folders,
            config,
            mode: Arc::new(RwLock::new(MonitorMode::Stopped)),
            running: Arc::new(AtomicBool::new(false)),
            notification_tx: tx,
            folder_states: Arc::new(RwLock::new(folder_states)),
            supports_idle,
        };

        (monitor, rx)
    }

    /// Get current monitoring mode
    pub async fn mode(&self) -> MonitorMode {
        *self.mode.read().await
    }

    /// Check if monitor is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Stop the monitor
    pub fn stop(&self) {
        info!(
            "Stopping mailbox monitor for account: {}",
            self.account_id
        );
        self.running.store(false, Ordering::SeqCst);
        let _ = self.notification_tx.send(ChangeNotification::Shutdown);
    }

    /// Mark monitor as running (call before spawning the monitor task)
    pub fn mark_running(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Get the notification sender (for external triggers)
    pub fn notification_sender(&self) -> Sender<ChangeNotification> {
        self.notification_tx.clone()
    }

    /// Start the monitoring loop
    ///
    /// This runs in a background task and sends notifications when changes are detected.
    /// Currently uses polling; structured to support IDLE in the future.
    ///
    /// Note: The running flag should be set to true (via mark_running()) before calling this.
    pub async fn start(&self) {
        // Note: We don't check/set running here because it's set externally via mark_running()
        // to avoid a race condition with the notification processing loop

        // Determine monitoring mode
        let mode = if self.supports_idle && self.config.prefer_idle {
            // TODO: When email-lib supports IDLE, implement native IDLE here
            // For now, fall back to polling even if server supports IDLE
            info!(
                "Server supports IDLE but using polling (IDLE not yet implemented in backend)"
            );
            MonitorMode::Polling
        } else {
            info!(
                "Using polling mode for account: {} (interval: {}s)",
                self.account_id, self.config.poll_interval_seconds
            );
            MonitorMode::Polling
        };

        {
            let mut mode_lock = self.mode.write().await;
            *mode_lock = mode;
        }

        match mode {
            MonitorMode::Idle => {
                self.run_idle_loop().await;
            }
            MonitorMode::Polling => {
                self.run_poll_loop().await;
            }
            MonitorMode::Stopped => {}
        }

        // Cleanup
        {
            let mut mode_lock = self.mode.write().await;
            *mode_lock = MonitorMode::Stopped;
        }
        info!("Monitor stopped for account: {}", self.account_id);
    }

    /// Run the polling loop
    async fn run_poll_loop(&self) {
        let interval = tokio::time::Duration::from_secs(self.config.poll_interval_seconds);
        let mut poll_interval = tokio::time::interval(interval);

        info!(
            "Starting poll loop for account: {} (folders: {:?}, interval: {:?})",
            self.account_id, self.folders, interval
        );

        let mut poll_count = 0u64;
        let start_time = std::time::Instant::now();

        while self.running.load(Ordering::SeqCst) {
            poll_interval.tick().await;

            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            poll_count += 1;
            let elapsed = start_time.elapsed();

            debug!(
                "Poll tick #{} for account: {} (elapsed: {:?})",
                poll_count, self.account_id, elapsed
            );

            // Send poll trigger notification
            if let Err(e) = self.notification_tx.send(ChangeNotification::PollTrigger) {
                error!(
                    "Failed to send poll trigger #{} for account {}: {}",
                    poll_count, self.account_id, e
                );
                break;
            }
        }

        info!(
            "Poll loop stopped for account: {} after {} polls over {:?}",
            self.account_id,
            poll_count,
            start_time.elapsed()
        );
    }

    /// Run the IDLE loop (placeholder for future implementation)
    async fn run_idle_loop(&self) {
        // TODO: Implement native IMAP IDLE when email-lib supports it
        //
        // The implementation would:
        // 1. Open a persistent IMAP connection
        // 2. SELECT the folder
        // 3. Send IDLE command
        // 4. Parse untagged responses for EXISTS, EXPUNGE, FETCH
        // 5. Break IDLE before timeout (29 min NAT, use 20 min)
        // 6. Re-issue IDLE command
        //
        // For now, fall back to polling
        warn!("IDLE mode requested but not implemented, falling back to polling");
        self.run_poll_loop().await;
    }

    /// Update folder state after checking
    pub async fn update_folder_state(&self, folder: &str, latest_message_id: Option<String>) {
        let mut states = self.folder_states.write().await;
        if let Some(state) = states.iter_mut().find(|s| s.folder == folder) {
            state.update(latest_message_id.clone());
            debug!(
                "Updated folder state for '{}': latest_message_id = {:?}",
                folder, latest_message_id
            );
        }
    }

    /// Check if a folder has changes based on latest message ID
    pub async fn check_folder_changes(&self, folder: &str, latest_message_id: Option<&str>) -> bool {
        let states = self.folder_states.read().await;
        if let Some(state) = states.iter().find(|s| s.folder == folder) {
            state.has_changes(latest_message_id)
        } else {
            true // Unknown folder, assume changes
        }
    }

    /// Get time since last check for a folder
    pub async fn time_since_last_check(&self, folder: &str) -> Option<Duration> {
        let states = self.folder_states.read().await;
        states
            .iter()
            .find(|s| s.folder == folder)
            .and_then(|s| s.last_check)
            .map(|t| Utc::now() - t)
    }
}

/// Parse IMAP untagged response for change notifications
///
/// Used when we have access to raw IMAP responses (e.g., from IDLE)
pub fn parse_untagged_response(line: &str, folder: &str) -> Option<ChangeNotification> {
    let line = line.trim();
    debug!("Parsing IMAP response: {}", line);

    // Handle EXISTS response: "* 42 EXISTS"
    if line.ends_with(" EXISTS") {
        if let Some(count_str) = line
            .strip_prefix("* ")
            .and_then(|s| s.strip_suffix(" EXISTS"))
        {
            if count_str.parse::<u32>().is_ok() {
                info!("Detected EXISTS response for folder '{}'", folder);
                return Some(ChangeNotification::NewMessages {
                    folder: folder.to_string(),
                });
            }
        }
    }

    // Handle EXPUNGE response: "* 42 EXPUNGE"
    if line.ends_with(" EXPUNGE") {
        if line.strip_prefix("* ").is_some() {
            info!("Detected EXPUNGE response for folder '{}'", folder);
            return Some(ChangeNotification::MessagesExpunged {
                folder: folder.to_string(),
            });
        }
    }

    // Handle FETCH response for flag changes: "* 42 FETCH (FLAGS (...))"
    if line.contains(" FETCH ") && line.contains("FLAGS") {
        info!("Detected FLAGS change for folder '{}'", folder);
        return Some(ChangeNotification::FlagsChanged {
            folder: folder.to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exists() {
        let result = parse_untagged_response("* 42 EXISTS", "INBOX");
        match result {
            Some(ChangeNotification::NewMessages { folder }) => {
                assert_eq!(folder, "INBOX");
            }
            _ => panic!("Expected NewMessages"),
        }
    }

    #[test]
    fn test_parse_expunge() {
        let result = parse_untagged_response("* 15 EXPUNGE", "INBOX");
        match result {
            Some(ChangeNotification::MessagesExpunged { folder }) => {
                assert_eq!(folder, "INBOX");
            }
            _ => panic!("Expected MessagesExpunged"),
        }
    }

    #[test]
    fn test_parse_fetch_flags() {
        let result = parse_untagged_response("* 42 FETCH (FLAGS (\\Seen \\Flagged))", "INBOX");
        match result {
            Some(ChangeNotification::FlagsChanged { folder }) => {
                assert_eq!(folder, "INBOX");
            }
            _ => panic!("Expected FlagsChanged"),
        }
    }

    #[test]
    fn test_folder_state_changes() {
        let mut state = FolderState::new("INBOX".to_string());

        // First check - no previous state
        assert!(state.has_changes(Some("msg-123")));
        state.update(Some("msg-123".to_string()));

        // Same message ID - no changes
        assert!(!state.has_changes(Some("msg-123")));

        // Different message ID - changes
        assert!(state.has_changes(Some("msg-456")));
        state.update(Some("msg-456".to_string()));

        // Same new message ID - no changes
        assert!(!state.has_changes(Some("msg-456")));

        // Empty folder
        assert!(state.has_changes(None));
        state.update(None);
        assert!(!state.has_changes(None));
    }
}
