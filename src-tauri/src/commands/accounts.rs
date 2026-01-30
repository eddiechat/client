//! Account management Tauri commands
//!
//! Commands for listing, checking, and removing email accounts.

use tracing::info;

use crate::config::{AuthConfig, ImapConfig, SmtpConfig};
use crate::sync::db::{
    delete_connection_config, get_active_connection_config, get_all_connection_configs,
    get_connection_config, init_config_db,
};
use crate::types::{Account, AccountDetails, EddieError};

/// List all configured accounts
#[tauri::command]
pub async fn list_accounts() -> Result<Vec<Account>, EddieError> {
    info!("Listing accounts");

    init_config_db()?;
    let configs = get_all_connection_configs()?;
    let active_config = get_active_connection_config()?;
    let active_id = active_config.map(|c| c.account_id);

    Ok(configs
        .into_iter()
        .map(|config| Account {
            name: config.account_id.clone(),
            is_default: Some(&config.account_id) == active_id.as_ref(),
            backend: if config.imap_config.is_some() {
                "imap".to_string()
            } else {
                "unknown".to_string()
            },
        })
        .collect())
}

/// Get the default account name
#[tauri::command]
pub async fn get_default_account() -> Result<Option<String>, EddieError> {
    info!("Getting default account");

    init_config_db()?;
    let active_config = get_active_connection_config()?;
    Ok(active_config.map(|c| c.account_id))
}

/// Check if an account exists
#[tauri::command]
pub async fn account_exists(name: String) -> Result<bool, EddieError> {
    info!("Checking if account exists: {}", name);

    init_config_db()?;
    let config = get_connection_config(&name)?;
    Ok(config.is_some())
}

/// Remove an account from the configuration
#[tauri::command]
pub async fn remove_account(name: String) -> Result<(), EddieError> {
    info!("Removing account: {}", name);

    init_config_db()?;
    delete_connection_config(&name)?;
    Ok(())
}

/// Get account details for editing
#[tauri::command]
pub async fn get_account_details(name: String) -> Result<AccountDetails, EddieError> {
    info!("Getting account details: {}", name);

    init_config_db()?;
    let db_config = get_connection_config(&name)?
        .ok_or_else(|| EddieError::AccountNotFound(name.clone()))?;

    let imap_config = db_config
        .imap_config
        .as_ref()
        .ok_or_else(|| EddieError::Config("No IMAP configuration found".into()))?;
    let smtp_config = db_config
        .smtp_config
        .as_ref()
        .ok_or_else(|| EddieError::Config("No SMTP configuration found".into()))?;

    let imap: ImapConfig =
        serde_json::from_str(imap_config).map_err(|e| EddieError::Parse(e.to_string()))?;
    let smtp: SmtpConfig =
        serde_json::from_str(smtp_config).map_err(|e| EddieError::Parse(e.to_string()))?;

    let username = match &imap.auth {
        AuthConfig::Password { user, .. } => user.clone(),
        AuthConfig::AppPassword { user } => user.clone(),
    };

    Ok(AccountDetails {
        name: name.clone(),
        email: db_config.email.clone(),
        display_name: db_config.display_name.clone(),
        imap_host: imap.host.clone(),
        imap_port: imap.port,
        imap_tls: imap.tls,
        imap_tls_cert: imap.tls_cert.clone(),
        smtp_host: smtp.host.clone(),
        smtp_port: smtp.port,
        smtp_tls: smtp.tls,
        smtp_tls_cert: smtp.tls_cert.clone(),
        username,
    })
}
