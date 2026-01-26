//! Action Queue for Offline Support
//!
//! Queues user actions locally when offline and replays them on reconnect.
//! Uses additive flag operations (+FLAGS/-FLAGS) to avoid conflicts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::sync::db::{QueuedActionRecord, SyncDatabase};
use crate::types::error::HimalayaError;

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
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

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
    pub fn queue(&self, account_id: &str, action: ActionType) -> Result<i64, HimalayaError> {
        let payload = serde_json::to_string(&action)
            .map_err(|e| HimalayaError::Backend(format!("Failed to serialize action: {}", e)))?;

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

    /// Queue a mark-as-read action
    pub fn queue_mark_read(
        &self,
        account_id: &str,
        folder: &str,
        uids: Vec<u32>,
    ) -> Result<i64, HimalayaError> {
        self.queue(
            account_id,
            ActionType::AddFlags {
                folder: folder.to_string(),
                uids,
                flags: vec!["\\Seen".to_string()],
            },
        )
    }

    /// Queue a mark-as-unread action
    pub fn queue_mark_unread(
        &self,
        account_id: &str,
        folder: &str,
        uids: Vec<u32>,
    ) -> Result<i64, HimalayaError> {
        self.queue(
            account_id,
            ActionType::RemoveFlags {
                folder: folder.to_string(),
                uids,
                flags: vec!["\\Seen".to_string()],
            },
        )
    }

    /// Queue a delete action
    pub fn queue_delete(
        &self,
        account_id: &str,
        folder: &str,
        uids: Vec<u32>,
    ) -> Result<i64, HimalayaError> {
        self.queue(
            account_id,
            ActionType::Delete {
                folder: folder.to_string(),
                uids,
            },
        )
    }

    /// Queue a move action
    pub fn queue_move(
        &self,
        account_id: &str,
        source: &str,
        target: &str,
        uids: Vec<u32>,
    ) -> Result<i64, HimalayaError> {
        self.queue(
            account_id,
            ActionType::Move {
                source_folder: source.to_string(),
                target_folder: target.to_string(),
                uids,
            },
        )
    }

    /// Queue a send action
    pub fn queue_send(
        &self,
        account_id: &str,
        raw_message: Vec<u8>,
        save_to_sent: bool,
    ) -> Result<i64, HimalayaError> {
        self.queue(
            account_id,
            ActionType::Send {
                raw_message,
                save_to_sent,
            },
        )
    }

    /// Get all pending actions for an account
    pub fn get_pending(&self, account_id: &str) -> Result<Vec<QueuedAction>, HimalayaError> {
        let records = self.db.get_pending_actions(account_id)?;

        let mut actions = Vec::new();
        for record in records {
            let action: ActionType = serde_json::from_str(&record.payload).map_err(|e| {
                HimalayaError::Backend(format!("Failed to deserialize action: {}", e))
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
    pub fn has_pending(&self, account_id: &str) -> Result<bool, HimalayaError> {
        let actions = self.db.get_pending_actions(account_id)?;
        Ok(!actions.is_empty())
    }

    /// Mark an action as processing
    pub fn mark_processing(&self, id: i64) -> Result<(), HimalayaError> {
        self.db.update_action_status(id, "processing", None)
    }

    /// Mark an action as completed
    pub fn mark_completed(&self, id: i64) -> Result<(), HimalayaError> {
        self.db.update_action_status(id, "completed", None)
    }

    /// Mark an action as failed
    pub fn mark_failed(&self, id: i64, error: &str) -> Result<(), HimalayaError> {
        self.db.update_action_status(id, "failed", Some(error))
    }

    /// Check if an action should be retried
    pub fn should_retry(&self, action: &QueuedAction) -> bool {
        action.retry_count < self.max_retries
    }

    /// Retry a failed action (reset to pending)
    pub fn retry(&self, id: i64) -> Result<(), HimalayaError> {
        self.db.update_action_status(id, "pending", None)
    }

    /// Clean up completed actions
    pub fn cleanup_completed(&self, account_id: &str) -> Result<u64, HimalayaError> {
        self.db.delete_completed_actions(account_id)
    }

    /// Optimize queue by merging similar actions
    pub fn optimize(&self, account_id: &str) -> Result<(), HimalayaError> {
        let pending = self.get_pending(account_id)?;

        // Group actions by type and folder
        let mut merged: Vec<QueuedAction> = Vec::new();

        for action in pending {
            let mut found_merge = false;

            for existing in &mut merged {
                if existing.action.can_merge(&action.action) {
                    existing.action.merge_uids(&action.action);
                    // Mark the original as completed since it's merged
                    if let Some(id) = action.id {
                        self.mark_completed(id)?;
                    }
                    found_merge = true;
                    break;
                }
            }

            if !found_merge {
                merged.push(action);
            }
        }

        Ok(())
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
