//! Account management service
//!
//! Business logic for account operations, separated from Tauri commands.

use chrono::Utc;
use tracing::info;

use crate::config::{AuthConfig, EmailAccountConfig, ImapConfig, PasswordSource, SmtpConfig};
use crate::encryption::DeviceEncryption;
use crate::sync::db::{
    init_config_db, save_connection_config, set_active_account, EmailConnectionConfig,
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

/// Create and save a new account to the Config DB.
///
/// Note: This only saves credentials to the Config DB. The sync DB setup
/// (ensure_account, seed_tasks, register entities) happens when
/// `init_sync_engine` is called from the frontend.
pub fn create_account(params: CreateEmailAccountParams) -> Result<()> {
    info!("Creating account: {}", params.name);

    // Extract password from auth method for encryption
    let password = match &params.auth_method {
        AuthMethod::Password { password, .. } => password.clone(),
        AuthMethod::AppPassword { password } => password.clone(),
    };

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

    save_account_config(&params.email, &params.display_name, &account_config, &password)
}

/// Build auth config from auth method
fn build_auth_config(email: &str, auth_method: &AuthMethod) -> Result<AuthConfig> {
    match auth_method {
        AuthMethod::Password { username, password } => Ok(AuthConfig::Password {
            user: username.clone(),
            password: PasswordSource::Raw(password.clone()),
        }),
        AuthMethod::AppPassword { password: _ } => {
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
    password: &str,
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

    // Encrypt password for secure storage
    let encryption = DeviceEncryption::new()
        .map_err(|e| EddieError::Config(format!("Failed to initialize encryption: {}", e)))?;
    let encrypted_password = encryption
        .encrypt(password)
        .map_err(|e| EddieError::Config(format!("Failed to encrypt password: {}", e)))?;

    let db_config = EmailConnectionConfig {
        account_id: email.to_string(),
        active: true,
        email: email.to_string(),
        display_name: display_name.clone(),
        aliases: None,
        imap_config: imap_json,
        smtp_config: smtp_json,
        encrypted_password: Some(encrypted_password),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    init_config_db()?;
    save_connection_config(&db_config)?;
    set_active_account(&db_config.account_id)?;

    info!("Account saved successfully: {}", email);
    Ok(())
}
