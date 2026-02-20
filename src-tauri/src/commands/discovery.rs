//! Tauri commands for email autodiscovery

use serde::Serialize;
use tracing::info;

use crate::autodiscovery::{
    AuthMethod, DiscoveryPipeline, EmailDiscoveryConfig, Security, UsernameHint,
};
use crate::error::EddieError;

/// Flattened discovery result for the frontend
#[derive(Debug, Serialize)]
pub struct DiscoveryResult {
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    pub auth_method: String,
    pub requires_app_password: bool,
    pub username_hint: String,
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
