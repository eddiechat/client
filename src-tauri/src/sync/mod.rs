//! IMAP Sync Engine
//!
//! This module provides a local SQLite cache of email messages, synchronized with IMAP servers.
//! The local database is a cache of server state, not the source of truth.
//!
//! Key principles:
//! - UI renders exclusively from SQLite, never directly from IMAP responses
//! - All user actions execute on the IMAP/SMTP server first
//! - UI updates only after the next sync confirms the server state changed
//! - Server wins all conflicts

pub mod db;
pub mod engine;
pub mod capability;
pub mod action_queue;
pub mod conversation;
pub mod classifier;
pub mod idle;

pub use db::SyncDatabase;
pub use engine::SyncEngine;
pub use capability::{ServerCapability, CapabilityDetector};
pub use action_queue::{ActionQueue, QueuedAction, ActionType};
pub use conversation::ConversationGrouper;
pub use classifier::MessageClassifier;
