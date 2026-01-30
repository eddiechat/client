//! Tauri command modules
//!
//! This module contains all Tauri command handlers organized by domain.
//! Commands are thin wrappers that delegate to services for business logic.
//!
//! ## Module Organization
//!
//! - `accounts`: Account listing and management
//! - `config`: Application and account configuration
//! - `conversations`: Conversation listing (deprecated - use sync)
//! - `discovery`: Email autodiscovery and OAuth2
//! - `envelopes`: Email envelope listing (deprecated - use sync)
//! - `flags`: Message flag operations
//! - `folders`: Folder/mailbox operations
//! - `messages`: Message reading, sending, and management
//! - `sync`: Sync engine operations (recommended)

pub mod accounts;
pub mod config;
pub mod conversations;
pub mod discovery;
pub mod envelopes;
pub mod flags;
pub mod folders;
pub mod messages;
pub mod sync;

// Re-export all commands for convenience
pub use accounts::*;
pub use config::*;
pub use conversations::*;
pub use discovery::*;
pub use envelopes::*;
pub use flags::*;
pub use folders::*;
pub use messages::*;
pub use sync::*;
