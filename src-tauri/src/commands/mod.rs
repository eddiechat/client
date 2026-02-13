//! Tauri command modules
//!
//! This module contains all Tauri command handlers organized by domain.
//! Commands are thin wrappers that delegate to services for business logic.
//!
//! ## Module Organization
//!
//! - `accounts`: Account listing and management
//! - `app`: Application-level information and utilities
//! - `config`: Application and account configuration
//! - `discovery`: Email autodiscovery
//! - `sync`: Sync engine operations

pub mod accounts;
pub mod app;
pub mod config;
pub mod discovery;
pub mod sync;

// Re-export all commands for convenience
pub use accounts::*;
pub use app::*;
pub use config::*;
pub use discovery::*;
pub use sync::*;
