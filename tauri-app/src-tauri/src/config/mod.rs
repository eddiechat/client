use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::info;

use crate::types::error::HimalayaError;

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
    /// OAuth2 authentication
    OAuth2 {
        /// OAuth2 client ID
        client_id: String,
        /// OAuth2 client secret (optional)
        client_secret: Option<String>,
        /// OAuth2 token URL
        token_url: String,
        /// OAuth2 authorization URL
        auth_url: String,
        /// OAuth2 scopes
        scopes: Vec<String>,
        /// Refresh token
        refresh_token: Option<String>,
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

impl AppConfig {
    /// Get the default account name
    pub fn default_account_name(&self) -> Option<&str> {
        self.accounts
            .iter()
            .find(|(_, acc)| acc.default)
            .map(|(name, _)| name.as_str())
            .or_else(|| self.accounts.keys().next().map(|s| s.as_str()))
    }

    /// Get account by name, or default if name is None
    pub fn get_account(&self, name: Option<&str>) -> Option<(&str, &AccountConfig)> {
        match name {
            Some(n) => self
                .accounts
                .get_key_value(n)
                .map(|(k, v)| (k.as_str(), v)),
            None => {
                let default_name = self.default_account_name()?;
                self.accounts
                    .get_key_value(default_name)
                    .map(|(k, v)| (k.as_str(), v))
            }
        }
    }
}

/// Get default config paths
pub fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // XDG config path
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("himalaya").join("config.toml"));
    }

    // Home directory fallback
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".config").join("himalaya").join("config.toml"));
        paths.push(home_dir.join(".himalayarc"));
    }

    paths
}

/// Initialize configuration from default paths
pub fn init_config() -> Result<(), HimalayaError> {
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
pub fn init_config_from_path(path: &PathBuf) -> Result<(), HimalayaError> {
    info!("Loading configuration from: {:?}", path);

    let content = fs::read_to_string(path)
        .map_err(|e| HimalayaError::Config(format!("Failed to read config: {}", e)))?;

    let config: AppConfig = toml::from_str(&content)
        .map_err(|e| HimalayaError::Config(format!("Failed to parse config: {}", e)))?;

    set_config(config)
}

/// Set the global configuration
fn set_config(config: AppConfig) -> Result<(), HimalayaError> {
    match CONFIG.get() {
        Some(lock) => {
            let mut guard = lock
                .write()
                .map_err(|e| HimalayaError::Config(format!("Failed to lock config: {}", e)))?;
            *guard = config;
        }
        None => {
            CONFIG.set(RwLock::new(config)).ok();
        }
    }
    Ok(())
}

/// Get a clone of the current configuration
pub fn get_config() -> Result<AppConfig, HimalayaError> {
    CONFIG
        .get()
        .ok_or_else(|| HimalayaError::Config("Configuration not initialized".to_string()))?
        .read()
        .map(|guard| guard.clone())
        .map_err(|e| HimalayaError::Config(format!("Failed to lock config: {}", e)))
}

/// Check if configuration is initialized
pub fn is_initialized() -> bool {
    CONFIG.get().is_some()
}

/// Get the primary config file path (where we write new config)
pub fn get_primary_config_path() -> Result<PathBuf, HimalayaError> {
    dirs::config_dir()
        .map(|p| p.join("himalaya").join("config.toml"))
        .ok_or_else(|| HimalayaError::Config("Cannot determine config directory".to_string()))
}

/// Save the current configuration to file
pub fn save_config_to_file(config: &AppConfig) -> Result<(), HimalayaError> {
    let path = get_primary_config_path()?;

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| HimalayaError::Config(format!("Failed to create config directory: {}", e)))?;
    }

    let content = toml::to_string_pretty(config)
        .map_err(|e| HimalayaError::Config(format!("Failed to serialize config: {}", e)))?;

    fs::write(&path, content)
        .map_err(|e| HimalayaError::Config(format!("Failed to write config file: {}", e)))?;

    info!("Configuration saved to {:?}", path);
    Ok(())
}

/// Add or update an account in the configuration and save to file
pub fn save_account(name: String, account: AccountConfig) -> Result<(), HimalayaError> {
    let mut config = get_config()?;
    config.accounts.insert(name, account);

    // Update in-memory config
    set_config(config.clone())?;

    // Persist to file
    save_config_to_file(&config)?;

    Ok(())
}

/// Remove an account from the configuration and save to file
pub fn remove_account(name: &str) -> Result<(), HimalayaError> {
    let mut config = get_config()?;

    if !config.accounts.contains_key(name) {
        return Err(HimalayaError::Config(format!(
            "Account '{}' not found",
            name
        )));
    }

    config.accounts.remove(name);

    // Update in-memory config
    set_config(config.clone())?;

    // Persist to file
    save_config_to_file(&config)?;

    info!("Account '{}' removed", name);
    Ok(())
}
