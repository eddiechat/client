//! DNS-based email autodiscovery
//!
//! Implements:
//! - RFC 6186: DNS SRV records for email services
//! - MX record analysis for provider detection

use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::TokioAsyncResolver;
use tracing::{debug, info, warn};

use super::{
    providers, AuthMethod, AutodiscoveryError, EmailDiscoveryConfig, OAuthProvider, Security,
    ServerConfig, UsernameHint,
};

// ============================================================================
// DNS SRV Record Discovery (RFC 6186 + RFC 8314)
// ============================================================================

/// Try to discover email configuration via DNS SRV records
///
/// Queries these records in priority order (per RFC 8314, implicit TLS preferred):
/// - _imaps._tcp.{domain} (port 993, IMAP over TLS)
/// - _imap._tcp.{domain} (port 143, IMAP with STARTTLS)
/// - _submissions._tcp.{domain} (port 465, SMTP submission over TLS)
/// - _submission._tcp.{domain} (port 587, SMTP submission with STARTTLS)
pub async fn try_srv_records(domain: &str) -> Result<EmailDiscoveryConfig, AutodiscoveryError> {
    let resolver = create_resolver()?;

    // Try IMAP SRV records
    let (imap_host, imap_port, imap_security) = {
        // Try IMAPS first (implicit TLS - preferred per RFC 8314)
        if let Some((host, port)) = query_srv(&resolver, &format!("_imaps._tcp.{}", domain)).await {
            (host, port, Security::Tls)
        }
        // Fall back to IMAP with STARTTLS
        else if let Some((host, port)) = query_srv(&resolver, &format!("_imap._tcp.{}", domain)).await {
            (host, port, Security::Starttls)
        } else {
            return Err(AutodiscoveryError::DnsError(
                "No IMAP SRV records found".to_string(),
            ));
        }
    };

    // Try SMTP SRV records
    let (smtp_host, smtp_port, smtp_security) = {
        // Try submissions first (implicit TLS - preferred per RFC 8314)
        if let Some((host, port)) = query_srv(&resolver, &format!("_submissions._tcp.{}", domain)).await {
            (host, port, Security::Tls)
        }
        // Fall back to submission with STARTTLS
        else if let Some((host, port)) = query_srv(&resolver, &format!("_submission._tcp.{}", domain)).await {
            (host, port, Security::Starttls)
        }
        // Last resort: standard SMTP (usually blocked by ISPs)
        else if let Some((host, port)) = query_srv(&resolver, &format!("_smtp._tcp.{}", domain)).await {
            (host, port, Security::Starttls)
        } else {
            // Use the IMAP host as SMTP fallback
            (imap_host.clone(), 587, Security::Starttls)
        }
    };

    info!(
        "Found SRV records - IMAP: {}:{}, SMTP: {}:{}",
        imap_host, imap_port, smtp_host, smtp_port
    );

    Ok(EmailDiscoveryConfig {
        provider: None,
        provider_id: None,
        imap: ServerConfig {
            hostname: imap_host,
            port: imap_port,
            security: imap_security,
        },
        smtp: ServerConfig {
            hostname: smtp_host,
            port: smtp_port,
            security: smtp_security,
        },
        auth_method: AuthMethod::Password,
        oauth_provider: None,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: false,
        source: "srv".to_string(),
    })
}

/// Query a single SRV record
async fn query_srv(resolver: &TokioAsyncResolver, name: &str) -> Option<(String, u16)> {
    debug!("Querying SRV record: {}", name);

    match resolver.srv_lookup(name).await {
        Ok(response) => {
            // Get the record with lowest priority (highest preference)
            if let Some(record) = response.iter().min_by_key(|r| r.priority()) {
                let host = record.target().to_string().trim_end_matches('.').to_string();
                let port = record.port();

                // Skip if target is "." (service not available)
                if host == "." || host.is_empty() {
                    debug!("SRV record {} indicates service not available", name);
                    return None;
                }

                debug!("Found SRV record: {} -> {}:{}", name, host, port);
                return Some((host, port));
            }
            None
        }
        Err(e) => {
            debug!("SRV lookup failed for {}: {}", name, e);
            None
        }
    }
}

// ============================================================================
// MX Record Analysis for Provider Detection
// ============================================================================

/// Analyze MX records to detect email provider
///
/// Maps known MX patterns to provider configurations
pub async fn try_mx_analysis(domain: &str) -> Result<EmailDiscoveryConfig, AutodiscoveryError> {
    let resolver = create_resolver()?;

    debug!("Querying MX records for {}", domain);

    let mx_records = resolver
        .mx_lookup(domain)
        .await
        .map_err(|e| AutodiscoveryError::DnsError(format!("MX lookup failed: {}", e)))?;

    // Get the MX host with lowest preference (highest priority)
    let mx_host = mx_records
        .iter()
        .min_by_key(|r| r.preference())
        .map(|r| r.exchange().to_string().to_lowercase())
        .map(|s| s.trim_end_matches('.').to_string())
        .ok_or_else(|| AutodiscoveryError::DnsError("No MX records found".to_string()))?;

    info!("Primary MX record for {}: {}", domain, mx_host);

    // Match MX host patterns to known providers
    if let Some(config) = detect_provider_from_mx(&mx_host, domain) {
        return Ok(config);
    }

    // If no known provider, we can't determine configuration from MX alone
    Err(AutodiscoveryError::NotFound(format!(
        "Unknown provider for MX: {}",
        mx_host
    )))
}

/// Detect provider configuration from MX hostname patterns
fn detect_provider_from_mx(mx_host: &str, domain: &str) -> Option<EmailDiscoveryConfig> {
    let mx_lower = mx_host.to_lowercase();

    // Google Workspace / Gmail
    if mx_lower.contains("google.com")
        || mx_lower.contains("googlemail.com")
        || mx_lower.ends_with("aspmx.l.google.com")
        || mx_lower.contains(".google.com")
    {
        return Some(providers::google_config(domain));
    }

    // Microsoft 365 / Office 365
    if mx_lower.contains("mail.protection.outlook.com")
        || mx_lower.contains("outlook.com")
        || mx_lower.contains("microsoft.com")
    {
        return Some(providers::microsoft_config(domain));
    }

    // Yahoo
    if mx_lower.contains("yahoodns.net") || mx_lower.contains("yahoo.com") {
        return Some(providers::yahoo_config(domain));
    }

    // iCloud
    if mx_lower.contains("mail.icloud.com") || mx_lower.contains("apple.com") {
        return Some(providers::icloud_config(domain));
    }

    // Fastmail
    if mx_lower.contains("messagingengine.com") || mx_lower.contains("fastmail.com") {
        return Some(providers::fastmail_config(domain));
    }

    // ProtonMail
    if mx_lower.contains("protonmail.ch") || mx_lower.contains("pm.me") {
        return Some(providers::protonmail_config(domain));
    }

    // Zoho
    if mx_lower.contains("zoho.com") || mx_lower.contains("zoho.eu") {
        return Some(EmailDiscoveryConfig {
            provider: Some("Zoho Mail".to_string()),
            provider_id: Some("zoho.com".to_string()),
            imap: ServerConfig {
                hostname: "imap.zoho.com".to_string(),
                port: 993,
                security: Security::Tls,
            },
            smtp: ServerConfig {
                hostname: "smtp.zoho.com".to_string(),
                port: 465,
                security: Security::Tls,
            },
            auth_method: AuthMethod::Password,
            oauth_provider: None,
            username_hint: UsernameHint::FullEmail,
            requires_app_password: false,
            source: "mx".to_string(),
        });
    }

    // cPanel / typical hosting providers
    if mx_lower.starts_with("mail.") && mx_lower.ends_with(domain) {
        return Some(EmailDiscoveryConfig {
            provider: None,
            provider_id: None,
            imap: ServerConfig {
                hostname: format!("mail.{}", domain),
                port: 993,
                security: Security::Tls,
            },
            smtp: ServerConfig {
                hostname: format!("mail.{}", domain),
                port: 587,
                security: Security::Starttls,
            },
            auth_method: AuthMethod::Password,
            oauth_provider: None,
            username_hint: UsernameHint::FullEmail,
            requires_app_password: false,
            source: "mx".to_string(),
        });
    }

    None
}

// ============================================================================
// Helper functions
// ============================================================================

/// Create a DNS resolver
fn create_resolver() -> Result<TokioAsyncResolver, AutodiscoveryError> {
    Ok(TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default()))
}
