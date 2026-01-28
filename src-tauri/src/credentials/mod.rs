//! Secure credential storage module
//!
//! Uses platform-native secure storage:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service (GNOME Keyring, KDE Wallet)

use keyring::Entry;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::oauth::OAuthTokens;

/// Service name for keyring entries
const SERVICE_NAME: &str = "eddie.chat";

/// Errors that can occur during credential operations
#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("Keyring error: {0}")]
    Keyring(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Credential not found: {0}")]
    NotFound(String),
}

impl From<keyring::Error> for CredentialError {
    fn from(err: keyring::Error) -> Self {
        match err {
            keyring::Error::NoEntry => CredentialError::NotFound("No entry found".to_string()),
            _ => CredentialError::Keyring(err.to_string()),
        }
    }
}

/// Credential store for secure storage of passwords and OAuth tokens
pub struct CredentialStore {
    service: String,
}

impl CredentialStore {
    /// Create a new credential store
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME.to_string(),
        }
    }

    /// Create a credential store with a custom service name
    pub fn with_service(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }

    // ========================================================================
    // Password storage
    // ========================================================================

    /// Store a password for an email account
    pub fn store_password(&self, email: &str, password: &str) -> Result<(), CredentialError> {
        let key = format!("password:{}", email);
        let entry = Entry::new(&self.service, &key)?;
        entry.set_password(password)?;
        info!("Stored password for {}", email);
        Ok(())
    }

    /// Get a password for an email account
    pub fn get_password(&self, email: &str) -> Result<String, CredentialError> {
        let key = format!("password:{}", email);
        let entry = Entry::new(&self.service, &key)?;
        let password = entry.get_password()?;
        debug!("Retrieved password for {}", email);
        Ok(password)
    }

    /// Delete a password for an email account
    pub fn delete_password(&self, email: &str) -> Result<(), CredentialError> {
        let key = format!("password:{}", email);
        let entry = Entry::new(&self.service, &key)?;
        entry.delete_credential()?;
        info!("Deleted password for {}", email);
        Ok(())
    }

    /// Check if a password exists for an email account
    pub fn has_password(&self, email: &str) -> bool {
        self.get_password(email).is_ok()
    }

    // ========================================================================
    // OAuth token storage
    // ========================================================================

    /// Store OAuth tokens for an email account
    pub fn store_oauth_tokens(&self, email: &str, tokens: &OAuthTokens) -> Result<(), CredentialError> {
        let key = format!("oauth:{}", email);
        let entry = Entry::new(&self.service, &key)?;

        let json = serde_json::to_string(tokens)
            .map_err(|e| CredentialError::Serialization(e.to_string()))?;

        entry.set_password(&json)?;
        info!("Stored OAuth tokens for {}", email);
        Ok(())
    }

    /// Get OAuth tokens for an email account
    pub fn get_oauth_tokens(&self, email: &str) -> Result<OAuthTokens, CredentialError> {
        let key = format!("oauth:{}", email);
        let entry = Entry::new(&self.service, &key)?;

        let json = entry.get_password()?;
        let tokens: OAuthTokens = serde_json::from_str(&json)
            .map_err(|e| CredentialError::Serialization(e.to_string()))?;

        debug!("Retrieved OAuth tokens for {}", email);
        Ok(tokens)
    }

    /// Delete OAuth tokens for an email account
    pub fn delete_oauth_tokens(&self, email: &str) -> Result<(), CredentialError> {
        let key = format!("oauth:{}", email);
        let entry = Entry::new(&self.service, &key)?;
        entry.delete_credential()?;
        info!("Deleted OAuth tokens for {}", email);
        Ok(())
    }

    /// Check if OAuth tokens exist for an email account
    pub fn has_oauth_tokens(&self, email: &str) -> bool {
        self.get_oauth_tokens(email).is_ok()
    }

    /// Update only the access token (after refresh)
    pub fn update_access_token(
        &self,
        email: &str,
        access_token: &str,
        expires_at: Option<i64>,
    ) -> Result<(), CredentialError> {
        let mut tokens = self.get_oauth_tokens(email)?;
        tokens.access_token = access_token.to_string();
        tokens.expires_at = expires_at;
        self.store_oauth_tokens(email, &tokens)
    }

    // ========================================================================
    // App-specific password storage (for iCloud, etc.)
    // ========================================================================

    /// Store an app-specific password
    pub fn store_app_password(&self, email: &str, password: &str) -> Result<(), CredentialError> {
        let key = format!("app_password:{}", email);
        let entry = Entry::new(&self.service, &key)?;
        entry.set_password(password)?;
        info!("Stored app-specific password for {}", email);
        Ok(())
    }

    /// Get an app-specific password
    pub fn get_app_password(&self, email: &str) -> Result<String, CredentialError> {
        let key = format!("app_password:{}", email);
        let entry = Entry::new(&self.service, &key)?;
        let password = entry.get_password()?;
        debug!("Retrieved app-specific password for {}", email);
        Ok(password)
    }

    /// Delete an app-specific password
    pub fn delete_app_password(&self, email: &str) -> Result<(), CredentialError> {
        let key = format!("app_password:{}", email);
        let entry = Entry::new(&self.service, &key)?;
        entry.delete_credential()?;
        info!("Deleted app-specific password for {}", email);
        Ok(())
    }

    // ========================================================================
    // Account cleanup
    // ========================================================================

    /// Delete all credentials for an email account
    pub fn delete_all_credentials(&self, email: &str) -> Result<(), CredentialError> {
        // Try to delete each type of credential, ignoring "not found" errors
        let _ = self.delete_password(email);
        let _ = self.delete_oauth_tokens(email);
        let _ = self.delete_app_password(email);
        info!("Deleted all credentials for {}", email);
        Ok(())
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Cached credentials for OAuth
// ============================================================================

/// Cached OAuth credentials with refresh handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedOAuthCredentials {
    pub email: String,
    pub provider: String,
    pub tokens: OAuthTokens,
}

impl CachedOAuthCredentials {
    /// Check if the access token is expired or about to expire
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.tokens.expires_at {
            let now = chrono::Utc::now().timestamp();
            // Consider expired if less than 60 seconds remaining
            expires_at - now < 60
        } else {
            // No expiry info, assume not expired
            false
        }
    }

    /// Check if the token should be proactively refreshed
    pub fn should_refresh(&self) -> bool {
        if let Some(expires_at) = self.tokens.expires_at {
            let now = chrono::Utc::now().timestamp();
            // Refresh if less than 5 minutes remaining
            expires_at - now < 300
        } else {
            false
        }
    }
}
