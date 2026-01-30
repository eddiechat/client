//! OAuth manager state
//!
//! Manages OAuth2 flows for email provider authentication.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::oauth::OAuthManager;

/// OAuth manager state for Tauri
pub struct OAuthState {
    pub manager: Arc<RwLock<OAuthManager>>,
}

impl OAuthState {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(OAuthManager::new())),
        }
    }
}

impl Default for OAuthState {
    fn default() -> Self {
        Self::new()
    }
}
