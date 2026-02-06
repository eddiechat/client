//! Action Queue for Offline Support
//!
//! Queues user actions locally when offline and replays them on reconnect.
//! Uses additive flag operations (+FLAGS/-FLAGS) to avoid conflicts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::backend::EmailBackend;
use crate::sync::db::{is_read_only_mode, QueuedActionRecord, SyncDatabase};
use crate::types::error::EddieError;

/// Types of actions that can be queued
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionType {
    /// Add flags to messages (uses +FLAGS, not FLAGS)
    AddFlags {
        folder: String,
        uids: Vec<u32>,
        flags: Vec<String>,
    },

    /// Remove flags from messages (uses -FLAGS)
    RemoveFlags {
        folder: String,
        uids: Vec<u32>,
        flags: Vec<String>,
    },

    /// Delete messages (mark as \Deleted, then EXPUNGE)
    Delete { folder: String, uids: Vec<u32> },

    /// Move messages to another folder
    Move {
        source_folder: String,
        target_folder: String,
        uids: Vec<u32>,
    },

    /// Copy messages to another folder
    Copy {
        source_folder: String,
        target_folder: String,
        uids: Vec<u32>,
    },

    /// Send a message via SMTP
    Send {
        raw_message: Vec<u8>,
        save_to_sent: bool,
    },

    /// Save a message to a folder (draft, etc.)
    Save {
        folder: String,
        raw_message: Vec<u8>,
    },
}

impl ActionType {
    /// Get the action type string for database storage
    pub fn type_str(&self) -> &'static str {
        match self {
            Self::AddFlags { .. } => "add_flags",
            Self::RemoveFlags { .. } => "remove_flags",
            Self::Delete { .. } => "delete",
            Self::Move { .. } => "move",
            Self::Copy { .. } => "copy",
            Self::Send { .. } => "send",
            Self::Save { .. } => "save",
        }
    }

    /// Get the folder this action operates on
    pub fn folder(&self) -> Option<&str> {
        match self {
            Self::AddFlags { folder, .. } => Some(folder),
            Self::RemoveFlags { folder, .. } => Some(folder),
            Self::Delete { folder, .. } => Some(folder),
            Self::Move { source_folder, .. } => Some(source_folder),
            Self::Copy { source_folder, .. } => Some(source_folder),
            Self::Send { .. } => None,
            Self::Save { folder, .. } => Some(folder),
        }
    }

    /// Get the UIDs this action operates on
    pub fn uids(&self) -> Option<&[u32]> {
        match self {
            Self::AddFlags { uids, .. } => Some(uids),
            Self::RemoveFlags { uids, .. } => Some(uids),
            Self::Delete { uids, .. } => Some(uids),
            Self::Move { uids, .. } => Some(uids),
            Self::Copy { uids, .. } => Some(uids),
            Self::Send { .. } => None,
            Self::Save { .. } => None,
        }
    }

    /// Check if this action can be merged with another
    pub fn can_merge(&self, other: &ActionType) -> bool {
        match (self, other) {
            (
                Self::AddFlags {
                    folder: f1,
                    flags: flags1,
                    ..
                },
                Self::AddFlags {
                    folder: f2,
                    flags: flags2,
                    ..
                },
            ) => f1 == f2 && flags1 == flags2,
            (
                Self::RemoveFlags {
                    folder: f1,
                    flags: flags1,
                    ..
                },
                Self::RemoveFlags {
                    folder: f2,
                    flags: flags2,
                    ..
                },
            ) => f1 == f2 && flags1 == flags2,
            (Self::Delete { folder: f1, .. }, Self::Delete { folder: f2, .. }) => f1 == f2,
            _ => false,
        }
    }

    /// Merge UIDs from another action of the same type
    pub fn merge_uids(&mut self, other: &ActionType) {
        match (self, other) {
            (Self::AddFlags { uids: uids1, .. }, Self::AddFlags { uids: uids2, .. }) => {
                for uid in uids2 {
                    if !uids1.contains(uid) {
                        uids1.push(*uid);
                    }
                }
            }
            (Self::RemoveFlags { uids: uids1, .. }, Self::RemoveFlags { uids: uids2, .. }) => {
                for uid in uids2 {
                    if !uids1.contains(uid) {
                        uids1.push(*uid);
                    }
                }
            }
            (Self::Delete { uids: uids1, .. }, Self::Delete { uids: uids2, .. }) => {
                for uid in uids2 {
                    if !uids1.contains(uid) {
                        uids1.push(*uid);
                    }
                }
            }
            _ => {}
        }
    }
}

/// A queued action with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedAction {
    pub id: Option<i64>,
    pub account_id: String,
    pub action: ActionType,
    pub created_at: DateTime<Utc>,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub status: ActionStatus,
}

/// Status of a queued action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl ActionStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "processing" => Self::Processing,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

/// Action queue manager
pub struct ActionQueue {
    db: Arc<SyncDatabase>,
    max_retries: u32,
}

impl ActionQueue {
    /// Create a new action queue
    pub fn new(db: Arc<SyncDatabase>) -> Self {
        Self { db, max_retries: 5 }
    }

    /// Queue a new action
    pub fn queue(&self, account_id: &str, action: ActionType) -> Result<i64, EddieError> {
        // Check read-only mode
        if is_read_only_mode()? {
            info!(
                "Read-only mode: Blocked queuing {:?} action for account {}",
                action.type_str(),
                account_id
            );
            return Err(EddieError::ReadOnlyMode);
        }

        let payload = serde_json::to_string(&action)
            .map_err(|e| EddieError::Backend(format!("Failed to serialize action: {}", e)))?;

        let record = QueuedActionRecord {
            id: 0,
            account_id: account_id.to_string(),
            action_type: action.type_str().to_string(),
            folder_name: action.folder().map(|s| s.to_string()),
            uid: action.uids().and_then(|u| u.first().copied()),
            payload,
            created_at: Utc::now(),
            retry_count: 0,
            last_error: None,
            status: "pending".to_string(),
        };

        self.db.queue_action(&record)
    }

    /// Get all pending actions for an account
    pub fn get_pending(&self, account_id: &str) -> Result<Vec<QueuedAction>, EddieError> {
        let records = self.db.get_pending_actions(account_id)?;

        let mut actions = Vec::new();
        for record in records {
            let action: ActionType = serde_json::from_str(&record.payload).map_err(|e| {
                EddieError::Backend(format!("Failed to deserialize action: {}", e))
            })?;

            actions.push(QueuedAction {
                id: Some(record.id),
                account_id: record.account_id,
                action,
                created_at: record.created_at,
                retry_count: record.retry_count,
                last_error: record.last_error,
                status: ActionStatus::from_str(&record.status),
            });
        }

        Ok(actions)
    }

    /// Check if there are pending actions
    pub fn has_pending(&self, account_id: &str) -> Result<bool, EddieError> {
        let actions = self.db.get_pending_actions(account_id)?;
        Ok(!actions.is_empty())
    }

    /// Mark an action as processing (does not increment retry_count)
    pub fn mark_processing(&self, id: i64) -> Result<(), EddieError> {
        self.db
            .update_action_status_no_retry_increment(id, "processing")
    }

    /// Mark an action as completed (does not increment retry_count)
    pub fn mark_completed(&self, id: i64) -> Result<(), EddieError> {
        self.db
            .update_action_status_no_retry_increment(id, "completed")
    }

    /// Mark an action as failed
    pub fn mark_failed(&self, id: i64, error: &str) -> Result<(), EddieError> {
        self.db.update_action_status(id, "failed", Some(error))
    }

    /// Check if an action should be retried
    pub fn should_retry(&self, action: &QueuedAction) -> bool {
        action.retry_count < self.max_retries
    }

    /// Retry a failed action (reset to pending)
    pub fn retry(&self, id: i64) -> Result<(), EddieError> {
        self.db.update_action_status(id, "pending", None)
    }

    /// Clean up completed actions
    pub fn cleanup_completed(&self, account_id: &str) -> Result<u64, EddieError> {
        self.db.delete_completed_actions(account_id)
    }

    /// Replay all pending actions for an account
    ///
    /// Executes each pending action on the IMAP server via the EmailBackend.
    /// Actions are marked as processing before execution to prevent double-execution.
    /// On success, actions are marked as completed. On failure, retry_count is incremented
    /// and the action is marked as failed if max retries exceeded, otherwise reset to pending.
    pub async fn replay_pending(
        &self,
        account_id: &str,
        backend: &EmailBackend,
    ) -> Result<Vec<ReplayResult>, EddieError> {
        // Check read-only mode
        if is_read_only_mode()? {
            let pending = self.get_pending(account_id)?;
            info!(
                "Read-only mode: Skipping replay of {} pending actions for account {}",
                pending.len(),
                account_id
            );
            return Ok(vec![]);
        }

        let pending = self.get_pending(account_id)?;

        if pending.is_empty() {
            info!("No pending actions to replay for account {}", account_id);
            return Ok(vec![]);
        }

        info!(
            "Replaying {} pending actions for account {}",
            pending.len(),
            account_id
        );

        let mut results = Vec::with_capacity(pending.len());

        for action in pending {
            let action_id = match action.id {
                Some(id) => id,
                None => {
                    warn!("Action missing ID, skipping");
                    continue;
                }
            };

            // Mark as processing to prevent double-execution
            if let Err(e) = self.mark_processing(action_id) {
                error!("Failed to mark action {} as processing: {}", action_id, e);
                continue;
            }

            // Execute the action
            let result = self.execute_action(backend, &action.action).await;

            match result {
                Ok(()) => {
                    info!("Action {} completed successfully", action_id);
                    if let Err(e) = self.mark_completed(action_id) {
                        error!("Failed to mark action {} as completed: {}", action_id, e);
                    }
                    results.push(ReplayResult::Success);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    warn!("Action {} failed: {}", action_id, error_msg);

                    // Check if we should retry or discard
                    if self.should_retry(&action) {
                        // Mark as failed (increments retry_count) but leave for retry
                        if let Err(mark_err) = self.mark_failed(action_id, &error_msg) {
                            error!("Failed to mark action {} as failed: {}", action_id, mark_err);
                        }
                        // Reset to pending for next retry
                        if let Err(retry_err) = self.retry(action_id) {
                            error!("Failed to reset action {} to pending: {}", action_id, retry_err);
                        }
                        results.push(ReplayResult::Retry(error_msg));
                    } else {
                        // Max retries exceeded, mark as permanently failed
                        error!(
                            "Action {} exceeded max retries ({}), marking as failed",
                            action_id, self.max_retries
                        );
                        if let Err(mark_err) = self.mark_failed(action_id, &error_msg) {
                            error!("Failed to mark action {} as failed: {}", action_id, mark_err);
                        }
                        results.push(ReplayResult::Discard(error_msg));
                    }
                }
            }
        }

        // Clean up completed actions
        if let Ok(cleaned) = self.cleanup_completed(account_id) {
            if cleaned > 0 {
                info!("Cleaned up {} completed actions", cleaned);
            }
        }

        Ok(results)
    }

    /// Execute a single action on the IMAP server
    async fn execute_action(
        &self,
        backend: &EmailBackend,
        action: &ActionType,
    ) -> Result<(), EddieError> {
        match action {
            ActionType::AddFlags { folder, uids, flags } => {
                info!(
                    "Executing AddFlags: folder={}, uids={:?}, flags={:?}",
                    folder, uids, flags
                );
                let uid_strs: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
                let uid_refs: Vec<&str> = uid_strs.iter().map(|s| s.as_str()).collect();
                let flag_refs: Vec<&str> = flags.iter().map(|s| s.as_str()).collect();
                backend.add_flags(Some(folder), &uid_refs, &flag_refs).await
            }

            ActionType::RemoveFlags { folder, uids, flags } => {
                info!(
                    "Executing RemoveFlags: folder={}, uids={:?}, flags={:?}",
                    folder, uids, flags
                );
                let uid_strs: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
                let uid_refs: Vec<&str> = uid_strs.iter().map(|s| s.as_str()).collect();
                let flag_refs: Vec<&str> = flags.iter().map(|s| s.as_str()).collect();
                backend
                    .remove_flags(Some(folder), &uid_refs, &flag_refs)
                    .await
            }

            ActionType::Delete { folder, uids } => {
                info!("Executing Delete: folder={}, uids={:?}", folder, uids);
                let uid_strs: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
                let uid_refs: Vec<&str> = uid_strs.iter().map(|s| s.as_str()).collect();
                backend.delete_messages(Some(folder), &uid_refs).await
            }

            ActionType::Move {
                source_folder,
                target_folder,
                uids,
            } => {
                info!(
                    "Executing Move: source={}, target={}, uids={:?}",
                    source_folder, target_folder, uids
                );
                let uid_strs: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
                let uid_refs: Vec<&str> = uid_strs.iter().map(|s| s.as_str()).collect();
                backend
                    .move_messages(Some(source_folder), target_folder, &uid_refs)
                    .await
            }

            ActionType::Copy {
                source_folder,
                target_folder,
                uids,
            } => {
                info!(
                    "Executing Copy: source={}, target={}, uids={:?}",
                    source_folder, target_folder, uids
                );
                let uid_strs: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
                let uid_refs: Vec<&str> = uid_strs.iter().map(|s| s.as_str()).collect();
                backend
                    .copy_messages(Some(source_folder), target_folder, &uid_refs)
                    .await
            }

            ActionType::Send {
                raw_message,
                save_to_sent,
            } => {
                info!("Executing Send: save_to_sent={}", save_to_sent);
                // Note: send_message already handles saving to Sent folder based on find_sent_folder
                // If save_to_sent is false, we'd need a different approach, but for now we use the default
                backend.send_message(raw_message).await?;
                Ok(())
            }

            ActionType::Save { folder, raw_message } => {
                info!("Executing Save: folder={}", folder);
                backend.save_message(Some(folder), raw_message).await?;
                Ok(())
            }
        }
    }
}

/// Result of replaying an action
#[derive(Debug)]
pub enum ReplayResult {
    /// Action completed successfully
    Success,
    /// Action failed but can be retried
    Retry(String),
    /// Action failed permanently (conflict, message deleted, etc.)
    Discard(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_type_serialization() {
        let action = ActionType::AddFlags {
            folder: "INBOX".to_string(),
            uids: vec![1, 2, 3],
            flags: vec!["\\Seen".to_string()],
        };

        let json = serde_json::to_string(&action).unwrap();
        let deserialized: ActionType = serde_json::from_str(&json).unwrap();

        match deserialized {
            ActionType::AddFlags {
                folder,
                uids,
                flags,
            } => {
                assert_eq!(folder, "INBOX");
                assert_eq!(uids, vec![1, 2, 3]);
                assert_eq!(flags, vec!["\\Seen"]);
            }
            _ => panic!("Wrong action type"),
        }
    }

    #[test]
    fn test_action_merge() {
        let mut action1 = ActionType::AddFlags {
            folder: "INBOX".to_string(),
            uids: vec![1, 2],
            flags: vec!["\\Seen".to_string()],
        };

        let action2 = ActionType::AddFlags {
            folder: "INBOX".to_string(),
            uids: vec![3, 4],
            flags: vec!["\\Seen".to_string()],
        };

        assert!(action1.can_merge(&action2));
        action1.merge_uids(&action2);

        match action1 {
            ActionType::AddFlags { uids, .. } => {
                assert_eq!(uids, vec![1, 2, 3, 4]);
            }
            _ => panic!("Wrong action type"),
        }
    }
}
