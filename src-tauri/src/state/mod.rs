//! Application state management
//!
//! This module contains managed state types for the Tauri application.
//! State types are Tauri-agnostic where possible and focus on thread-safe
//! management of application resources.

mod oauth_state;
mod sync_manager;

pub use oauth_state::OAuthState;
pub use sync_manager::SyncManager;
