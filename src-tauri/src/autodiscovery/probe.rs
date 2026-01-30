//! Heuristic server probing for email configuration
//!
//! When autodiscovery protocols fail, this module probes common
//! server hostname patterns and port combinations to find working
//! email configurations.

use std::net::ToSocketAddrs;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use super::{
    AuthMethod, AutodiscoveryError, EmailDiscoveryConfig, Security, ServerConfig, UsernameHint,
};

/// Probe common server patterns for a domain
pub async fn probe_common_servers(domain: &str) -> Result<EmailDiscoveryConfig, AutodiscoveryError> {
    // Common hostname patterns for self-hosted email
    let hostname_patterns = [
        format!("mail.{}", domain),
        format!("imap.{}", domain),
        domain.to_string(),
        format!("mx.{}", domain),
        format!("email.{}", domain),
    ];

    let smtp_patterns = [
        format!("mail.{}", domain),
        format!("smtp.{}", domain),
        domain.to_string(),
        format!("mx.{}", domain),
        format!("email.{}", domain),
    ];

    // Try to find a working IMAP server
    let mut imap_config: Option<ServerConfig> = None;
    for hostname in &hostname_patterns {
        // Try IMAPS (port 993) first - implicit TLS
        if let Ok(()) = probe_imap_server(hostname, 993, Security::Tls).await {
            info!("Found IMAP server: {}:993 (TLS)", hostname);
            imap_config = Some(ServerConfig {
                hostname: hostname.clone(),
                port: 993,
                security: Security::Tls,
            });
            break;
        }

        // Try IMAP with STARTTLS (port 143)
        if let Ok(()) = probe_imap_server(hostname, 143, Security::Starttls).await {
            info!("Found IMAP server: {}:143 (STARTTLS)", hostname);
            imap_config = Some(ServerConfig {
                hostname: hostname.clone(),
                port: 143,
                security: Security::Starttls,
            });
            break;
        }
    }

    let imap = imap_config.ok_or_else(|| {
        AutodiscoveryError::ConnectionFailed("Could not find a working IMAP server".to_string())
    })?;

    // Try to find a working SMTP server
    let mut smtp_config: Option<ServerConfig> = None;
    for hostname in &smtp_patterns {
        // Try port 587 with STARTTLS first (modern submission standard)
        if let Ok(()) = probe_smtp_server(hostname, 587, Security::Starttls).await {
            info!("Found SMTP server: {}:587 (STARTTLS)", hostname);
            smtp_config = Some(ServerConfig {
                hostname: hostname.clone(),
                port: 587,
                security: Security::Starttls,
            });
            break;
        }

        // Try port 465 with implicit TLS (legacy but still widely used)
        if let Ok(()) = probe_smtp_server(hostname, 465, Security::Tls).await {
            info!("Found SMTP server: {}:465 (TLS)", hostname);
            smtp_config = Some(ServerConfig {
                hostname: hostname.clone(),
                port: 465,
                security: Security::Tls,
            });
            break;
        }

        // Try port 25 as last resort (often blocked by ISPs)
        if let Ok(()) = probe_smtp_server(hostname, 25, Security::Starttls).await {
            info!("Found SMTP server: {}:25 (STARTTLS)", hostname);
            smtp_config = Some(ServerConfig {
                hostname: hostname.clone(),
                port: 25,
                security: Security::Starttls,
            });
            break;
        }
    }

    // If no SMTP server found, use the same host as IMAP
    let smtp = smtp_config.unwrap_or_else(|| {
        warn!("No SMTP server found, using IMAP host with port 587");
        ServerConfig {
            hostname: imap.hostname.clone(),
            port: 587,
            security: Security::Starttls,
        }
    });

    Ok(EmailDiscoveryConfig {
        provider: None,
        provider_id: None,
        imap,
        smtp,
        auth_method: AuthMethod::Password,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: false,
        source: "probe".to_string(),
    })
}

/// Probe an IMAP server to check if it's responding
async fn probe_imap_server(
    hostname: &str,
    port: u16,
    security: Security,
) -> Result<(), AutodiscoveryError> {
    debug!("Probing IMAP server {}:{}", hostname, port);

    let addr = format!("{}:{}", hostname, port);

    // First check if the hostname resolves
    let _socket_addr = addr
        .to_socket_addrs()
        .map_err(|_| AutodiscoveryError::DnsError(format!("Cannot resolve {}", hostname)))?
        .next()
        .ok_or_else(|| AutodiscoveryError::DnsError(format!("No addresses for {}", hostname)))?;

    // Try to connect with timeout
    let stream = timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| AutodiscoveryError::Timeout(format!("Connection timeout to {}", addr)))?
        .map_err(|e| AutodiscoveryError::ConnectionFailed(e.to_string()))?;

    // For TLS, we need to establish TLS connection and check banner
    if security == Security::Tls {
        // Use rustls for TLS
        if let Ok(()) = check_imap_tls_banner(hostname, stream).await {
            return Ok(());
        }
    } else {
        // For plain or STARTTLS, check plain banner first
        if let Ok(()) = check_imap_banner(stream).await {
            return Ok(());
        }
    }

    Err(AutodiscoveryError::ConnectionFailed(format!(
        "IMAP server {}:{} not responding correctly",
        hostname, port
    )))
}

/// Check IMAP banner on a plain connection
async fn check_imap_banner(mut stream: TcpStream) -> Result<(), AutodiscoveryError> {
    let mut buf = [0u8; 256];
    let n = timeout(Duration::from_secs(5), stream.read(&mut buf))
        .await
        .map_err(|_| AutodiscoveryError::Timeout("Reading IMAP banner".to_string()))?
        .map_err(|e| AutodiscoveryError::ConnectionFailed(e.to_string()))?;

    let response = String::from_utf8_lossy(&buf[..n]);
    debug!("IMAP banner: {}", response.trim());

    // IMAP greeting starts with "* OK" or "* PREAUTH"
    if response.starts_with("* OK") || response.starts_with("* PREAUTH") {
        return Ok(());
    }

    Err(AutodiscoveryError::ConnectionFailed(
        "Invalid IMAP banner".to_string(),
    ))
}

/// Check IMAP banner over TLS
async fn check_imap_tls_banner(hostname: &str, stream: TcpStream) -> Result<(), AutodiscoveryError> {
    use tokio_rustls::rustls::pki_types::ServerName;
    use tokio_rustls::rustls::ClientConfig;
    use tokio_rustls::TlsConnector;
    use std::sync::Arc;

    // Create TLS config that accepts any certificate (for probing only)
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(hostname.to_string())
        .map_err(|_| AutodiscoveryError::ConnectionFailed("Invalid hostname".to_string()))?;

    let mut tls_stream = timeout(
        Duration::from_secs(5),
        connector.connect(server_name, stream),
    )
    .await
    .map_err(|_| AutodiscoveryError::Timeout("TLS handshake timeout".to_string()))?
    .map_err(|e| AutodiscoveryError::ConnectionFailed(format!("TLS error: {}", e)))?;

    // Read IMAP banner
    let mut buf = [0u8; 256];
    let n = timeout(Duration::from_secs(5), tls_stream.read(&mut buf))
        .await
        .map_err(|_| AutodiscoveryError::Timeout("Reading IMAP banner".to_string()))?
        .map_err(|e| AutodiscoveryError::ConnectionFailed(e.to_string()))?;

    let response = String::from_utf8_lossy(&buf[..n]);
    debug!("IMAP TLS banner: {}", response.trim());

    if response.starts_with("* OK") || response.starts_with("* PREAUTH") {
        return Ok(());
    }

    Err(AutodiscoveryError::ConnectionFailed(
        "Invalid IMAP TLS banner".to_string(),
    ))
}

/// Probe an SMTP server to check if it's responding
async fn probe_smtp_server(
    hostname: &str,
    port: u16,
    security: Security,
) -> Result<(), AutodiscoveryError> {
    debug!("Probing SMTP server {}:{}", hostname, port);

    let addr = format!("{}:{}", hostname, port);

    // First check if the hostname resolves
    let _socket_addr = addr
        .to_socket_addrs()
        .map_err(|_| AutodiscoveryError::DnsError(format!("Cannot resolve {}", hostname)))?
        .next()
        .ok_or_else(|| AutodiscoveryError::DnsError(format!("No addresses for {}", hostname)))?;

    // Try to connect with timeout
    let stream = timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| AutodiscoveryError::Timeout(format!("Connection timeout to {}", addr)))?
        .map_err(|e| AutodiscoveryError::ConnectionFailed(e.to_string()))?;

    // For TLS, we need to establish TLS connection and check banner
    if security == Security::Tls {
        if let Ok(()) = check_smtp_tls_banner(hostname, stream).await {
            return Ok(());
        }
    } else {
        // For plain or STARTTLS, check plain banner first
        if let Ok(()) = check_smtp_banner(stream).await {
            return Ok(());
        }
    }

    Err(AutodiscoveryError::ConnectionFailed(format!(
        "SMTP server {}:{} not responding correctly",
        hostname, port
    )))
}

/// Check SMTP banner on a plain connection
async fn check_smtp_banner(mut stream: TcpStream) -> Result<(), AutodiscoveryError> {
    let mut buf = [0u8; 512];
    let n = timeout(Duration::from_secs(5), stream.read(&mut buf))
        .await
        .map_err(|_| AutodiscoveryError::Timeout("Reading SMTP banner".to_string()))?
        .map_err(|e| AutodiscoveryError::ConnectionFailed(e.to_string()))?;

    let response = String::from_utf8_lossy(&buf[..n]);
    debug!("SMTP banner: {}", response.trim());

    // SMTP greeting starts with "220"
    if response.starts_with("220") {
        return Ok(());
    }

    Err(AutodiscoveryError::ConnectionFailed(
        "Invalid SMTP banner".to_string(),
    ))
}

/// Check SMTP banner over TLS
async fn check_smtp_tls_banner(hostname: &str, stream: TcpStream) -> Result<(), AutodiscoveryError> {
    use tokio_rustls::rustls::pki_types::ServerName;
    use tokio_rustls::rustls::ClientConfig;
    use tokio_rustls::TlsConnector;
    use std::sync::Arc;

    // Create TLS config that accepts any certificate (for probing only)
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(hostname.to_string())
        .map_err(|_| AutodiscoveryError::ConnectionFailed("Invalid hostname".to_string()))?;

    let mut tls_stream = timeout(
        Duration::from_secs(5),
        connector.connect(server_name, stream),
    )
    .await
    .map_err(|_| AutodiscoveryError::Timeout("TLS handshake timeout".to_string()))?
    .map_err(|e| AutodiscoveryError::ConnectionFailed(format!("TLS error: {}", e)))?;

    // Read SMTP banner
    let mut buf = [0u8; 512];
    let n = timeout(Duration::from_secs(5), tls_stream.read(&mut buf))
        .await
        .map_err(|_| AutodiscoveryError::Timeout("Reading SMTP banner".to_string()))?
        .map_err(|e| AutodiscoveryError::ConnectionFailed(e.to_string()))?;

    let response = String::from_utf8_lossy(&buf[..n]);
    debug!("SMTP TLS banner: {}", response.trim());

    if response.starts_with("220") {
        return Ok(());
    }

    Err(AutodiscoveryError::ConnectionFailed(
        "Invalid SMTP TLS banner".to_string(),
    ))
}

// ============================================================================
// TLS certificate verifier that accepts any certificate (for probing only)
// ============================================================================

#[derive(Debug)]
struct AcceptAnyCert;

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[tokio_rustls::rustls::pki_types::CertificateDer<'_>],
        _server_name: &tokio_rustls::rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> Result<tokio_rustls::rustls::client::danger::ServerCertVerified, tokio_rustls::rustls::Error>
    {
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error>
    {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error>
    {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        vec![
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA512,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA512,
            tokio_rustls::rustls::SignatureScheme::ED25519,
        ]
    }
}
