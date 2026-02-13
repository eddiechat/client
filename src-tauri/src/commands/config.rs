//! Configuration Tauri commands
//!
//! Commands for managing application and account configuration.

use serde::Deserialize;
use std::path::PathBuf;
use tauri::State;
use tokio::sync::mpsc;
use tracing::info;

use crate::adapters::sqlite::{self, DbPool};
use crate::config::{self, AuthConfig, EmailAccountConfig, ImapConfig, SmtpConfig};
use crate::encryption::DeviceEncryption;
use crate::sync::db::{
    delete_connection_config, get_active_connection_config, get_all_connection_configs,
    get_app_setting, get_connection_config, init_config_db, save_connection_config,
    set_active_account, set_app_setting, EmailConnectionConfig,
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
    pool: State<'_, DbPool>,
    wake_tx: State<'_, mpsc::Sender<()>>,
) -> Result<(), EddieError> {
    info!("Saving account: {}", request.name);

    init_config_db()?;

    // Determine the encrypted password to use
    let encrypted_password = match &request.password {
        Some(new_password) if !new_password.is_empty() => {
            // New password provided - encrypt it
            info!("Encrypting new password for account: {}", request.email);
            let encryption = DeviceEncryption::new().map_err(|e| {
                EddieError::Config(format!("Failed to initialize encryption: {}", e))
            })?;
            Some(encryption.encrypt(new_password).map_err(|e| {
                EddieError::Config(format!("Failed to encrypt password: {}", e))
            })?)
        }
        _ => {
            // No password provided - check if we're updating an existing account
            if let Some(existing_config) = get_connection_config(&request.email)? {
                info!(
                    "Reusing existing encrypted password for account: {}",
                    request.email
                );
                existing_config.encrypted_password
            } else {
                // New account but no password provided
                return Err(EddieError::InvalidInput(
                    "Password is required when creating a new account".to_string(),
                ));
            }
        }
    };

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

    // Save to Config DB
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

    let account_id = request.email.clone();

    let db_config = EmailConnectionConfig {
        account_id: account_id.clone(),
        active: true,
        email: request.email,
        display_name: request.display_name,
        aliases: request.aliases.clone(),
        imap_config: imap_json,
        smtp_config: smtp_json,
        encrypted_password,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    save_connection_config(&db_config)?;
    set_active_account(&db_config.account_id)?;

    // Ensure account exists in sync DB
    sqlite::accounts::ensure_account(&pool, &account_id)?;

    // Register user entity
    sqlite::entities::insert_entity(&pool, &account_id, &account_id, "account", "user")?;

    // Register alias entities
    if let Some(ref alias_str) = request.aliases {
        for alias in alias_str.split(&[',', ' '][..]) {
            let trimmed = alias.trim();
            if !trimmed.is_empty() {
                sqlite::entities::insert_entity(&pool, &account_id, trimmed, "account", "alias")?;
            }
        }
    }

    // Seed onboarding tasks
    sqlite::onboarding_tasks::seed_tasks(&pool, &account_id)?;

    // Wake the background worker to start syncing
    let _ = wake_tx.send(()).await;

    info!("Account saved and sync initiated for: {}", account_id);
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
    pool: State<'_, DbPool>,
) -> Result<(), EddieError> {
    info!("Deleting account: {}", account_id);

    // Delete account from sync DB — ON DELETE CASCADE removes all child data
    // (messages, conversations, entities, action_queue, sync_state, folder_sync, onboarding_tasks)
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM accounts WHERE id = ?1",
        rusqlite::params![&account_id],
    )?;
    drop(conn);

    // Delete from Config DB
    init_config_db()?;
    delete_connection_config(&account_id)?;

    info!("Account deleted: {}", account_id);
    Ok(())
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
    set_app_setting(
        "read_only_mode",
        if enabled { "true" } else { "false" },
    )
}
