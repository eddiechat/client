//! Known email provider configurations
//!
//! This module contains pre-configured settings for major email providers.
//! These configurations are used as fast paths when the domain matches
//! a known provider, avoiding the need for autodiscovery.

use super::{
    AuthMethod, EmailDiscoveryConfig, Security, ServerConfig, UsernameHint,
};

/// Check if the email domain matches a known provider
pub fn check_known_provider(email: &str, domain: &str) -> Option<EmailDiscoveryConfig> {
    let domain_lower = domain.to_lowercase();

    // Gmail / Google
    if matches!(
        domain_lower.as_str(),
        "gmail.com" | "googlemail.com" | "google.com"
    ) {
        return Some(google_config(&domain_lower));
    }

    // Microsoft consumer domains
    if matches!(
        domain_lower.as_str(),
        "outlook.com"
            | "hotmail.com"
            | "live.com"
            | "msn.com"
            | "hotmail.co.uk"
            | "hotmail.fr"
            | "hotmail.de"
            | "outlook.co.uk"
            | "outlook.fr"
            | "outlook.de"
    ) {
        return Some(microsoft_consumer_config(&domain_lower));
    }

    // Yahoo domains
    if matches!(
        domain_lower.as_str(),
        "yahoo.com"
            | "yahoo.co.uk"
            | "yahoo.fr"
            | "yahoo.de"
            | "yahoo.ca"
            | "yahoo.com.au"
            | "ymail.com"
            | "rocketmail.com"
    ) {
        return Some(yahoo_config(&domain_lower));
    }

    // AOL (uses Yahoo infrastructure)
    if domain_lower == "aol.com" {
        return Some(aol_config(&domain_lower));
    }

    // iCloud
    if matches!(
        domain_lower.as_str(),
        "icloud.com" | "me.com" | "mac.com"
    ) {
        return Some(icloud_config(&domain_lower));
    }

    // Fastmail
    if matches!(
        domain_lower.as_str(),
        "fastmail.com" | "fastmail.fm" | "messagingengine.com"
    ) {
        return Some(fastmail_config(&domain_lower));
    }

    // ProtonMail
    if matches!(
        domain_lower.as_str(),
        "protonmail.com" | "protonmail.ch" | "pm.me" | "proton.me"
    ) {
        return Some(protonmail_config(&domain_lower));
    }

    // GMX
    if matches!(
        domain_lower.as_str(),
        "gmx.com" | "gmx.de" | "gmx.net" | "gmx.at" | "gmx.ch"
    ) {
        return Some(gmx_config(&domain_lower));
    }

    // mail.com (owned by GMX/1&1)
    if domain_lower == "mail.com" {
        return Some(mail_com_config(&domain_lower));
    }

    // Zoho
    if domain_lower == "zoho.com" {
        return Some(zoho_config(&domain_lower));
    }

    None
}

// ============================================================================
// Provider-specific configurations
// ============================================================================

/// Google / Gmail configuration
/// Note: Gmail requires app-specific password for third-party IMAP access
pub fn google_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("Gmail".to_string()),
        provider_id: Some("gmail.com".to_string()),
        imap: ServerConfig {
            hostname: "imap.gmail.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "smtp.gmail.com".to_string(),
            port: 587,
            security: Security::Starttls,
        },
        auth_method: AuthMethod::AppPassword,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: true,
        source: "known_provider".to_string(),
    }
}

/// Microsoft 365 / Office 365 configuration (for custom domains)
pub fn microsoft_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
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
        source: "known_provider".to_string(),
    }
}

/// Microsoft consumer (Outlook.com, Hotmail, Live) configuration
fn microsoft_consumer_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("Outlook.com".to_string()),
        provider_id: Some("outlook.com".to_string()),
        imap: ServerConfig {
            hostname: "outlook.office365.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "smtp-mail.outlook.com".to_string(),
            port: 587,
            security: Security::Starttls,
        },
        auth_method: AuthMethod::Password,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: false,
        source: "known_provider".to_string(),
    }
}

/// Yahoo Mail configuration
/// Note: Yahoo requires app-specific password for third-party IMAP access
pub fn yahoo_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("Yahoo Mail".to_string()),
        provider_id: Some("yahoo.com".to_string()),
        imap: ServerConfig {
            hostname: "imap.mail.yahoo.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "smtp.mail.yahoo.com".to_string(),
            port: 465,
            security: Security::Tls,
        },
        auth_method: AuthMethod::AppPassword,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: true,
        source: "known_provider".to_string(),
    }
}

/// AOL configuration
fn aol_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("AOL Mail".to_string()),
        provider_id: Some("aol.com".to_string()),
        imap: ServerConfig {
            hostname: "imap.aol.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "smtp.aol.com".to_string(),
            port: 465,
            security: Security::Tls,
        },
        auth_method: AuthMethod::AppPassword,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: true,
        source: "known_provider".to_string(),
    }
}

/// iCloud configuration
pub fn icloud_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("iCloud".to_string()),
        provider_id: Some("icloud.com".to_string()),
        imap: ServerConfig {
            hostname: "imap.mail.me.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "smtp.mail.me.com".to_string(),
            port: 587,
            security: Security::Starttls,
        },
        auth_method: AuthMethod::AppPassword,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: true,
        source: "known_provider".to_string(),
    }
}

/// Fastmail configuration
pub fn fastmail_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("Fastmail".to_string()),
        provider_id: Some("fastmail.com".to_string()),
        imap: ServerConfig {
            hostname: "imap.fastmail.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "smtp.fastmail.com".to_string(),
            port: 465,
            security: Security::Tls,
        },
        auth_method: AuthMethod::AppPassword,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: true,
        source: "known_provider".to_string(),
    }
}

/// ProtonMail configuration
///
/// Note: ProtonMail requires Bridge for IMAP access (paid feature)
pub fn protonmail_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("ProtonMail".to_string()),
        provider_id: Some("protonmail.com".to_string()),
        // ProtonMail Bridge runs locally
        imap: ServerConfig {
            hostname: "127.0.0.1".to_string(),
            port: 1143,
            security: Security::Starttls,
        },
        smtp: ServerConfig {
            hostname: "127.0.0.1".to_string(),
            port: 1025,
            security: Security::Starttls,
        },
        // Bridge provides its own password
        auth_method: AuthMethod::Password,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: false,
        source: "known_provider".to_string(),
    }
}

/// GMX configuration
fn gmx_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("GMX".to_string()),
        provider_id: Some("gmx.com".to_string()),
        imap: ServerConfig {
            hostname: "imap.gmx.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "mail.gmx.com".to_string(),
            port: 587,
            security: Security::Starttls,
        },
        auth_method: AuthMethod::Password,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: false,
        source: "known_provider".to_string(),
    }
}

/// mail.com configuration
fn mail_com_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
        provider: Some("mail.com".to_string()),
        provider_id: Some("mail.com".to_string()),
        imap: ServerConfig {
            hostname: "imap.mail.com".to_string(),
            port: 993,
            security: Security::Tls,
        },
        smtp: ServerConfig {
            hostname: "smtp.mail.com".to_string(),
            port: 587,
            security: Security::Starttls,
        },
        auth_method: AuthMethod::Password,
        username_hint: UsernameHint::FullEmail,
        requires_app_password: false,
        source: "known_provider".to_string(),
    }
}

/// Zoho configuration
fn zoho_config(domain: &str) -> EmailDiscoveryConfig {
    EmailDiscoveryConfig {
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
        username_hint: UsernameHint::FullEmail,
        requires_app_password: false,
        source: "known_provider".to_string(),
    }
}
