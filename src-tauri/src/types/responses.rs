//! Response types for Tauri commands
//!
//! These types are serialized and sent to the frontend.
//! They should be lean and contain only what the frontend needs.

use serde::{Deserialize, Serialize};

// ============================================================================
// Account Response Types
// ============================================================================

/// Response structure for account info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAccountInfo {
    pub account_id: String,
    pub active: bool,
    pub email: String,
    pub display_name: Option<String>,
}

impl From<crate::sync::db::EmailConnectionConfig> for EmailAccountInfo {
    fn from(config: crate::sync::db::EmailConnectionConfig) -> Self {
        Self {
            account_id: config.account_id,
            active: config.active,
            email: config.email,
            display_name: config.display_name,
        }
    }
}

// ============================================================================
// Discovery Response Types
// ============================================================================

/// Discovery result returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    /// Provider name (if detected)
    pub provider: Option<String>,
    /// Provider ID for known providers
    pub provider_id: Option<String>,
    /// IMAP configuration
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    /// SMTP configuration
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    /// Authentication method: "password", "app_password"
    pub auth_method: String,
    /// Whether app-specific password is required (iCloud, Gmail, Yahoo)
    pub requires_app_password: bool,
    /// Username format hint: "full_email", "local_part", or custom
    pub username_hint: String,
    /// Discovery source for debugging
    pub source: String,
}

/// Progress update for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProgressUpdate {
    pub stage: String,
    pub progress: u8,
    pub message: String,
}

// ============================================================================
// Attachment Response Types
// ============================================================================

/// Attachment info for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub index: usize,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
}
