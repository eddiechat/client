//! Application state management
//!
//! This module contains managed state types for the Tauri application.
//! State types are Tauri-agnostic where possible and focus on thread-safe
//! management of application resources.

mod sync_manager;

pub use sync_manager::SyncManager;
