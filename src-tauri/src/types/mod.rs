pub mod error;
pub mod responses;

#[allow(unused_imports)]
pub use error::{EddieError, Result};
#[allow(unused_imports)]
pub use responses::*;

use serde::{Deserialize, Serialize};

/// Represents an account configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAccount {
    pub name: String,
    pub is_default: bool,
    pub backend: String,
}

/// Account details for editing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAccountDetails {
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub aliases: Option<String>,
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

/// Represents an attachment to be sent with a composed message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeAttachment {
    pub path: String,
    pub name: String,
    pub mime_type: String,
    pub size: usize,
}
