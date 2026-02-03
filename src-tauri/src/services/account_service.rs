//! Account management service
//!
//! Business logic for account operations, separated from Tauri commands.

use chrono::Utc;
use std::path::PathBuf;
use tracing::info;

use crate::config::{EmailAccountConfig, AuthConfig, ImapConfig, PasswordSource, SmtpConfig};
use crate::credentials::CredentialStore;
use crate::services::helpers::sanitize_email_for_filename;
use crate::sync::db::{
    delete_connection_config, get_connection_config, init_config_db, save_connection_config,
    set_active_account, EmailConnectionConfig,
};
use crate::types::error::{EddieError, Result};

/// Parameters for creating a new account
pub struct CreateEmailAccountParams {
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
    pub auth_method: AuthMethod,
}

/// Authentication method for account creation
pub enum AuthMethod {
    Password {
        username: String,
        password: String,
    },
    AppPassword {
        password: String,
    },
}

/// Create and save a new account
pub fn create_account(params: CreateEmailAccountParams) -> Result<()> {
    info!("Creating account: {}", params.name);

    // Build auth config based on auth method
    let auth_config = build_auth_config(&params.email, &params.auth_method)?;

    let account_config = EmailAccountConfig {
        name: Some(params.name.clone()),
        default: true,
        email: params.email.clone(),
        display_name: params.display_name.clone(),
        imap: Some(ImapConfig {
            host: params.imap_host,
            port: params.imap_port,
            tls: params.imap_tls,
            tls_cert: params.imap_tls_cert,
            auth: auth_config.clone(),
        }),
        smtp: Some(SmtpConfig {
            host: params.smtp_host,
            port: params.smtp_port,
            tls: params.smtp_tls,
            tls_cert: params.smtp_tls_cert,
            auth: auth_config,
        }),
    };

    save_account_config(&params.email, &params.display_name, &account_config)
}

/// Build auth config from auth method and store credentials
fn build_auth_config(email: &str, auth_method: &AuthMethod) -> Result<AuthConfig> {
    match auth_method {
        AuthMethod::Password { username, password } => {
            // Store password in credential store for secure storage
            let cred_store = CredentialStore::new();
            cred_store
                .store_password(email, password)
                .map_err(|e| EddieError::Credential(e.to_string()))?;

            Ok(AuthConfig::Password {
                user: username.clone(),
                password: PasswordSource::Raw(password.clone()),
            })
        }
        AuthMethod::AppPassword { password } => {
            // Store app password in credential store
            let cred_store = CredentialStore::new();
            cred_store
                .store_app_password(email, password)
                .map_err(|e| EddieError::Credential(e.to_string()))?;

            Ok(AuthConfig::AppPassword {
                user: email.to_string(),
            })
        }
    }
}

/// Save account configuration to database
fn save_account_config(
    email: &str,
    display_name: &Option<String>,
    account: &EmailAccountConfig,
) -> Result<()> {
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
        account_id: email.to_string(),
        active: true,
        email: email.to_string(),
        display_name: display_name.clone(),
        aliases: None, // TODO: Add aliases support to account creation
        imap_config: imap_json,
        smtp_config: smtp_json,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    init_config_db()?;
    save_connection_config(&db_config)?;
    set_active_account(&db_config.account_id)?;

    info!("Account saved successfully: {}", email);
    Ok(())
}

/// Delete an account and its associated data
pub fn delete_account_data(account_id: &str, db_directory: &PathBuf) -> Result<()> {
    info!("Deleting account data for: {}", account_id);

    init_config_db()?;

    // Get the connection config to retrieve the account name
    let db_config = get_connection_config(account_id)?;

    // Delete the sync database file
    if let Some(config) = db_config {
        let safe_name = sanitize_email_for_filename(&config.account_id);
        let db_path = db_directory.join(format!("{}.db", safe_name));

        if db_path.exists() {
            info!("Deleting sync database file: {:?}", db_path);
            std::fs::remove_file(&db_path)?;
        } else {
            info!("Sync database file not found: {:?}", db_path);
        }
    }

    // Remove connection config from database
    delete_connection_config(account_id)?;

    info!("Account deleted successfully: {}", account_id);
    Ok(())
}
