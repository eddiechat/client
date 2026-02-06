//! Mozilla Autoconfig and Microsoft Autodiscover implementations
//!
//! Mozilla Autoconfig (ISPDB): https://wiki.mozilla.org/Thunderbird:Autoconfiguration
//! Microsoft Autodiscover v2: JSON-based protocol for Office 365

use quick_xml::de::from_str;
use serde::Deserialize;
use tracing::{debug, info, warn};

use super::{
    AuthMethod, AutodiscoveryError, EmailDiscoveryConfig, Security, ServerConfig,
    UsernameHint,
};

// ============================================================================
// Mozilla Autoconfig XML structures
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename = "clientConfig")]
struct ClientConfig {
    #[serde(rename = "emailProvider")]
    email_provider: Option<EmailProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmailProvider {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "displayShortName")]
    display_short_name: Option<String>,
    #[serde(rename = "incomingServer", default)]
    incoming_servers: Vec<IncomingServer>,
    #[serde(rename = "outgoingServer", default)]
    outgoing_servers: Vec<OutgoingServer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct IncomingServer {
    #[serde(rename = "@type")]
    server_type: Option<String>,
    hostname: Option<String>,
    port: Option<u16>,
    #[serde(rename = "socketType")]
    socket_type: Option<String>,
    username: Option<String>,
    authentication: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct OutgoingServer {
    #[serde(rename = "@type")]
    server_type: Option<String>,
    hostname: Option<String>,
    port: Option<u16>,
    #[serde(rename = "socketType")]
    socket_type: Option<String>,
    username: Option<String>,
    authentication: Option<String>,
}

// ============================================================================
// Microsoft Autodiscover v2 JSON structures
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct AutodiscoverResponse {
    protocol: Option<String>,
    url: Option<String>,
}

// ============================================================================
// Mozilla Autoconfig implementation
// ============================================================================

/// Try Mozilla Autoconfig discovery
///
/// Attempts these URLs in order:
/// 1. ISPDB: https://autoconfig.thunderbird.net/v1.1/{domain}
/// 2. Domain autoconfig: https://autoconfig.{domain}/mail/config-v1.1.xml
/// 3. Well-known: https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml
pub async fn try_mozilla_autoconfig(
    client: &reqwest::Client,
    email: &str,
    domain: &str,
) -> Result<EmailDiscoveryConfig, AutodiscoveryError> {
    let urls = [
        // ISPDB - Thunderbird's database of known providers
        format!("https://autoconfig.thunderbird.net/v1.1/{}", domain),
        // Domain-hosted autoconfig
        format!(
            "https://autoconfig.{}/mail/config-v1.1.xml?emailaddress={}",
            domain, email
        ),
        // Well-known path
        format!(
            "https://{}/.well-known/autoconfig/mail/config-v1.1.xml?emailaddress={}",
            domain, email
        ),
        // HTTP fallbacks (some servers don't have HTTPS)
        format!(
            "http://autoconfig.{}/mail/config-v1.1.xml?emailaddress={}",
            domain, email
        ),
        format!(
            "http://{}/.well-known/autoconfig/mail/config-v1.1.xml?emailaddress={}",
            domain, email
        ),
    ];

    for url in urls {
        debug!("Trying autoconfig URL: {}", url);

        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                let text = response.text().await?;
                debug!("Got autoconfig response, parsing XML");

                match parse_autoconfig_xml(&text, email, domain) {
                    Ok(config) => {
                        info!("Successfully parsed autoconfig from {}", url);
                        return Ok(config);
                    }
                    Err(e) => {
                        warn!("Failed to parse autoconfig XML from {}: {}", url, e);
                        continue;
                    }
                }
            }
            Ok(response) => {
                debug!("Autoconfig URL {} returned status {}", url, response.status());
            }
            Err(e) => {
                debug!("Failed to fetch autoconfig from {}: {}", url, e);
            }
        }
    }

    Err(AutodiscoveryError::NotFound(format!(
        "No autoconfig found for {}",
        domain
    )))
}

/// Parse Mozilla Autoconfig XML
fn parse_autoconfig_xml(
    xml: &str,
    email: &str,
    domain: &str,
) -> Result<EmailDiscoveryConfig, AutodiscoveryError> {
    let config: ClientConfig = from_str(xml)?;

    let provider = config
        .email_provider
        .ok_or_else(|| AutodiscoveryError::NotFound("No emailProvider in XML".to_string()))?;

    // Find IMAP server (prefer over POP3)
    let imap_server = provider
        .incoming_servers
        .iter()
        .find(|s| s.server_type.as_deref() == Some("imap"))
        .or_else(|| provider.incoming_servers.first())
        .ok_or_else(|| AutodiscoveryError::NotFound("No incoming server in autoconfig".to_string()))?;

    // Find SMTP server
    let smtp_server = provider
        .outgoing_servers
        .iter()
        .find(|s| s.server_type.as_deref() == Some("smtp"))
        .or_else(|| provider.outgoing_servers.first())
        .ok_or_else(|| AutodiscoveryError::NotFound("No outgoing server in autoconfig".to_string()))?;

    // Parse security type
    let imap_security = parse_socket_type(imap_server.socket_type.as_deref());
    let smtp_security = parse_socket_type(smtp_server.socket_type.as_deref());

    // Parse username hint
    let username_hint = parse_username_hint(imap_server.username.as_deref(), email);

    // Apply placeholders to hostnames
    let imap_hostname = apply_placeholders(
        imap_server.hostname.as_deref().unwrap_or(""),
        email,
        domain,
    );
    let smtp_hostname = apply_placeholders(
        smtp_server.hostname.as_deref().unwrap_or(""),
        email,
        domain,
    );

    // Apply placeholders to provider name
    let provider_name = provider
        .display_name
        .or(provider.display_short_name)
        .map(|name| apply_placeholders(&name, email, domain));

    Ok(EmailDiscoveryConfig {
        provider: provider_name,
        provider_id: provider.id,
        imap: ServerConfig {
            hostname: imap_hostname,
            port: imap_server.port.unwrap_or(993),
            security: imap_security,
        },
        smtp: ServerConfig {
            hostname: smtp_hostname,
            port: smtp_server.port.unwrap_or(587),
            security: smtp_security,
        },
        auth_method: AuthMethod::Password,
        username_hint,
        requires_app_password: false,
        source: "autoconfig".to_string(),
    })
}

// ============================================================================
// Microsoft Autodiscover v2 implementation
// ============================================================================

/// Try Microsoft Autodiscover v2 (JSON protocol for Office 365)
pub async fn try_microsoft_autodiscover(
    client: &reqwest::Client,
    email: &str,
    domain: &str,
) -> Result<EmailDiscoveryConfig, AutodiscoveryError> {
    // First, check if this might be a Microsoft/Office 365 domain
    // by trying the standard Autodiscover v2 endpoint
    let autodiscover_url = format!(
        "https://outlook.office365.com/autodiscover/autodiscover.json?Email={}&Protocol=Ews",
        email
    );

    debug!("Trying Microsoft Autodiscover: {}", autodiscover_url);

    let response = client.get(&autodiscover_url).send().await?;

    if response.status().is_success() {
        let json: AutodiscoverResponse = response.json().await?;

        // If we get a valid response, this is Office 365
        if json.url.is_some() {
            info!("Microsoft Autodiscover indicates Office 365");
            return Ok(EmailDiscoveryConfig {
                provider: Some("Microsoft 365".to_string()),
                provider_id: Some("outlook.com".to_string()),
                imap: ServerConfig {
                    hostname: "outlook.office365.com".to_string(),
                    port: 993,
                    security: Security::Tls,
                },
                smtp: ServerConfig {
                    hostname: "smtp.office365.com".to_string(),
                    port: 587,
                    security: Security::Starttls,
                },
                auth_method: AuthMethod::Password,
                username_hint: UsernameHint::FullEmail,
                requires_app_password: false,
                source: "autodiscover".to_string(),
            });
        }
    }

    // Try domain-specific Autodiscover endpoints for on-premises Exchange
    let domain_urls = [
        format!("https://{}/autodiscover/autodiscover.json?Email={}&Protocol=Ews", domain, email),
        format!("https://autodiscover.{}/autodiscover/autodiscover.json?Email={}&Protocol=Ews", domain, email),
    ];

    for url in domain_urls {
        debug!("Trying domain Autodiscover: {}", url);

        if let Ok(response) = client.get(&url).send().await {
            if response.status().is_success() {
                if let Ok(json) = response.json::<AutodiscoverResponse>().await {
                    if json.url.is_some() {
                        // On-premises Exchange found
                        info!("Found on-premises Exchange via Autodiscover");
                        return Ok(EmailDiscoveryConfig {
                            provider: Some("Exchange Server".to_string()),
                            provider_id: Some(domain.to_string()),
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
                            username_hint: UsernameHint::FullEmail,
                            requires_app_password: false,
                            source: "autodiscover".to_string(),
                        });
                    }
                }
            }
        }
    }

    Err(AutodiscoveryError::NotFound(format!(
        "No Autodiscover found for {}",
        domain
    )))
}

// ============================================================================
// Helper functions
// ============================================================================

/// Parse socket type from autoconfig XML
fn parse_socket_type(socket_type: Option<&str>) -> Security {
    match socket_type {
        Some("SSL") | Some("TLS") => Security::Tls,
        Some("STARTTLS") => Security::Starttls,
        Some("plain") | Some("PLAIN") => Security::None,
        _ => Security::Tls, // Default to TLS for security
    }
}

/// Parse username hint from autoconfig placeholder
fn parse_username_hint(username: Option<&str>, email: &str) -> UsernameHint {
    match username {
        Some("%EMAILADDRESS%") | Some("%EMAIL%") => UsernameHint::FullEmail,
        Some("%EMAILLOCALPART%") => UsernameHint::LocalPart,
        Some(s) if s.contains('%') => {
            // Apply placeholders to get actual format
            let resolved = apply_placeholders(s, email, "");
            if resolved == email {
                UsernameHint::FullEmail
            } else if resolved == email.split('@').next().unwrap_or("") {
                UsernameHint::LocalPart
            } else {
                UsernameHint::Custom(resolved)
            }
        }
        _ => UsernameHint::FullEmail,
    }
}

/// Apply Mozilla Autoconfig placeholders
fn apply_placeholders(template: &str, email: &str, domain: &str) -> String {
    let local_part = email.split('@').next().unwrap_or("");
    let email_domain = email.split('@').nth(1).unwrap_or(domain);

    template
        .replace("%EMAILADDRESS%", email)
        .replace("%EMAIL%", email)
        .replace("%EMAILLOCALPART%", local_part)
        .replace("%EMAILDOMAIN%", email_domain)
}
