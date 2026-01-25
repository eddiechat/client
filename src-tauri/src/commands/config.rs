use std::path::PathBuf;
use serde::Deserialize;
use tracing::info;

use crate::config::{self, AccountConfig, AuthConfig, CardDAVConfig, ImapConfig, PasswordSource, SmtpConfig};

/// Initialize configuration from default paths
#[tauri::command]
pub async fn init_config() -> Result<(), String> {
    info!("Tauri command: init_config");
    config::init_config().map_err(|e| e.to_string())
}

/// Initialize configuration from specific paths
#[tauri::command]
pub async fn init_config_from_paths(paths: Vec<String>) -> Result<(), String> {
    info!("Tauri command: init_config_from_paths - {:?}", paths);

    for path_str in paths {
        let path = PathBuf::from(&path_str);
        if path.exists() {
            return config::init_config_from_path(&path).map_err(|e| e.to_string());
        }
    }

    Err("No valid config file found in provided paths".to_string())
}

/// Check if configuration is initialized
#[tauri::command]
pub async fn is_config_initialized() -> bool {
    config::is_initialized()
}

/// Get configuration file paths (for UI display)
#[tauri::command]
pub async fn get_config_paths() -> Result<Vec<String>, String> {
    Ok(config::default_config_paths()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// Request structure for saving an account
#[derive(Debug, Deserialize)]
pub struct SaveAccountRequest {
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub imap_tls_cert: Option<String>,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    pub smtp_tls_cert: Option<String>,
    pub username: String,
    pub password: String,
    // CardDAV settings (optional)
    pub carddav_url: Option<String>,
    pub carddav_tls: Option<bool>,
    pub carddav_tls_cert: Option<String>,
    pub carddav_username: Option<String>,
    pub carddav_password: Option<String>,
}

/// Save a new account configuration
#[tauri::command]
pub async fn save_account(request: SaveAccountRequest) -> Result<(), String> {
    info!("Tauri command: save_account - name: {}", request.name);

    let imap_auth = AuthConfig::Password {
        user: request.username.clone(),
        password: PasswordSource::Raw(request.password.clone()),
    };

    let smtp_auth = AuthConfig::Password {
        user: request.username.clone(),
        password: PasswordSource::Raw(request.password.clone()),
    };

    // Build CardDAV config if URL is provided
    let carddav = request.carddav_url.as_ref().map(|url| {
        let carddav_username = request.carddav_username.clone().unwrap_or_else(|| request.username.clone());
        let carddav_password = request.carddav_password.clone().unwrap_or_else(|| request.password.clone());

        CardDAVConfig {
            url: url.clone(),
            tls: request.carddav_tls.unwrap_or(true),
            tls_cert: request.carddav_tls_cert.clone(),
            auth: AuthConfig::Password {
                user: carddav_username,
                password: PasswordSource::Raw(carddav_password),
            },
            address_book: None,
        }
    });

    let account = AccountConfig {
        name: Some(request.name.clone()),
        default: true,
        email: request.email,
        display_name: request.display_name,
        imap: Some(ImapConfig {
            host: request.imap_host,
            port: request.imap_port,
            tls: request.imap_tls,
            tls_cert: request.imap_tls_cert,
            auth: imap_auth,
        }),
        smtp: Some(SmtpConfig {
            host: request.smtp_host,
            port: request.smtp_port,
            tls: request.smtp_tls,
            tls_cert: request.smtp_tls_cert,
            auth: smtp_auth,
        }),
        carddav,
    };

    config::save_account(request.name, account).map_err(|e| e.to_string())
}
