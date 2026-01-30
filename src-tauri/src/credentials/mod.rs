//! Secure credential storage module
//!
//! Uses platform-native secure storage:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service (GNOME Keyring, KDE Wallet)

use keyring::Entry;
use thiserror::Error;
use tracing::{debug, info};

/// Service name for keyring entries
const SERVICE_NAME: &str = "eddie.chat";

/// Errors that can occur during credential operations
#[derive(Debug, Error)]
#[allow(dead_code)]
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

/// Credential store for secure storage of passwords
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
    #[allow(dead_code)]
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
