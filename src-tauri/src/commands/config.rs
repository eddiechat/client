//! Configuration Tauri commands
//!
//! Commands for managing application and account configuration.

use serde::Deserialize;
use std::path::PathBuf;
use tauri::State;
use tracing::info;

use crate::config::{self, EmailAccountConfig, AuthConfig, CardDAVConfig, ImapConfig, PasswordSource, SmtpConfig};
use crate::services::delete_account_data;
use crate::state::SyncManager;
use crate::sync::db::{
    get_active_connection_config, get_all_connection_configs, get_connection_config, init_config_db,
    save_connection_config, set_active_account, EmailConnectionConfig,
};
use crate::types::responses::EmailAccountInfo;
use crate::types::EddieError;

/// Initialize configuration from default paths
#[tauri::command]
pub async fn init_config() -> Result<(), EddieError> {
    info!("Initializing config from default paths");
    config::init_config().map_err(|e| EddieError::Config(e.to_string()))
}

/// Initialize configuration from specific paths
#[tauri::command]
pub async fn init_config_from_paths(paths: Vec<String>) -> Result<(), EddieError> {
    info!("Initializing config from paths: {:?}", paths);

    for path_str in paths {
        let path = PathBuf::from(&path_str);
        if path.exists() {
            return config::init_config_from_path(&path)
                .map_err(|e| EddieError::Config(e.to_string()));
        }
    }

    Err(EddieError::Config(
        "No valid config file found in provided paths".into(),
    ))
}

/// Check if configuration is initialized
#[tauri::command]
pub async fn is_config_initialized() -> bool {
    config::is_initialized()
}

/// Get configuration file paths (for UI display)
#[tauri::command]
pub async fn get_config_paths() -> Result<Vec<String>, EddieError> {
    Ok(config::default_config_paths()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// Request structure for saving an account
#[derive(Debug, Deserialize)]
pub struct SaveEmailAccountRequest {
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
    // CardDAV settings (optional)
    pub carddav_url: Option<String>,
    pub carddav_tls: Option<bool>,
    pub carddav_tls_cert: Option<String>,
    pub carddav_username: Option<String>,
    pub carddav_password: Option<String>,
}

/// Save a new account configuration
#[tauri::command]
pub async fn save_account(request: SaveEmailAccountRequest) -> Result<(), EddieError> {
    info!("Saving account: {}", request.name);

    let imap_auth = AuthConfig::Password {
        user: request.username.clone(),
        password: PasswordSource::Raw(request.password.clone()),
    };

    let smtp_auth = AuthConfig::Password {
        user: request.username.clone(),
        password: PasswordSource::Raw(request.password.clone()),
    };

    // Build CardDAV config if URL is provided
    let carddav = request.carddav_url.as_ref().map(|url| {
        let carddav_username = request.carddav_username.clone().unwrap_or_else(|| request.username.clone());
        let carddav_password = request.carddav_password.clone().unwrap_or_else(|| request.password.clone());

        CardDAVConfig {
            url: url.clone(),
            tls: request.carddav_tls.unwrap_or(true),
            tls_cert: request.carddav_tls_cert.clone(),
            auth: AuthConfig::Password {
                user: carddav_username,
                password: PasswordSource::Raw(carddav_password),
            },
            address_book: None,
        }
    });

    let account = EmailAccountConfig {
        name: Some(request.name.clone()),
        default: true,
        email: request.email.clone(),
        display_name: request.display_name.clone(),
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
        carddav,
    };

    // Save to database
    let imap_json = account
        .imap
        .as_ref()
        .map(|c| serde_json::to_string(c))
        .transpose()?;
    let smtp_json = account
        .smtp
        .as_ref()
        .map(|c| serde_json::to_string(c))
        .transpose()?;

    let db_config = EmailConnectionConfig {
        account_id: request.email.clone(),
        active: true,
        email: request.email,
        display_name: request.display_name,
        imap_config: imap_json,
        smtp_config: smtp_json,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    init_config_db()?;
    save_connection_config(&db_config)?;
    set_active_account(&db_config.account_id)?;

    Ok(())
}

// ========== Database-backed Account Commands ==========

/// Initialize the config database
#[tauri::command]
pub async fn init_config_database() -> Result<(), EddieError> {
    info!("Initializing config database");
    init_config_db()?;
    Ok(())
}

/// Get all accounts from the database
#[tauri::command]
pub async fn get_accounts() -> Result<Vec<EmailAccountInfo>, EddieError> {
    info!("Getting all accounts");
    init_config_db()?;
    let configs = get_all_connection_configs()?;
    Ok(configs.into_iter().map(EmailAccountInfo::from).collect())
}

/// Get the currently active account
#[tauri::command]
pub async fn get_active_account() -> Result<Option<EmailAccountInfo>, EddieError> {
    info!("Getting active account");
    init_config_db()?;
    let config = get_active_connection_config()?;
    Ok(config.map(EmailAccountInfo::from))
}

/// Set the active account by account_id
#[tauri::command]
pub async fn switch_account(account_id: String) -> Result<(), EddieError> {
    info!("Switching to account: {}", account_id);
    init_config_db()?;

    // Verify the account exists
    let config = get_connection_config(&account_id)?;
    if config.is_none() {
        return Err(EddieError::AccountNotFound(account_id));
    }

    set_active_account(&account_id)?;
    Ok(())
}

/// Delete an account from the database
#[tauri::command]
pub async fn delete_account(
    account_id: String,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    info!("Deleting account: {}", account_id);

    // Shutdown the sync engine if it's running
    sync_manager.remove(&account_id).await;

    // Delete account data (database file and config)
    delete_account_data(&account_id, sync_manager.db_directory())
}
