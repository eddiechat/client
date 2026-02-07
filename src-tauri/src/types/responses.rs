//! Response types for Tauri commands
//!
//! These types are serialized and sent to the frontend.
//! They should be lean and contain only what the frontend needs.

use serde::{Deserialize, Serialize};

use crate::sync::db::{CachedConversation, CachedChatMessage, Entity};
use crate::sync::engine::{SyncState, SyncStatus};

// ============================================================================
// Sync Response Types
// ============================================================================

/// Response for sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusResponse {
    pub state: String,
    pub account_id: String,
    pub current_folder: Option<String>,
    pub progress_current: Option<u32>,
    pub progress_total: Option<u32>,
    pub progress_message: Option<String>,
    pub last_sync: Option<String>,
    pub error: Option<String>,
    pub is_online: bool,
    pub pending_actions: u32,
    pub monitor_mode: Option<String>,
}

impl From<SyncStatus> for SyncStatusResponse {
    fn from(s: SyncStatus) -> Self {
        Self {
            state: match s.state {
                SyncState::Idle => "idle".to_string(),
                SyncState::Connecting => "connecting".to_string(),
                SyncState::Syncing => "syncing".to_string(),
                SyncState::InitialSync => "initial_sync".to_string(),
                SyncState::Error => "error".to_string(),
            },
            account_id: s.account_id,
            current_folder: s.current_folder,
            progress_current: s.progress.as_ref().map(|p| p.current),
            progress_total: s.progress.as_ref().and_then(|p| p.total),
            progress_message: s.progress.map(|p| p.message),
            last_sync: s.last_sync.map(|d| d.to_rfc3339()),
            error: s.error,
            is_online: s.is_online,
            pending_actions: s.pending_actions,
            monitor_mode: s.monitor_mode,
        }
    }
}

impl Default for SyncStatusResponse {
    fn default() -> Self {
        Self {
            state: "idle".to_string(),
            account_id: String::new(),
            current_folder: None,
            progress_current: None,
            progress_total: None,
            progress_message: None,
            last_sync: None,
            error: None,
            is_online: false,
            pending_actions: 0,
            monitor_mode: None,
        }
    }
}

impl SyncStatusResponse {
    /// Create an idle status for a given account
    pub fn idle(account_id: String) -> Self {
        Self {
            account_id,
            ..Default::default()
        }
    }
}

// ============================================================================
// Conversation Response Types
// ============================================================================

/// Cached conversation for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationResponse {
    pub id: i64,
    pub participant_key: String,
    pub participants: Vec<ParticipantInfo>,
    pub last_message_date: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_from: Option<String>,
    pub message_count: u32,
    pub unread_count: u32,
    pub is_outgoing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub email: String,
    pub name: Option<String>,
}

impl From<CachedConversation> for ConversationResponse {
    fn from(c: CachedConversation) -> Self {
        let participants: Vec<ParticipantInfo> =
            serde_json::from_str(&c.participants).unwrap_or_default();

        Self {
            id: c.id,
            participant_key: c.participant_key,
            participants,
            last_message_date: c.last_message_date.map(|d| d.to_rfc3339()),
            last_message_preview: c.last_message_preview,
            last_message_from: c.last_message_from,
            message_count: c.message_count,
            unread_count: c.unread_count,
            is_outgoing: c.is_outgoing,
        }
    }
}

// ============================================================================
// Entity Response Types (for autocomplete)
// ============================================================================

/// Entity response for autocomplete suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResponse {
    pub id: i64,
    pub email: String,
    pub name: Option<String>,
    pub is_connection: bool,
    pub latest_contact: String,
    pub contact_count: u32,
}

impl From<Entity> for EntityResponse {
    fn from(e: Entity) -> Self {
        Self {
            id: e.id,
            email: e.email,
            name: e.name,
            is_connection: e.is_connection,
            latest_contact: e.latest_contact.to_rfc3339(),
            contact_count: e.contact_count,
        }
    }
}

// ============================================================================
// Message Response Types
// ============================================================================

/// Cached message for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedChatMessageResponse {
    pub id: i64,
    pub folder: String,
    pub uid: u32,
    pub message_id: Option<String>,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub subject: Option<String>,
    pub date: Option<String>,
    pub flags: Vec<String>,
    pub has_attachment: bool,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub body_cached: bool,
}

impl From<CachedChatMessage> for CachedChatMessageResponse {
    fn from(m: CachedChatMessage) -> Self {
        Self {
            id: m.id,
            folder: m.folder_name,
            uid: m.uid,
            message_id: m.message_id,
            from_address: m.from_address,
            from_name: m.from_name,
            to_addresses: serde_json::from_str(&m.to_addresses).unwrap_or_default(),
            cc_addresses: m
                .cc_addresses
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default(),
            subject: m.subject,
            date: m.date.map(|d| d.to_rfc3339()),
            flags: serde_json::from_str(&m.flags).unwrap_or_default(),
            has_attachment: m.has_attachment,
            text_body: m.text_body,
            html_body: m.html_body,
            body_cached: m.body_cached,
        }
    }
}

// ============================================================================
// Ollama / Settings Response Types
// ============================================================================

/// Ollama configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaSettingsResponse {
    pub url: String,
    pub model: String,
    pub enabled: bool,
}

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
