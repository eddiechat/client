//! OAuth2 authentication module
//!
//! Implements OAuth2 with PKCE for email providers:
//! - Google (Gmail)
//! - Microsoft (Outlook/Office 365)
//! - Yahoo
//! - Fastmail

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::autodiscovery::OAuthProvider;

/// Errors that can occur during OAuth operations
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("OAuth configuration error: {0}")]
    Configuration(String),

    #[error("OAuth request failed: {0}")]
    Request(String),

    #[error("Token exchange failed: {0}")]
    TokenExchange(String),

    #[error("Invalid state parameter")]
    InvalidState,

    #[error("Provider not supported: {0}")]
    UnsupportedProvider(String),

    #[error("No pending OAuth flow found")]
    NoPendingFlow,

    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),
}

/// OAuth2 tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    /// Access token for API requests
    pub access_token: String,
    /// Refresh token for obtaining new access tokens
    pub refresh_token: Option<String>,
    /// Token expiration time (Unix timestamp)
    pub expires_at: Option<i64>,
    /// Scopes granted
    pub scopes: Vec<String>,
}

/// OAuth2 provider configuration
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Client ID (must be registered with provider)
    pub client_id: String,
    /// Client secret (optional for public clients using PKCE)
    pub client_secret: Option<String>,
    /// Authorization endpoint URL
    pub auth_url: String,
    /// Token endpoint URL
    pub token_url: String,
    /// Scopes required for IMAP/SMTP access
    pub scopes: Vec<String>,
}

/// Pending OAuth flow state
#[derive(Debug)]
struct PendingOAuthFlow {
    provider: OAuthProvider,
    email: String,
    pkce_verifier: PkceCodeVerifier,
    csrf_token: String,
}

/// OAuth2 manager for handling authentication flows
pub struct OAuthManager {
    /// Pending OAuth flows keyed by state parameter
    pending_flows: RwLock<HashMap<String, PendingOAuthFlow>>,
    /// Custom client configurations (for user-provided client IDs)
    custom_configs: RwLock<HashMap<String, ProviderConfig>>,
}

impl OAuthManager {
    /// Create a new OAuth manager
    pub fn new() -> Self {
        Self {
            pending_flows: RwLock::new(HashMap::new()),
            custom_configs: RwLock::new(HashMap::new()),
        }
    }

    /// Set a custom OAuth configuration for a provider
    pub fn set_custom_config(&self, provider: &OAuthProvider, config: ProviderConfig) {
        let key = provider_key(provider);
        self.custom_configs.write().unwrap().insert(key, config);
    }

    /// Get provider configuration
    pub fn get_provider_config(&self, provider: &OAuthProvider) -> Result<ProviderConfig, OAuthError> {
        // Check for custom config first
        let key = provider_key(provider);
        if let Some(config) = self.custom_configs.read().unwrap().get(&key) {
            return Ok(config.clone());
        }

        // Use default configuration
        get_default_provider_config(provider)
    }

    /// Start an OAuth2 authorization flow
    ///
    /// Returns the authorization URL to open in the browser
    pub fn start_auth_flow(
        &self,
        provider: &OAuthProvider,
        email: &str,
        redirect_uri: &str,
    ) -> Result<String, OAuthError> {
        let config = self.get_provider_config(provider)?;

        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate CSRF state token
        let csrf_token = generate_state_token();

        // Build the OAuth2 client
        let client = BasicClient::new(ClientId::new(config.client_id.clone()))
            .set_auth_uri(
                AuthUrl::new(config.auth_url.clone())
                    .map_err(|e| OAuthError::Configuration(e.to_string()))?,
            )
            .set_token_uri(
                TokenUrl::new(config.token_url.clone())
                    .map_err(|e| OAuthError::Configuration(e.to_string()))?,
            )
            .set_redirect_uri(
                RedirectUrl::new(redirect_uri.to_string())
                    .map_err(|e| OAuthError::Configuration(e.to_string()))?,
            );

        // Add client secret if available
        let client = if let Some(secret) = &config.client_secret {
            client.set_client_secret(ClientSecret::new(secret.clone()))
        } else {
            client
        };

        // Build authorization URL with PKCE
        let mut auth_request = client
            .authorize_url(|| CsrfToken::new(csrf_token.clone()))
            .set_pkce_challenge(pkce_challenge);

        // Add scopes
        for scope in &config.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        // Add provider-specific parameters
        let auth_url = match provider {
            OAuthProvider::Google => {
                // Google needs login_hint and access_type for refresh tokens
                auth_request
                    .add_extra_param("login_hint", email)
                    .add_extra_param("access_type", "offline")
                    .add_extra_param("prompt", "consent")
                    .url()
                    .0
                    .to_string()
            }
            OAuthProvider::Microsoft => {
                // Microsoft needs login_hint
                auth_request
                    .add_extra_param("login_hint", email)
                    .url()
                    .0
                    .to_string()
            }
            OAuthProvider::Yahoo => {
                auth_request.url().0.to_string()
            }
            OAuthProvider::Fastmail => {
                auth_request.url().0.to_string()
            }
        };

        // Store pending flow
        self.pending_flows.write().unwrap().insert(
            csrf_token.clone(),
            PendingOAuthFlow {
                provider: provider.clone(),
                email: email.to_string(),
                pkce_verifier,
                csrf_token: csrf_token.clone(),
            },
        );

        info!("Started OAuth flow for {} with state {}", email, csrf_token);

        Ok(auth_url)
    }

    /// Complete an OAuth2 authorization flow with the authorization code
    pub async fn complete_auth_flow(
        &self,
        code: &str,
        state: &str,
        redirect_uri: &str,
    ) -> Result<(OAuthTokens, String), OAuthError> {
        // Get and remove pending flow
        let pending = self
            .pending_flows
            .write()
            .unwrap()
            .remove(state)
            .ok_or(OAuthError::NoPendingFlow)?;

        // Verify state matches
        if pending.csrf_token != state {
            return Err(OAuthError::InvalidState);
        }

        let config = self.get_provider_config(&pending.provider)?;

        info!(
            "Completing OAuth flow for {} (provider: {:?})",
            pending.email, pending.provider
        );

        // Exchange authorization code for tokens
        let tokens = exchange_code_for_tokens(
            &config,
            code,
            &pending.pkce_verifier,
            redirect_uri,
        )
        .await?;

        Ok((tokens, pending.email))
    }

    /// Refresh an access token using a refresh token
    pub async fn refresh_tokens(
        &self,
        provider: &OAuthProvider,
        refresh_token: &str,
    ) -> Result<OAuthTokens, OAuthError> {
        let config = self.get_provider_config(provider)?;

        info!("Refreshing OAuth tokens for provider {:?}", provider);

        refresh_access_token(&config, refresh_token).await
    }

    /// Build XOAUTH2 SASL string for IMAP/SMTP authentication
    pub fn build_xoauth2_string(email: &str, access_token: &str) -> String {
        // Format: base64("user=" + email + "\x01auth=Bearer " + access_token + "\x01\x01")
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        URL_SAFE_NO_PAD.encode(auth_string.as_bytes())
    }

    /// Check if tokens need refresh (within 5 minutes of expiry)
    pub fn should_refresh(tokens: &OAuthTokens) -> bool {
        if let Some(expires_at) = tokens.expires_at {
            let now = chrono::Utc::now().timestamp();
            // Refresh if less than 5 minutes remaining
            expires_at - now < 300
        } else {
            // No expiry info, assume we should refresh
            true
        }
    }
}

impl Default for OAuthManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Provider configurations
// ============================================================================

/// Get default provider configuration
///
/// Note: These use placeholder client IDs that need to be replaced
/// with properly registered application credentials
fn get_default_provider_config(provider: &OAuthProvider) -> Result<ProviderConfig, OAuthError> {
    match provider {
        OAuthProvider::Google => Ok(ProviderConfig {
            // Users must register their own app and set this
            client_id: std::env::var("EDDIE_GOOGLE_CLIENT_ID")
                .unwrap_or_else(|_| "YOUR_GOOGLE_CLIENT_ID".to_string()),
            client_secret: std::env::var("EDDIE_GOOGLE_CLIENT_SECRET").ok(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            // Gmail scope covers both IMAP and SMTP
            scopes: vec!["https://mail.google.com/".to_string()],
        }),

        OAuthProvider::Microsoft => Ok(ProviderConfig {
            client_id: std::env::var("EDDIE_MICROSOFT_CLIENT_ID")
                .unwrap_or_else(|_| "YOUR_MICROSOFT_CLIENT_ID".to_string()),
            client_secret: std::env::var("EDDIE_MICROSOFT_CLIENT_SECRET").ok(),
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
            scopes: vec![
                "https://outlook.office.com/IMAP.AccessAsUser.All".to_string(),
                "https://outlook.office.com/SMTP.Send".to_string(),
                "offline_access".to_string(),
            ],
        }),

        OAuthProvider::Yahoo => Ok(ProviderConfig {
            client_id: std::env::var("EDDIE_YAHOO_CLIENT_ID")
                .unwrap_or_else(|_| "YOUR_YAHOO_CLIENT_ID".to_string()),
            client_secret: std::env::var("EDDIE_YAHOO_CLIENT_SECRET").ok(),
            auth_url: "https://api.login.yahoo.com/oauth2/request_auth".to_string(),
            token_url: "https://api.login.yahoo.com/oauth2/get_token".to_string(),
            // Yahoo uses different scopes - these need verification
            scopes: vec!["mail-r".to_string(), "mail-w".to_string()],
        }),

        OAuthProvider::Fastmail => Ok(ProviderConfig {
            client_id: std::env::var("EDDIE_FASTMAIL_CLIENT_ID")
                .unwrap_or_else(|_| "YOUR_FASTMAIL_CLIENT_ID".to_string()),
            client_secret: std::env::var("EDDIE_FASTMAIL_CLIENT_SECRET").ok(),
            auth_url: "https://www.fastmail.com/oauth/authorize".to_string(),
            token_url: "https://api.fastmail.com/oauth/token".to_string(),
            scopes: vec![
                "https://www.fastmail.com/dev/protocol-imap".to_string(),
                "https://www.fastmail.com/dev/protocol-smtp".to_string(),
            ],
        }),
    }
}

fn provider_key(provider: &OAuthProvider) -> String {
    match provider {
        OAuthProvider::Google => "google".to_string(),
        OAuthProvider::Microsoft => "microsoft".to_string(),
        OAuthProvider::Yahoo => "yahoo".to_string(),
        OAuthProvider::Fastmail => "fastmail".to_string(),
    }
}

// ============================================================================
// Token exchange
// ============================================================================

/// Exchange authorization code for tokens
async fn exchange_code_for_tokens(
    config: &ProviderConfig,
    code: &str,
    pkce_verifier: &PkceCodeVerifier,
    redirect_uri: &str,
) -> Result<OAuthTokens, OAuthError> {
    let client = reqwest::Client::new();

    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", &config.client_id),
        ("code_verifier", pkce_verifier.secret()),
    ];

    // Add client secret if available (confidential clients)
    let secret_str;
    if let Some(secret) = &config.client_secret {
        secret_str = secret.clone();
        params.push(("client_secret", &secret_str));
    }

    debug!("Exchanging authorization code for tokens");

    let response = client
        .post(&config.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| OAuthError::Request(e.to_string()))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Token exchange failed: {}", error_text);
        return Err(OAuthError::TokenExchange(error_text));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;

    let expires_at = token_response.expires_in.map(|secs| {
        chrono::Utc::now().timestamp() + secs as i64
    });

    Ok(OAuthTokens {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        expires_at,
        scopes: config.scopes.clone(),
    })
}

/// Refresh an access token
async fn refresh_access_token(
    config: &ProviderConfig,
    refresh_token: &str,
) -> Result<OAuthTokens, OAuthError> {
    let client = reqwest::Client::new();

    let mut params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", &config.client_id),
    ];

    // Add client secret if available
    let secret_str;
    if let Some(secret) = &config.client_secret {
        secret_str = secret.clone();
        params.push(("client_secret", &secret_str));
    }

    debug!("Refreshing access token");

    let response = client
        .post(&config.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| OAuthError::Request(e.to_string()))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Token refresh failed: {}", error_text);
        return Err(OAuthError::RefreshFailed(error_text));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| OAuthError::RefreshFailed(e.to_string()))?;

    let expires_at = token_response.expires_in.map(|secs| {
        chrono::Utc::now().timestamp() + secs as i64
    });

    // Some providers don't return a new refresh token
    let new_refresh_token = token_response
        .refresh_token
        .unwrap_or_else(|| refresh_token.to_string());

    Ok(OAuthTokens {
        access_token: token_response.access_token,
        refresh_token: Some(new_refresh_token),
        expires_at,
        scopes: config.scopes.clone(),
    })
}

// ============================================================================
// Token response parsing
// ============================================================================

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    #[serde(default)]
    token_type: String,
}

// ============================================================================
// Helper functions
// ============================================================================

/// Generate a cryptographically secure state token
fn generate_state_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    URL_SAFE_NO_PAD.encode(bytes)
}
