//! IMAP IDLE and Polling for Change Monitoring
//!
//! Desktop: Maintains persistent connection with IDLE command
//! Mobile: Polls at configurable intervals

use chrono::{DateTime, Duration, Utc};
use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::interval;

/// IDLE/polling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleConfig {
    /// Whether to use IDLE (if available) vs polling
    pub use_idle: bool,
    /// IDLE timeout in minutes (re-issue before NAT timeout)
    pub idle_timeout_minutes: u64,
    /// Poll interval in seconds (when IDLE not available/disabled)
    pub poll_interval_seconds: u64,
    /// Quick-check optimization: skip full sync if UIDNEXT/HIGHESTMODSEQ unchanged
    pub use_quick_check: bool,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            use_idle: true,
            idle_timeout_minutes: 20, // Re-issue before 29-minute NAT timeout
            poll_interval_seconds: 60 * 15, // 15 minutes for polling
            use_quick_check: true,
        }
    }
}

/// Change notification types from IDLE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeNotification {
    /// New message(s) arrived (EXISTS)
    NewMessages { folder: String, count: u32 },
    /// Message(s) expunged (EXPUNGE)
    MessagesExpunged { folder: String, count: u32 },
    /// Flags changed (FETCH)
    FlagsChanged { folder: String, uid: u32 },
    /// Folder selected state changed (UIDNEXT, HIGHESTMODSEQ, etc.)
    FolderStateChanged { folder: String },
    /// IDLE connection needs to be re-established
    IdleRefresh,
    /// Time for a poll check
    PollTrigger,
    /// Connection lost
    ConnectionLost { error: String },
}

/// IDLE state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleState {
    /// Not connected
    Disconnected,
    /// Connecting to server
    Connecting,
    /// Selecting folder
    Selecting,
    /// In IDLE mode, waiting for notifications
    Idling,
    /// Processing a notification
    Processing,
    /// Temporarily paused (for other operations)
    Paused,
}

/// IDLE monitor for a single folder
pub struct IdleMonitor {
    account_id: String,
    folder: String,
    config: IdleConfig,
    state: Arc<RwLock<IdleState>>,
    running: Arc<AtomicBool>,
    notification_tx: Sender<ChangeNotification>,
    last_idle_time: Arc<RwLock<DateTime<Utc>>>,
}

impl IdleMonitor {
    /// Create a new IDLE monitor
    pub fn new(
        account_id: String,
        folder: String,
        config: IdleConfig,
    ) -> (Self, Receiver<ChangeNotification>) {
        let (tx, rx) = flume::unbounded();

        let monitor = Self {
            account_id,
            folder,
            config,
            state: Arc::new(RwLock::new(IdleState::Disconnected)),
            running: Arc::new(AtomicBool::new(false)),
            notification_tx: tx,
            last_idle_time: Arc::new(RwLock::new(Utc::now())),
        };

        (monitor, rx)
    }

    /// Get current state
    pub async fn state(&self) -> IdleState {
        *self.state.read().await
    }

    /// Check if monitor is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Stop the monitor
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Pause IDLE temporarily (for other operations)
    pub async fn pause(&self) {
        let mut state = self.state.write().await;
        if *state == IdleState::Idling {
            *state = IdleState::Paused;
        }
    }

    /// Resume IDLE after pause
    pub async fn resume(&self) {
        let mut state = self.state.write().await;
        if *state == IdleState::Paused {
            *state = IdleState::Idling;
        }
    }

    /// Send a notification
    fn notify(&self, notification: ChangeNotification) {
        let _ = self.notification_tx.send(notification);
    }

    /// Check if IDLE needs to be refreshed
    pub async fn needs_refresh(&self) -> bool {
        let last_time = *self.last_idle_time.read().await;
        let elapsed = Utc::now() - last_time;
        elapsed > Duration::minutes(self.config.idle_timeout_minutes as i64)
    }

    /// Update last IDLE time
    pub async fn update_idle_time(&self) {
        let mut last_time = self.last_idle_time.write().await;
        *last_time = Utc::now();
    }
}

/// Poll-based change monitor (for when IDLE is not available)
pub struct PollMonitor {
    account_id: String,
    folders: Vec<String>,
    config: IdleConfig,
    running: Arc<AtomicBool>,
    notification_tx: Sender<ChangeNotification>,
    last_poll_time: Arc<RwLock<DateTime<Utc>>>,
}

impl PollMonitor {
    /// Create a new poll monitor
    pub fn new(
        account_id: String,
        folders: Vec<String>,
        config: IdleConfig,
    ) -> (Self, Receiver<ChangeNotification>) {
        let (tx, rx) = flume::unbounded();

        let monitor = Self {
            account_id,
            folders,
            config,
            running: Arc::new(AtomicBool::new(false)),
            notification_tx: tx,
            last_poll_time: Arc::new(RwLock::new(Utc::now())),
        };

        (monitor, rx)
    }

    /// Start the poll loop
    pub async fn start(&self) {
        self.running.store(true, Ordering::SeqCst);

        let interval_duration = tokio::time::Duration::from_secs(self.config.poll_interval_seconds);
        let mut poll_interval = interval(interval_duration);

        while self.running.load(Ordering::SeqCst) {
            poll_interval.tick().await;

            if self.running.load(Ordering::SeqCst) {
                // Update last poll time
                {
                    let mut last_time = self.last_poll_time.write().await;
                    *last_time = Utc::now();
                }

                // Notify that it's time to poll
                let _ = self.notification_tx.send(ChangeNotification::PollTrigger);
            }
        }
    }

    /// Stop the poll loop
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if monitor is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get time until next poll
    pub async fn time_until_next_poll(&self) -> Duration {
        let last_time = *self.last_poll_time.read().await;
        let elapsed = Utc::now() - last_time;
        let interval = Duration::seconds(self.config.poll_interval_seconds as i64);

        if elapsed >= interval {
            Duration::zero()
        } else {
            interval - elapsed
        }
    }

    /// Trigger an immediate poll
    pub fn trigger_poll(&self) {
        let _ = self.notification_tx.send(ChangeNotification::PollTrigger);
    }
}

/// Quick-check state for optimizing polls
#[derive(Debug, Clone, Default)]
pub struct QuickCheckState {
    pub uidnext: Option<u32>,
    pub highestmodseq: Option<u64>,
    pub exists: Option<u32>,
}

impl QuickCheckState {
    /// Check if anything changed
    pub fn changed(&self, other: &QuickCheckState) -> bool {
        // If any value changed, we need a full sync
        if self.uidnext != other.uidnext {
            return true;
        }
        if self.highestmodseq != other.highestmodseq {
            return true;
        }
        if self.exists != other.exists {
            return true;
        }
        false
    }
}

/// Parse IMAP untagged response for change notifications
pub fn parse_untagged_response(line: &str, folder: &str) -> Option<ChangeNotification> {
    let line = line.trim();

    // Handle EXISTS response: "* 42 EXISTS"
    if line.ends_with(" EXISTS") {
        if let Some(count_str) = line
            .strip_prefix("* ")
            .and_then(|s| s.strip_suffix(" EXISTS"))
        {
            if let Ok(count) = count_str.parse::<u32>() {
                return Some(ChangeNotification::NewMessages {
                    folder: folder.to_string(),
                    count,
                });
            }
        }
    }

    // Handle EXPUNGE response: "* 42 EXPUNGE"
    if line.ends_with(" EXPUNGE") {
        if let Some(_) = line
            .strip_prefix("* ")
            .and_then(|s| s.strip_suffix(" EXPUNGE"))
        {
            return Some(ChangeNotification::MessagesExpunged {
                folder: folder.to_string(),
                count: 1, // EXPUNGE is per-message
            });
        }
    }

    // Handle FETCH response for flag changes: "* 42 FETCH (FLAGS (...))"
    if line.contains(" FETCH ") && line.contains("FLAGS") {
        if let Some(num_str) = line.strip_prefix("* ") {
            if let Some(space_pos) = num_str.find(' ') {
                if let Ok(uid) = num_str[..space_pos].parse::<u32>() {
                    return Some(ChangeNotification::FlagsChanged {
                        folder: folder.to_string(),
                        uid,
                    });
                }
            }
        }
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
            Some(ChangeNotification::NewMessages { folder, count }) => {
                assert_eq!(folder, "INBOX");
                assert_eq!(count, 42);
            }
            _ => panic!("Expected NewMessages"),
        }
    }

    #[test]
    fn test_parse_expunge() {
        let result = parse_untagged_response("* 15 EXPUNGE", "INBOX");
        match result {
            Some(ChangeNotification::MessagesExpunged { folder, .. }) => {
                assert_eq!(folder, "INBOX");
            }
            _ => panic!("Expected MessagesExpunged"),
        }
    }

    #[test]
    fn test_parse_fetch_flags() {
        let result = parse_untagged_response("* 42 FETCH (FLAGS (\\Seen \\Flagged))", "INBOX");
        match result {
            Some(ChangeNotification::FlagsChanged { folder, uid }) => {
                assert_eq!(folder, "INBOX");
                assert_eq!(uid, 42);
            }
            _ => panic!("Expected FlagsChanged"),
        }
    }

    #[test]
    fn test_quick_check_changed() {
        let state1 = QuickCheckState {
            uidnext: Some(100),
            highestmodseq: Some(500),
            exists: Some(50),
        };

        let state2 = QuickCheckState {
            uidnext: Some(101), // New message
            highestmodseq: Some(500),
            exists: Some(51),
        };

        assert!(state1.changed(&state2));

        let state3 = QuickCheckState {
            uidnext: Some(100),
            highestmodseq: Some(500),
            exists: Some(50),
        };

        assert!(!state1.changed(&state3));
    }
}
