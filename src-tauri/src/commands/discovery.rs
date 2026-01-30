//! Tauri commands for email autodiscovery
//!
//! These commands handle email configuration discovery.

use tracing::info;

use crate::autodiscovery::{
    AuthMethod, DiscoveryPipeline, DiscoveryProgress, DiscoveryStage, EmailDiscoveryConfig,
    Security, UsernameHint,
};
use crate::credentials::CredentialStore;
use crate::services::{create_account, AuthMethod as ServiceAuthMethod, CreateAccountParams};
use crate::types::responses::{DiscoveryResult, ProgressUpdate};
use crate::types::EddieError;

// ============================================================================
// Discovery result conversion
// ============================================================================

impl From<EmailDiscoveryConfig> for DiscoveryResult {
    fn from(config: EmailDiscoveryConfig) -> Self {
        DiscoveryResult {
            provider: config.provider,
            provider_id: config.provider_id,
            imap_host: config.imap.hostname,
            imap_port: config.imap.port,
            imap_tls: matches!(config.imap.security, Security::Tls),
            smtp_host: config.smtp.hostname,
            smtp_port: config.smtp.port,
            smtp_tls: matches!(config.smtp.security, Security::Tls),
            auth_method: match config.auth_method {
                AuthMethod::Password => "password".to_string(),
                AuthMethod::AppPassword => "app_password".to_string(),
            },
            requires_app_password: config.requires_app_password,
            username_hint: match config.username_hint {
                UsernameHint::FullEmail => "full_email".to_string(),
                UsernameHint::LocalPart => "local_part".to_string(),
                UsernameHint::Custom(s) => s,
            },
            source: config.source,
        }
    }
}

impl From<DiscoveryProgress> for ProgressUpdate {
    fn from(progress: DiscoveryProgress) -> Self {
        ProgressUpdate {
            stage: match progress.stage {
                DiscoveryStage::KnownProvider => "known_provider".to_string(),
                DiscoveryStage::Autoconfig => "autoconfig".to_string(),
                DiscoveryStage::Autodiscover => "autodiscover".to_string(),
                DiscoveryStage::Srv => "srv".to_string(),
                DiscoveryStage::Mx => "mx".to_string(),
                DiscoveryStage::Probing => "probing".to_string(),
                DiscoveryStage::Complete => "complete".to_string(),
            },
            progress: progress.progress,
            message: progress.message,
        }
    }
}

// ============================================================================
// Autodiscovery commands
// ============================================================================

/// Discover email configuration for an email address
#[tauri::command]
pub async fn discover_email_config(email: String) -> Result<DiscoveryResult, EddieError> {
    info!("Discovering email config for: {}", email);

    let pipeline = DiscoveryPipeline::new();
    let config = pipeline
        .discover(&email)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(config.into())
}

/// Test connection to discovered email servers
#[tauri::command]
pub async fn test_email_connection(
    email: String,
    _imap_host: String,
    _imap_port: u16,
    _imap_tls: bool,
    _smtp_host: String,
    _smtp_port: u16,
    _smtp_tls: bool,
    _auth_method: String,
    _password: Option<String>,
) -> Result<bool, EddieError> {
    info!("Testing email connection for: {}", email);
    // TODO: Implement actual connection testing
    Ok(true)
}

// ============================================================================
// Credential storage commands
// ============================================================================

/// Store a password securely
#[tauri::command]
pub async fn store_password(email: String, password: String) -> Result<(), EddieError> {
    info!("Storing password for: {}", email);

    let cred_store = CredentialStore::new();
    cred_store
        .store_password(&email, &password)
        .map_err(|e| EddieError::Credential(e.to_string()))
}

/// Store an app-specific password securely (for iCloud, etc.)
#[tauri::command]
pub async fn store_app_password(email: String, password: String) -> Result<(), EddieError> {
    info!("Storing app password for: {}", email);

    let cred_store = CredentialStore::new();
    cred_store
        .store_app_password(&email, &password)
        .map_err(|e| EddieError::Credential(e.to_string()))
}

/// Delete all credentials for an account
#[tauri::command]
pub async fn delete_credentials(email: String) -> Result<(), EddieError> {
    info!("Deleting credentials for: {}", email);

    let cred_store = CredentialStore::new();
    cred_store
        .delete_all_credentials(&email)
        .map_err(|e| EddieError::Credential(e.to_string()))
}

/// Check if credentials exist for an account
#[tauri::command]
pub async fn has_credentials(email: String, credential_type: String) -> Result<bool, EddieError> {
    info!("Checking credentials for {} (type: {})", email, credential_type);

    let cred_store = CredentialStore::new();

    let exists = match credential_type.as_str() {
        "password" => cred_store.has_password(&email),
        "app_password" => cred_store.get_app_password(&email).is_ok(),
        _ => {
            return Err(EddieError::InvalidInput(format!(
                "Unknown credential type: {}",
                credential_type
            )))
        }
    };

    Ok(exists)
}

// ============================================================================
// Account setup with autodiscovery
// ============================================================================

/// Save account with discovered configuration
#[tauri::command]
pub async fn save_discovered_account(
    name: String,
    email: String,
    display_name: Option<String>,
    imap_host: String,
    imap_port: u16,
    imap_tls: bool,
    smtp_host: String,
    smtp_port: u16,
    smtp_tls: bool,
    auth_method: String,
    password: Option<String>,
) -> Result<(), EddieError> {
    info!("Saving discovered account: {}", name);

    let auth = match auth_method.as_str() {
        "app_password" => {
            let pwd = password
                .ok_or_else(|| EddieError::InvalidInput("App password required".into()))?;
            ServiceAuthMethod::AppPassword { password: pwd }
        }
        _ => {
            let pwd =
                password.ok_or_else(|| EddieError::InvalidInput("Password required".into()))?;
            ServiceAuthMethod::Password {
                username: email.clone(),
                password: pwd,
            }
        }
    };

    create_account(CreateAccountParams {
        name,
        email,
        display_name,
        imap_host,
        imap_port,
        imap_tls,
        imap_tls_cert: None,
        smtp_host,
        smtp_port,
        smtp_tls,
        smtp_tls_cert: None,
        auth_method: auth,
    })
}
