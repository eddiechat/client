//! Email autodiscovery module
//!
//! This module implements multiple autodiscovery protocols:
//! - Mozilla Autoconfig (ISPDB)
//! - Microsoft Autodiscover v2
//! - DNS SRV records (RFC 6186)
//! - MX record analysis for provider detection
//! - Heuristic server probing

mod autoconfig;
mod dns;
mod providers;
mod probe;

pub use autoconfig::*;
pub use dns::*;
pub use providers::*;
pub use probe::*;

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::info;

/// Errors that can occur during autodiscovery
#[derive(Debug, Error)]
pub enum AutodiscoveryError {
    #[error("Invalid email address: {0}")]
    InvalidEmail(String),

    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("XML parsing failed: {0}")]
    XmlError(#[from] quick_xml::DeError),

    #[error("DNS lookup failed: {0}")]
    DnsError(String),

    #[error("No configuration found for domain: {0}")]
    NotFound(String),

    #[error("Connection test failed: {0}")]
    ConnectionFailed(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Security type for email connections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    /// No encryption
    None,
    /// STARTTLS upgrade
    Starttls,
    /// Implicit TLS (SSL)
    Tls,
}

impl Default for Security {
    fn default() -> Self {
        Self::Tls
    }
}

/// Authentication method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// Password authentication
    Password,
    /// App-specific password (iCloud, Yahoo)
    AppPassword,
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::Password
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server hostname
    pub hostname: String,
    /// Server port
    pub port: u16,
    /// Security type
    pub security: Security,
}

/// Complete email configuration discovered for an account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDiscoveryConfig {
    /// Provider name (if known)
    pub provider: Option<String>,
    /// Provider ID for known providers (e.g., "gmail.com", "outlook.com")
    pub provider_id: Option<String>,
    /// IMAP server configuration
    pub imap: ServerConfig,
    /// SMTP server configuration
    pub smtp: ServerConfig,
    /// Recommended authentication method
    pub auth_method: AuthMethod,
    /// Username format hint
    pub username_hint: UsernameHint,
    /// Whether app-specific password is required (for iCloud)
    pub requires_app_password: bool,
    /// Discovery source (autoconfig, autodiscover, srv, mx, probe)
    pub source: String,
}

/// Hint for username format
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsernameHint {
    /// Full email address (user@example.com)
    FullEmail,
    /// Local part only (user)
    LocalPart,
    /// Custom format
    Custom(String),
}

impl Default for UsernameHint {
    fn default() -> Self {
        Self::FullEmail
    }
}

/// Progress updates during autodiscovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryProgress {
    /// Current stage
    pub stage: DiscoveryStage,
    /// Progress percentage (0-100)
    pub progress: u8,
    /// Human-readable message
    pub message: String,
}

/// Stages of the autodiscovery process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryStage {
    /// Checking known providers
    KnownProvider,
    /// Trying Mozilla Autoconfig
    Autoconfig,
    /// Trying Microsoft Autodiscover
    Autodiscover,
    /// Querying DNS SRV records
    Srv,
    /// Analyzing MX records
    Mx,
    /// Probing common server patterns
    Probing,
    /// Discovery complete
    Complete,
}

/// Main autodiscovery pipeline
pub struct DiscoveryPipeline {
    http_client: reqwest::Client,
}

impl DiscoveryPipeline {
    /// Create a new discovery pipeline
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .user_agent("eddie.chat/1.0 (Email Client)")
            .build()
            .unwrap_or_default();

        Self { http_client }
    }

    /// Discover email configuration for an email address
    pub async fn discover(&self, email: &str) -> Result<EmailDiscoveryConfig, AutodiscoveryError> {
        // Validate and parse email
        let email = email.trim().to_lowercase();
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(AutodiscoveryError::InvalidEmail(email));
        }
        let domain = parts[1];

        info!("Starting autodiscovery for domain: {}", domain);

        // Step 1: Check known providers first (fastest)
        if let Some(config) = providers::check_known_provider(&email, domain) {
            info!("Found known provider configuration");
            return Ok(config);
        }

        // Step 2: Try Mozilla Autoconfig (parallel with Autodiscover)
        let autoconfig_future = autoconfig::try_mozilla_autoconfig(&self.http_client, &email, domain);
        let autodiscover_future = autoconfig::try_microsoft_autodiscover(&self.http_client, &email, domain);

        // Run autoconfig and autodiscover in parallel
        let (autoconfig_result, autodiscover_result) = tokio::join!(
            tokio::time::timeout(Duration::from_secs(8), autoconfig_future),
            tokio::time::timeout(Duration::from_secs(8), autodiscover_future),
        );

        // Check autoconfig result first (more common for general domains)
        if let Ok(Ok(config)) = autoconfig_result {
            info!("Found Mozilla Autoconfig configuration");
            return Ok(config);
        }

        // Check autodiscover result
        if let Ok(Ok(config)) = autodiscover_result {
            info!("Found Microsoft Autodiscover configuration");
            return Ok(config);
        }

        // Step 3: Try DNS SRV records
        if let Ok(config) = dns::try_srv_records(domain).await {
            info!("Found DNS SRV configuration");
            return Ok(config);
        }

        // Step 4: Analyze MX records for provider detection
        if let Ok(config) = dns::try_mx_analysis(domain).await {
            info!("Detected provider from MX records");
            return Ok(config);
        }

        // Step 5: Heuristic probing as last resort
        if let Ok(config) = probe::probe_common_servers(domain).await {
            info!("Found configuration via server probing");
            return Ok(config);
        }

        Err(AutodiscoveryError::NotFound(domain.to_string()))
    }

    /// Discover with progress updates via a callback
    pub async fn discover_with_progress<F>(
        &self,
        email: &str,
        mut on_progress: F,
    ) -> Result<EmailDiscoveryConfig, AutodiscoveryError>
    where
        F: FnMut(DiscoveryProgress),
    {
        // Validate and parse email
        let email = email.trim().to_lowercase();
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(AutodiscoveryError::InvalidEmail(email));
        }
        let domain = parts[1];

        // Step 1: Known providers
        on_progress(DiscoveryProgress {
            stage: DiscoveryStage::KnownProvider,
            progress: 10,
            message: "Checking known email providers...".to_string(),
        });

        if let Some(config) = providers::check_known_provider(&email, domain) {
            on_progress(DiscoveryProgress {
                stage: DiscoveryStage::Complete,
                progress: 100,
                message: format!("Found configuration for {}", config.provider.as_deref().unwrap_or(domain)),
            });
            return Ok(config);
        }

        // Step 2: Mozilla Autoconfig
        on_progress(DiscoveryProgress {
            stage: DiscoveryStage::Autoconfig,
            progress: 25,
            message: "Trying Mozilla Autoconfig...".to_string(),
        });

        if let Ok(Ok(config)) = tokio::time::timeout(
            Duration::from_secs(8),
            autoconfig::try_mozilla_autoconfig(&self.http_client, &email, domain),
        ).await {
            on_progress(DiscoveryProgress {
                stage: DiscoveryStage::Complete,
                progress: 100,
                message: "Found configuration via Autoconfig".to_string(),
            });
            return Ok(config);
        }

        // Step 3: Microsoft Autodiscover
        on_progress(DiscoveryProgress {
            stage: DiscoveryStage::Autodiscover,
            progress: 40,
            message: "Trying Microsoft Autodiscover...".to_string(),
        });

        if let Ok(Ok(config)) = tokio::time::timeout(
            Duration::from_secs(8),
            autoconfig::try_microsoft_autodiscover(&self.http_client, &email, domain),
        ).await {
            on_progress(DiscoveryProgress {
                stage: DiscoveryStage::Complete,
                progress: 100,
                message: "Found configuration via Autodiscover".to_string(),
            });
            return Ok(config);
        }

        // Step 4: DNS SRV
        on_progress(DiscoveryProgress {
            stage: DiscoveryStage::Srv,
            progress: 55,
            message: "Querying DNS SRV records...".to_string(),
        });

        if let Ok(config) = dns::try_srv_records(domain).await {
            on_progress(DiscoveryProgress {
                stage: DiscoveryStage::Complete,
                progress: 100,
                message: "Found configuration via DNS SRV".to_string(),
            });
            return Ok(config);
        }

        // Step 5: MX analysis
        on_progress(DiscoveryProgress {
            stage: DiscoveryStage::Mx,
            progress: 70,
            message: "Analyzing MX records...".to_string(),
        });

        if let Ok(config) = dns::try_mx_analysis(domain).await {
            on_progress(DiscoveryProgress {
                stage: DiscoveryStage::Complete,
                progress: 100,
                message: format!("Detected provider from MX records: {}", config.provider.as_deref().unwrap_or("Unknown")),
            });
            return Ok(config);
        }

        // Step 6: Heuristic probing
        on_progress(DiscoveryProgress {
            stage: DiscoveryStage::Probing,
            progress: 85,
            message: "Probing common server configurations...".to_string(),
        });

        if let Ok(config) = probe::probe_common_servers(domain).await {
            on_progress(DiscoveryProgress {
                stage: DiscoveryStage::Complete,
                progress: 100,
                message: "Found configuration via server probing".to_string(),
            });
            return Ok(config);
        }

        Err(AutodiscoveryError::NotFound(domain.to_string()))
    }
}

impl Default for DiscoveryPipeline {
    fn default() -> Self {
        Self::new()
    }
}
