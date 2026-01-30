//! Tauri commands for email autodiscovery and OAuth2 flows
//!
//! These commands handle email configuration discovery and OAuth2 authentication.

use tauri::State;
use tracing::info;

use crate::autodiscovery::{
    AuthMethod, DiscoveryPipeline, DiscoveryProgress, DiscoveryStage, EmailDiscoveryConfig,
    OAuthProvider as DiscoveryOAuthProvider, Security, UsernameHint,
};
use crate::credentials::CredentialStore;
use crate::oauth::OAuthManager;
use crate::services::{
    create_account, parse_oauth_provider, AuthMethod as ServiceAuthMethod, CreateAccountParams,
};
use crate::state::OAuthState;
use crate::types::responses::{DiscoveryResult, OAuthStatus, ProgressUpdate};
use crate::types::EddieError;

// Re-export OAuthState for backward compatibility
pub use crate::state::OAuthState as OAuthStateType;

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
                AuthMethod::OAuth2 => "oauth2".to_string(),
                AuthMethod::AppPassword => "app_password".to_string(),
            },
            oauth_provider: config.oauth_provider.map(|p| match p {
                DiscoveryOAuthProvider::Google => "google".to_string(),
                DiscoveryOAuthProvider::Microsoft => "microsoft".to_string(),
                DiscoveryOAuthProvider::Yahoo => "yahoo".to_string(),
                DiscoveryOAuthProvider::Fastmail => "fastmail".to_string(),
            }),
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
    _oauth_provider: Option<String>,
) -> Result<bool, EddieError> {
    info!("Testing email connection for: {}", email);
    // TODO: Implement actual connection testing
    Ok(true)
}

// ============================================================================
// OAuth2 commands
// ============================================================================

/// Start OAuth2 authorization flow
/// Returns the authorization URL to open in browser
#[tauri::command]
pub async fn start_oauth_flow(
    state: State<'_, OAuthState>,
    provider: String,
    email: String,
    redirect_uri: String,
) -> Result<String, EddieError> {
    info!("Starting OAuth flow for {} with provider {}", email, provider);

    let oauth_provider = parse_discovery_oauth_provider(&provider)?;
    let manager = state.manager.read().await;
    let auth_url = manager
        .start_auth_flow(&oauth_provider, &email, &redirect_uri)
        .map_err(|e| EddieError::OAuth(e.to_string()))?;

    Ok(auth_url)
}

/// Complete OAuth2 authorization flow with the callback parameters
#[tauri::command]
pub async fn complete_oauth_flow(
    state: State<'_, OAuthState>,
    code: String,
    callback_state: String,
    redirect_uri: String,
) -> Result<String, EddieError> {
    info!("Completing OAuth flow");

    let manager = state.manager.read().await;
    let (tokens, email) = manager
        .complete_auth_flow(&code, &callback_state, &redirect_uri)
        .await
        .map_err(|e| EddieError::OAuth(e.to_string()))?;

    // Store tokens in credential store
    let cred_store = CredentialStore::new();
    cred_store
        .store_oauth_tokens(&email, &tokens)
        .map_err(|e| EddieError::Credential(e.to_string()))?;

    info!("OAuth flow completed successfully for {}", email);
    Ok(email)
}

/// Refresh OAuth2 tokens for an account
#[tauri::command]
pub async fn refresh_oauth_tokens(
    state: State<'_, OAuthState>,
    email: String,
    provider: String,
) -> Result<bool, EddieError> {
    info!("Refreshing OAuth tokens for: {}", email);

    let cred_store = CredentialStore::new();
    let tokens = cred_store
        .get_oauth_tokens(&email)
        .map_err(|e| EddieError::Credential(e.to_string()))?;

    let refresh_token = tokens
        .refresh_token
        .ok_or_else(|| EddieError::OAuth("No refresh token available".into()))?;

    let oauth_provider = parse_discovery_oauth_provider(&provider)?;
    let manager = state.manager.read().await;
    let new_tokens = manager
        .refresh_tokens(&oauth_provider, &refresh_token)
        .await
        .map_err(|e| EddieError::OAuth(e.to_string()))?;

    // Store refreshed tokens
    cred_store
        .store_oauth_tokens(&email, &new_tokens)
        .map_err(|e| EddieError::Credential(e.to_string()))?;

    info!("OAuth tokens refreshed for {}", email);
    Ok(true)
}

/// Check if OAuth tokens exist and are valid for an account
#[tauri::command]
pub async fn check_oauth_status(email: String) -> Result<OAuthStatus, EddieError> {
    info!("Checking OAuth status for: {}", email);

    let cred_store = CredentialStore::new();

    match cred_store.get_oauth_tokens(&email) {
        Ok(tokens) => {
            let needs_refresh = OAuthManager::should_refresh(&tokens);
            let is_expired = tokens
                .expires_at
                .map(|exp| {
                    let now = chrono::Utc::now().timestamp();
                    exp < now
                })
                .unwrap_or(false);

            Ok(OAuthStatus {
                has_tokens: true,
                needs_refresh,
                is_expired,
            })
        }
        Err(_) => Ok(OAuthStatus {
            has_tokens: false,
            needs_refresh: false,
            is_expired: false,
        }),
    }
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
        "oauth" => cred_store.has_oauth_tokens(&email),
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
    oauth_provider: Option<String>,
    password: Option<String>,
) -> Result<(), EddieError> {
    info!("Saving discovered account: {}", name);

    let auth = match auth_method.as_str() {
        "oauth2" => {
            let provider = oauth_provider
                .ok_or_else(|| EddieError::InvalidInput("OAuth provider required".into()))?;
            ServiceAuthMethod::OAuth2 { provider }
        }
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

// ============================================================================
// Helper functions
// ============================================================================

/// Parse discovery OAuth provider from string
fn parse_discovery_oauth_provider(provider: &str) -> Result<DiscoveryOAuthProvider, EddieError> {
    match provider.to_lowercase().as_str() {
        "google" => Ok(DiscoveryOAuthProvider::Google),
        "microsoft" => Ok(DiscoveryOAuthProvider::Microsoft),
        "yahoo" => Ok(DiscoveryOAuthProvider::Yahoo),
        "fastmail" => Ok(DiscoveryOAuthProvider::Fastmail),
        _ => Err(EddieError::InvalidInput(format!(
            "Unknown OAuth provider: {}",
            provider
        ))),
    }
}
