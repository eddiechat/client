use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::info;

use crate::types::error::EddieError;

/// Global configuration instance
static CONFIG: OnceCell<RwLock<AppConfig>> = OnceCell::new();

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Map of account name to account configuration
    #[serde(default)]
    pub accounts: HashMap<String, AccountConfig>,
}

/// Account configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    /// Display name for the account
    pub name: Option<String>,

    /// Whether this is the default account
    #[serde(default)]
    pub default: bool,

    /// Email address
    pub email: String,

    /// Display name for sent emails
    pub display_name: Option<String>,

    /// IMAP configuration for receiving
    pub imap: Option<ImapConfig>,

    /// SMTP configuration for sending
    pub smtp: Option<SmtpConfig>,
}

/// IMAP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    /// IMAP server hostname
    pub host: String,

    /// IMAP server port (default: 993 for TLS, 143 for STARTTLS)
    #[serde(default = "default_imap_port")]
    pub port: u16,

    /// Use TLS encryption
    #[serde(default = "default_true")]
    pub tls: bool,

    /// Path to custom certificate for TLS validation (for self-signed certs)
    pub tls_cert: Option<String>,

    /// Authentication method
    pub auth: AuthConfig,
}

/// SMTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// SMTP server hostname
    pub host: String,

    /// SMTP server port (default: 465 for TLS, 587 for STARTTLS)
    #[serde(default = "default_smtp_port")]
    pub port: u16,

    /// Use TLS encryption
    #[serde(default = "default_true")]
    pub tls: bool,

    /// Path to custom certificate for TLS validation (for self-signed certs)
    pub tls_cert: Option<String>,

    /// Authentication method
    pub auth: AuthConfig,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    /// Password authentication
    Password {
        /// Username (usually email address)
        user: String,
        /// Password (can use command for keychain integration)
        password: PasswordSource,
    },
    /// App-specific password (for iCloud, etc.)
    AppPassword {
        /// Username (usually email address)
        user: String,
    },
}

/// Password source - can be raw value or command to execute
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PasswordSource {
    /// Raw password value
    Raw(String),
    /// Command to execute to get password
    Command { command: String },
}

fn default_imap_port() -> u16 {
    993
}

fn default_smtp_port() -> u16 {
    465
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }
}

impl AppConfig {}

/// Get default config paths
pub fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // XDG config path
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("eddie.chat").join("config.toml"));
    }

    // Home directory fallback
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(
            home_dir
                .join(".config")
                .join("eddie.chat")
                .join("config.toml"),
        );
        paths.push(home_dir.join(".eddie.chat.rc"));
    }

    paths
}

/// Initialize configuration from default paths
pub fn init_config() -> Result<(), EddieError> {
    info!("Initializing configuration from default paths");

    for path in default_config_paths() {
        if path.exists() {
            info!("Found config at: {:?}", path);
            return init_config_from_path(&path);
        }
    }

    // No config found, initialize with empty config
    info!("No config file found, using empty config");
    set_config(AppConfig::default())
}

/// Initialize configuration from a specific path
pub fn init_config_from_path(path: &PathBuf) -> Result<(), EddieError> {
    info!("Loading configuration from: {:?}", path);

    let content = fs::read_to_string(path)
        .map_err(|e| EddieError::Config(format!("Failed to read config: {}", e)))?;

    let config: AppConfig = toml::from_str(&content)
        .map_err(|e| EddieError::Config(format!("Failed to parse config: {}", e)))?;

    set_config(config)
}

/// Set the global configuration
fn set_config(config: AppConfig) -> Result<(), EddieError> {
    match CONFIG.get() {
        Some(lock) => {
            let mut guard = lock
                .write()
                .map_err(|e| EddieError::Config(format!("Failed to lock config: {}", e)))?;
            *guard = config;
        }
        None => {
            CONFIG.set(RwLock::new(config)).ok();
        }
    }
    Ok(())
}

/// Check if configuration is initialized
pub fn is_initialized() -> bool {
    CONFIG.get().is_some()
}
