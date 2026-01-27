use tracing::info;

use crate::config;
use crate::types::{Account, AccountDetails};

/// List all configured accounts
#[tauri::command]
pub async fn list_accounts() -> Result<Vec<Account>, String> {
    info!("Tauri command: list_accounts");

    let config = config::get_config().map_err(|e| e.to_string())?;
    let default_name = config.default_account_name();

    Ok(config
        .accounts
        .iter()
        .map(|(name, acc)| Account {
            name: name.clone(),
            is_default: Some(name.as_str()) == default_name,
            backend: if acc.imap.is_some() {
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

    let config = config::get_config().map_err(|e| e.to_string())?;
    Ok(config.default_account_name().map(|s| s.to_string()))
}

/// Check if an account exists
#[tauri::command]
pub async fn account_exists(name: String) -> Result<bool, String> {
    info!("Tauri command: account_exists - {}", name);

    let config = config::get_config().map_err(|e| e.to_string())?;
    Ok(config.accounts.contains_key(&name))
}

/// Remove an account from the configuration
#[tauri::command]
pub async fn remove_account(name: String) -> Result<(), String> {
    info!("Tauri command: remove_account - {}", name);

    config::remove_account(&name).map_err(|e| e.to_string())
}

/// Get account details for editing
#[tauri::command]
pub async fn get_account_details(name: String) -> Result<AccountDetails, String> {
    info!("Tauri command: get_account_details - {}", name);

    let config = config::get_config().map_err(|e| e.to_string())?;
    let account = config
        .accounts
        .get(&name)
        .ok_or_else(|| format!("Account '{}' not found", name))?;

    let imap = account
        .imap
        .as_ref()
        .ok_or_else(|| "No IMAP configuration found".to_string())?;
    let smtp = account
        .smtp
        .as_ref()
        .ok_or_else(|| "No SMTP configuration found".to_string())?;

    let username = match &imap.auth {
        config::AuthConfig::Password { user, .. } => user.clone(),
        config::AuthConfig::AppPassword { user } => user.clone(),
        config::AuthConfig::OAuth2 { .. } => account.email.clone(),
    };

    Ok(AccountDetails {
        name: name.clone(),
        email: account.email.clone(),
        display_name: account.display_name.clone(),
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
