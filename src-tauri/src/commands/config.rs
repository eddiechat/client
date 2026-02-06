//! Configuration Tauri commands
//!
//! Commands for managing application and account configuration.

use serde::Deserialize;
use std::path::PathBuf;
use tauri::State;
use tracing::info;

use crate::config::{self, EmailAccountConfig, AuthConfig, ImapConfig, SmtpConfig};
use crate::encryption::DeviceEncryption;
use crate::services::delete_account_data;
use crate::state::SyncManager;
use crate::sync::db::{
    get_active_connection_config, get_all_connection_configs, get_app_setting, get_connection_config,
    init_config_db, save_connection_config, set_active_account, set_app_setting, EmailConnectionConfig,
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
    pub password: Option<String>,
}

/// Save a new account configuration
#[tauri::command]
pub async fn save_account(
    request: SaveEmailAccountRequest,
    manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    info!("Saving account: {}", request.name);

    init_config_db()?;

    // Determine the encrypted password to use
    let encrypted_password = match &request.password {
        Some(new_password) if !new_password.is_empty() => {
            // New password provided - encrypt it
            info!("Encrypting new password for account: {}", request.email);
            let encryption = DeviceEncryption::new()
                .map_err(|e| EddieError::Config(format!("Failed to initialize encryption: {}", e)))?;
            Some(encryption
                .encrypt(new_password)
                .map_err(|e| EddieError::Config(format!("Failed to encrypt password: {}", e)))?)
        }
        _ => {
            // No password provided - check if we're updating an existing account
            if let Some(existing_config) = get_connection_config(&request.email)? {
                info!("Reusing existing encrypted password for account: {}", request.email);
                existing_config.encrypted_password
            } else {
                // New account but no password provided
                return Err(EddieError::InvalidInput(
                    "Password is required when creating a new account".to_string()
                ));
            }
        }
    };

    // Use a placeholder password for the AuthConfig (actual password comes from database)
    let imap_auth = AuthConfig::AppPassword {
        user: request.username.clone(),
    };

    let smtp_auth = AuthConfig::AppPassword {
        user: request.username,
    };

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

    let has_aliases = request.aliases.is_some();
    let account_id = request.email.clone();

    let db_config = EmailConnectionConfig {
        account_id: account_id.clone(),
        active: true,
        email: request.email,
        display_name: request.display_name,
        aliases: request.aliases,
        imap_config: imap_json,
        smtp_config: smtp_json,
        encrypted_password,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    save_connection_config(&db_config)?;
    set_active_account(&db_config.account_id)?;

    // If aliases were provided, reprocess entities to mark recipients as connections
    if has_aliases {
        info!("Aliases provided, reprocessing entities for account: {}", account_id);

        // Remove existing engine so it gets recreated with the new aliases
        manager.remove(&account_id).await;

        // Create new sync engine with updated aliases from database
        if let Ok(engine) = manager.get_or_create(&account_id).await {
            let engine_guard = engine.read().await;

            // Update entities to mark recipients as connections when sender is user or alias
            match engine_guard.reprocess_entities_for_aliases() {
                Ok(count) => info!("Reprocessed {} entity connections for account: {}", count, account_id),
                Err(e) => tracing::warn!("Failed to reprocess entities: {}", e),
            }
        }
    }

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

/// Get the read-only mode setting
#[tauri::command]
pub async fn get_read_only_mode() -> Result<bool, EddieError> {
    info!("Getting read-only mode setting");
    get_app_setting("read_only_mode").map(|s| s == "true")
}

/// Set the read-only mode setting
#[tauri::command]
pub async fn set_read_only_mode(enabled: bool) -> Result<(), EddieError> {
    info!("Setting read-only mode to: {}", enabled);
    set_app_setting("read_only_mode", if enabled { "true" } else { "false" })
}
