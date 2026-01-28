use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

use crate::config::{self, AccountConfig, AuthConfig, ImapConfig, PasswordSource, SmtpConfig};
use crate::sync::db::{
    self as config_db, get_active_connection_config, get_all_connection_configs,
    get_connection_config, init_config_db, save_connection_config, set_active_account,
    ConnectionConfig,
};

/// Initialize configuration from default paths
#[tauri::command]
pub async fn init_config() -> Result<(), String> {
    info!("Tauri command: init_config");
    config::init_config().map_err(|e| e.to_string())
}

/// Initialize configuration from specific paths
#[tauri::command]
pub async fn init_config_from_paths(paths: Vec<String>) -> Result<(), String> {
    info!("Tauri command: init_config_from_paths - {:?}", paths);

    for path_str in paths {
        let path = PathBuf::from(&path_str);
        if path.exists() {
            return config::init_config_from_path(&path).map_err(|e| e.to_string());
        }
    }

    Err("No valid config file found in provided paths".to_string())
}

/// Check if configuration is initialized
#[tauri::command]
pub async fn is_config_initialized() -> bool {
    config::is_initialized()
}

/// Get configuration file paths (for UI display)
#[tauri::command]
pub async fn get_config_paths() -> Result<Vec<String>, String> {
    Ok(config::default_config_paths()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// Request structure for saving an account
#[derive(Debug, Deserialize)]
pub struct SaveAccountRequest {
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
    pub password: String,
}

/// Save a new account configuration
#[tauri::command]
pub async fn save_account(request: SaveAccountRequest) -> Result<(), String> {
    info!("Tauri command: save_account - name: {}", request.name);

    let imap_auth = AuthConfig::Password {
        user: request.username.clone(),
        password: PasswordSource::Raw(request.password.clone()),
    };

    let smtp_auth = AuthConfig::Password {
        user: request.username,
        password: PasswordSource::Raw(request.password),
    };

    let account = AccountConfig {
        name: Some(request.name.clone()),
        default: true,
        email: request.email,
        display_name: request.display_name,
        imap: Some(ImapConfig {
            host: request.imap_host,
            port: request.imap_port,
            tls: request.imap_tls,
            tls_cert: request.imap_tls_cert,
            auth: imap_auth,
        }),
        smtp: Some(SmtpConfig {
            host: request.smtp_host,
            port: request.smtp_port,
            tls: request.smtp_tls,
            tls_cert: request.smtp_tls_cert,
            auth: smtp_auth,
        }),
    };

    // Save to database
    let imap_json = account
        .imap
        .as_ref()
        .map(|c| serde_json::to_string(c).unwrap_or_default());
    let smtp_json = account
        .smtp
        .as_ref()
        .map(|c| serde_json::to_string(c).unwrap_or_default());

    let db_config = ConnectionConfig {
        account_id: request.name,
        active: true, // New accounts are active by default
        name: account.name,
        email: account.email,
        display_name: account.display_name,
        imap_config: imap_json,
        smtp_config: smtp_json,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Initialize config db if needed and save
    init_config_db().map_err(|e| e.to_string())?;
    save_connection_config(&db_config).map_err(|e| e.to_string())?;
    set_active_account(&db_config.account_id).map_err(|e| e.to_string())?;

    Ok(())
}

// ========== Database-backed Account Commands ==========

/// Response structure for account info
#[derive(Debug, Serialize)]
pub struct AccountInfo {
    pub account_id: String,
    pub active: bool,
    pub name: Option<String>,
    pub email: String,
    pub display_name: Option<String>,
}

impl From<ConnectionConfig> for AccountInfo {
    fn from(config: ConnectionConfig) -> Self {
        Self {
            account_id: config.account_id,
            active: config.active,
            name: config.name,
            email: config.email,
            display_name: config.display_name,
        }
    }
}

/// Initialize the config database
#[tauri::command]
pub async fn init_config_database() -> Result<(), String> {
    info!("Tauri command: init_config_database");
    init_config_db().map_err(|e| e.to_string())
}

/// Get all accounts from the database
#[tauri::command]
pub async fn get_accounts() -> Result<Vec<AccountInfo>, String> {
    info!("Tauri command: get_accounts");
    init_config_db().map_err(|e| e.to_string())?;
    let configs = get_all_connection_configs().map_err(|e| e.to_string())?;
    Ok(configs.into_iter().map(AccountInfo::from).collect())
}

/// Get the currently active account
#[tauri::command]
pub async fn get_active_account() -> Result<Option<AccountInfo>, String> {
    info!("Tauri command: get_active_account");
    init_config_db().map_err(|e| e.to_string())?;
    let config = get_active_connection_config().map_err(|e| e.to_string())?;
    Ok(config.map(AccountInfo::from))
}

/// Set the active account by account_id
#[tauri::command]
pub async fn switch_account(account_id: String) -> Result<(), String> {
    info!("Tauri command: switch_account - {}", account_id);
    init_config_db().map_err(|e| e.to_string())?;

    // Verify the account exists
    let config = get_connection_config(&account_id).map_err(|e| e.to_string())?;
    if config.is_none() {
        return Err(format!("Account '{}' not found", account_id));
    }

    set_active_account(&account_id).map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete an account from the database
#[tauri::command]
pub async fn delete_account(account_id: String) -> Result<(), String> {
    info!("Tauri command: delete_account - {}", account_id);

    // Remove from database
    init_config_db().map_err(|e| e.to_string())?;
    config_db::delete_connection_config(&account_id).map_err(|e| e.to_string())?;

    Ok(())
}
