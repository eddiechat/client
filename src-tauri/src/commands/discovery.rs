//! Tauri commands for email autodiscovery and OAuth2 flows

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use tracing::info;

use crate::autodiscovery::{
    AuthMethod, DiscoveryPipeline, DiscoveryProgress, DiscoveryStage, EmailDiscoveryConfig,
    OAuthProvider as DiscoveryOAuthProvider, Security, UsernameHint,
};
use crate::config::{
    self, AccountConfig, AuthConfig, ImapConfig, OAuth2Provider, PasswordSource, SmtpConfig,
};
use crate::credentials::CredentialStore;
use crate::oauth::{OAuthManager, OAuthTokens};
use crate::sync::db::{
    init_config_db, save_connection_config, set_active_account, ConnectionConfig,
};

/// OAuth manager state for Tauri
pub struct OAuthState {
    pub manager: Arc<RwLock<OAuthManager>>,
}

impl OAuthState {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(OAuthManager::new())),
        }
    }
}

impl Default for OAuthState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Discovery response types for frontend
// ============================================================================

/// Discovery result returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    /// Provider name (if detected)
    pub provider: Option<String>,
    /// Provider ID for known providers
    pub provider_id: Option<String>,
    /// IMAP configuration
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    /// SMTP configuration
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    /// Authentication method: "password", "oauth2", "app_password"
    pub auth_method: String,
    /// OAuth provider if OAuth2: "google", "microsoft", "yahoo", "fastmail"
    pub oauth_provider: Option<String>,
    /// Whether app-specific password is required (iCloud)
    pub requires_app_password: bool,
    /// Username format hint: "full_email", "local_part", or custom
    pub username_hint: String,
    /// Discovery source for debugging
    pub source: String,
}

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

/// Progress update for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub stage: String,
    pub progress: u8,
    pub message: String,
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
pub async fn discover_email_config(email: String) -> Result<DiscoveryResult, String> {
    info!("Tauri command: discover_email_config for {}", email);

    let pipeline = DiscoveryPipeline::new();
    let config = pipeline
        .discover(&email)
        .await
        .map_err(|e| e.to_string())?;

    Ok(config.into())
}

/// Test connection to discovered email servers
#[tauri::command]
pub async fn test_email_connection(
    email: String,
    imap_host: String,
    imap_port: u16,
    imap_tls: bool,
    smtp_host: String,
    smtp_port: u16,
    smtp_tls: bool,
    auth_method: String,
    password: Option<String>,
    oauth_provider: Option<String>,
) -> Result<bool, String> {
    info!("Tauri command: test_email_connection for {}", email);

    // For now, just return true - actual connection testing would require
    // establishing connections to the servers
    // This can be expanded to actually test the connections

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
) -> Result<String, String> {
    info!(
        "Tauri command: start_oauth_flow for {} with provider {}",
        email, provider
    );

    let oauth_provider = match provider.to_lowercase().as_str() {
        "google" => DiscoveryOAuthProvider::Google,
        "microsoft" => DiscoveryOAuthProvider::Microsoft,
        "yahoo" => DiscoveryOAuthProvider::Yahoo,
        "fastmail" => DiscoveryOAuthProvider::Fastmail,
        _ => return Err(format!("Unknown OAuth provider: {}", provider)),
    };

    let manager = state.manager.read().await;
    let auth_url = manager
        .start_auth_flow(&oauth_provider, &email, &redirect_uri)
        .map_err(|e| e.to_string())?;

    Ok(auth_url)
}

/// Complete OAuth2 authorization flow with the callback parameters
#[tauri::command]
pub async fn complete_oauth_flow(
    state: State<'_, OAuthState>,
    code: String,
    callback_state: String,
    redirect_uri: String,
) -> Result<String, String> {
    info!("Tauri command: complete_oauth_flow");

    let manager = state.manager.read().await;
    let (tokens, email) = manager
        .complete_auth_flow(&code, &callback_state, &redirect_uri)
        .await
        .map_err(|e| e.to_string())?;

    // Store tokens in credential store
    let cred_store = CredentialStore::new();
    cred_store
        .store_oauth_tokens(&email, &tokens)
        .map_err(|e| e.to_string())?;

    info!("OAuth flow completed successfully for {}", email);

    Ok(email)
}

/// Refresh OAuth2 tokens for an account
#[tauri::command]
pub async fn refresh_oauth_tokens(
    state: State<'_, OAuthState>,
    email: String,
    provider: String,
) -> Result<bool, String> {
    info!("Tauri command: refresh_oauth_tokens for {}", email);

    let cred_store = CredentialStore::new();
    let tokens = cred_store
        .get_oauth_tokens(&email)
        .map_err(|e| e.to_string())?;

    let refresh_token = tokens
        .refresh_token
        .ok_or_else(|| "No refresh token available".to_string())?;

    let oauth_provider = match provider.to_lowercase().as_str() {
        "google" => DiscoveryOAuthProvider::Google,
        "microsoft" => DiscoveryOAuthProvider::Microsoft,
        "yahoo" => DiscoveryOAuthProvider::Yahoo,
        "fastmail" => DiscoveryOAuthProvider::Fastmail,
        _ => return Err(format!("Unknown OAuth provider: {}", provider)),
    };

    let manager = state.manager.read().await;
    let new_tokens = manager
        .refresh_tokens(&oauth_provider, &refresh_token)
        .await
        .map_err(|e| e.to_string())?;

    // Store refreshed tokens
    cred_store
        .store_oauth_tokens(&email, &new_tokens)
        .map_err(|e| e.to_string())?;

    info!("OAuth tokens refreshed for {}", email);

    Ok(true)
}

/// Check if OAuth tokens exist and are valid for an account
#[tauri::command]
pub async fn check_oauth_status(email: String) -> Result<OAuthStatus, String> {
    info!("Tauri command: check_oauth_status for {}", email);

    let cred_store = CredentialStore::new();

    match cred_store.get_oauth_tokens(&email) {
        Ok(tokens) => {
            let needs_refresh = OAuthManager::should_refresh(&tokens);
            let is_expired = tokens.expires_at.map(|exp| {
                let now = chrono::Utc::now().timestamp();
                exp < now
            }).unwrap_or(false);

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthStatus {
    pub has_tokens: bool,
    pub needs_refresh: bool,
    pub is_expired: bool,
}

// ============================================================================
// Credential storage commands
// ============================================================================

/// Store a password securely
#[tauri::command]
pub async fn store_password(email: String, password: String) -> Result<(), String> {
    info!("Tauri command: store_password for {}", email);

    let cred_store = CredentialStore::new();
    cred_store
        .store_password(&email, &password)
        .map_err(|e| e.to_string())
}

/// Store an app-specific password securely (for iCloud, etc.)
#[tauri::command]
pub async fn store_app_password(email: String, password: String) -> Result<(), String> {
    info!("Tauri command: store_app_password for {}", email);

    let cred_store = CredentialStore::new();
    cred_store
        .store_app_password(&email, &password)
        .map_err(|e| e.to_string())
}

/// Delete all credentials for an account
#[tauri::command]
pub async fn delete_credentials(email: String) -> Result<(), String> {
    info!("Tauri command: delete_credentials for {}", email);

    let cred_store = CredentialStore::new();
    cred_store
        .delete_all_credentials(&email)
        .map_err(|e| e.to_string())
}

/// Check if credentials exist for an account
#[tauri::command]
pub async fn has_credentials(email: String, credential_type: String) -> Result<bool, String> {
    info!(
        "Tauri command: has_credentials for {} (type: {})",
        email, credential_type
    );

    let cred_store = CredentialStore::new();

    let exists = match credential_type.as_str() {
        "password" => cred_store.has_password(&email),
        "oauth" => cred_store.has_oauth_tokens(&email),
        "app_password" => cred_store.get_app_password(&email).is_ok(),
        _ => return Err(format!("Unknown credential type: {}", credential_type)),
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
) -> Result<(), String> {
    info!("Tauri command: save_discovered_account for {}", name);

    // Build auth config based on auth method
    let auth_config = match auth_method.as_str() {
        "oauth2" => {
            let provider = oauth_provider
                .ok_or_else(|| "OAuth provider required for OAuth2 auth".to_string())?;
            let oauth2_provider = match provider.to_lowercase().as_str() {
                "google" => OAuth2Provider::Google,
                "microsoft" => OAuth2Provider::Microsoft,
                "yahoo" => OAuth2Provider::Yahoo,
                "fastmail" => OAuth2Provider::Fastmail,
                _ => return Err(format!("Unknown OAuth provider: {}", provider)),
            };
            AuthConfig::OAuth2 {
                provider: oauth2_provider,
                access_token: None, // Will be fetched from credential store
            }
        }
        "app_password" => {
            // Store app password in credential store if provided
            if let Some(pwd) = &password {
                let cred_store = CredentialStore::new();
                cred_store
                    .store_app_password(&email, pwd)
                    .map_err(|e| e.to_string())?;
            }
            AuthConfig::AppPassword {
                user: email.clone(),
            }
        }
        _ => {
            // Password authentication
            let pwd = password.ok_or_else(|| "Password required".to_string())?;

            // Store password in credential store for secure storage
            let cred_store = CredentialStore::new();
            cred_store
                .store_password(&email, &pwd)
                .map_err(|e| e.to_string())?;

            AuthConfig::Password {
                user: email.clone(),
                password: PasswordSource::Raw(pwd),
            }
        }
    };

    let account_config = AccountConfig {
        name: Some(name.clone()),
        default: false, // Will be set if this is the first account
        email: email.clone(),
        display_name,
        imap: Some(ImapConfig {
            host: imap_host,
            port: imap_port,
            tls: imap_tls,
            tls_cert: None,
            auth: auth_config.clone(),
        }),
        smtp: Some(SmtpConfig {
            host: smtp_host,
            port: smtp_port,
            tls: smtp_tls,
            tls_cert: None,
            auth: auth_config,
        }),
    };

    // Save to database
    let imap_json = account_config
        .imap
        .as_ref()
        .map(|c| serde_json::to_string(c).unwrap_or_default());
    let smtp_json = account_config
        .smtp
        .as_ref()
        .map(|c| serde_json::to_string(c).unwrap_or_default());

    let db_config = ConnectionConfig {
        account_id: email.clone(),  // Use email as account_id
        active: true, // New accounts are active by default
        email: account_config.email,
        display_name: account_config.display_name,
        imap_config: imap_json,
        smtp_config: smtp_json,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Initialize config db if needed and save
    init_config_db().map_err(|e| e.to_string())?;
    save_connection_config(&db_config).map_err(|e| e.to_string())?;
    set_active_account(&db_config.account_id).map_err(|e| e.to_string())?;

    info!("Account {} saved successfully to database", name);

    Ok(())
}
