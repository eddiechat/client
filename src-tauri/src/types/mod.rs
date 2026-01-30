pub mod conversation;
pub mod error;
pub mod responses;

#[allow(unused_imports)]
pub use error::{EddieError, Result};
#[allow(unused_imports)]
pub use responses::*;

use serde::{Deserialize, Serialize};

/// Represents an email envelope (metadata) for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub id: String,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub date: String,
    pub flags: Vec<String>,
    pub has_attachment: bool,
}

/// Represents a folder/mailbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub name: String,
    pub desc: Option<String>,
}

/// Represents an email message with full content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub envelope: Envelope,
    pub headers: Vec<(String, String)>,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub attachments: Vec<Attachment>,
}

/// Represents an email attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: Option<String>,
    pub mime_type: String,
    pub size: usize,
}

/// Represents an attachment to be sent with a composed message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeAttachment {
    pub path: String,
    pub name: String,
    pub mime_type: String,
    pub size: usize,
}

/// Represents an account configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    pub is_default: bool,
    pub backend: String,
}

/// Account details for editing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDetails {
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub imap_tls_cert: Option<String>,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    pub smtp_tls_cert: Option<String>,
    pub username: String,
}

/// List envelopes request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEnvelopesRequest {
    pub account: Option<String>,
    pub folder: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub query: Option<String>,
}

/// List envelopes response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEnvelopesResponse {
    pub envelopes: Vec<Envelope>,
    pub page: usize,
    pub page_size: usize,
    pub total: Option<usize>,
}

/// Read message request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadMessageRequest {
    pub account: Option<String>,
    pub folder: Option<String>,
    pub id: String,
    pub preview: bool,
}

/// Flag operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagRequest {
    pub account: Option<String>,
    pub folder: Option<String>,
    pub ids: Vec<String>,
    pub flags: Vec<String>,
}
