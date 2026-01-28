use tracing::info;

use crate::config::{AuthConfig, ImapConfig, SmtpConfig};
use crate::sync::db::{
    delete_connection_config, get_active_connection_config, get_all_connection_configs,
    get_connection_config, init_config_db,
};
use crate::types::{Account, AccountDetails};

/// List all configured accounts
#[tauri::command]
pub async fn list_accounts() -> Result<Vec<Account>, String> {
    info!("Tauri command: list_accounts");

    init_config_db().map_err(|e| e.to_string())?;
    let configs = get_all_connection_configs().map_err(|e| e.to_string())?;
    let active_config = get_active_connection_config().map_err(|e| e.to_string())?;
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
pub async fn get_default_account() -> Result<Option<String>, String> {
    info!("Tauri command: get_default_account");

    init_config_db().map_err(|e| e.to_string())?;
    let active_config = get_active_connection_config().map_err(|e| e.to_string())?;
    Ok(active_config.map(|c| c.account_id))
}

/// Check if an account exists
#[tauri::command]
pub async fn account_exists(name: String) -> Result<bool, String> {
    info!("Tauri command: account_exists - {}", name);

    init_config_db().map_err(|e| e.to_string())?;
    let config = get_connection_config(&name).map_err(|e| e.to_string())?;
    Ok(config.is_some())
}

/// Remove an account from the configuration
#[tauri::command]
pub async fn remove_account(name: String) -> Result<(), String> {
    info!("Tauri command: remove_account - {}", name);

    init_config_db().map_err(|e| e.to_string())?;
    delete_connection_config(&name).map_err(|e| e.to_string())
}

/// Get account details for editing
#[tauri::command]
pub async fn get_account_details(name: String) -> Result<AccountDetails, String> {
    info!("Tauri command: get_account_details - {}", name);

    init_config_db().map_err(|e| e.to_string())?;
    let db_config = get_connection_config(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Account '{}' not found", name))?;

    let imap_config = db_config
        .imap_config
        .as_ref()
        .ok_or_else(|| "No IMAP configuration found".to_string())?;
    let smtp_config = db_config
        .smtp_config
        .as_ref()
        .ok_or_else(|| "No SMTP configuration found".to_string())?;

    let imap: ImapConfig = serde_json::from_str(imap_config)
        .map_err(|e| format!("Failed to parse IMAP config: {}", e))?;
    let smtp: SmtpConfig = serde_json::from_str(smtp_config)
        .map_err(|e| format!("Failed to parse SMTP config: {}", e))?;

    let username = match &imap.auth {
        AuthConfig::Password { user, .. } => user.clone(),
        AuthConfig::AppPassword { user } => user.clone(),
        AuthConfig::OAuth2 { .. } => db_config.email.clone(),
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
