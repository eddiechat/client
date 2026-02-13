//! Account adapter bridging the sync DB with eddie-client's Config DB.
//!
//! The Config DB (global singleton) stores account credentials, IMAP/SMTP configs,
//! and encrypted passwords. The sync DB `accounts` table is a minimal reference
//! used only for foreign key integrity (other tables reference `accounts(id)`).
//!
//! Credentials are retrieved from the Config DB and decrypted on-the-fly.

use tracing::{debug, info};

use super::DbPool;
use crate::config::ImapConfig;
use crate::encryption::DeviceEncryption;
use crate::sync::db::{get_all_connection_configs, get_connection_config};
use crate::types::error::EddieError;

/// IMAP credentials needed to connect to the server.
pub struct Credentials {
    pub host: String,
    pub port: u16,
    pub email: String,
    pub password: String,
}

/// Ensure an account row exists in the sync DB (for FK integrity).
/// The `account_id` is the email address, matching the Config DB.
pub fn ensure_account(pool: &DbPool, account_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT OR IGNORE INTO accounts (id, email, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![account_id, account_id, chrono::Utc::now().timestamp_millis()],
    )?;
    debug!(account_id = %account_id, "Ensured account row in sync DB");
    Ok(())
}

/// Find an account that needs onboarding (has incomplete or missing tasks).
/// Checks all accounts in the Config DB against the sync DB's onboarding_tasks.
pub fn find_account_for_onboarding(pool: &DbPool) -> Result<Option<String>, EddieError> {
    // Get all configured accounts from Config DB
    let configs = get_all_connection_configs()?;
    if configs.is_empty() {
        return Ok(None);
    }

    let conn = pool.get()?;

    for config in &configs {
        let account_id = &config.account_id;

        // Ensure account exists in sync DB
        ensure_account(pool, account_id)?;

        // Check if this account has incomplete onboarding tasks
        let has_incomplete: bool = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM onboarding_tasks
                    WHERE account_id = ?1 AND status != 'done'
                )",
                rusqlite::params![account_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if has_incomplete {
            return Ok(Some(account_id.clone()));
        }

        // Check if this account has NO onboarding tasks at all (new account)
        let has_tasks: bool = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM onboarding_tasks WHERE account_id = ?1
                )",
                rusqlite::params![account_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_tasks {
            return Ok(Some(account_id.clone()));
        }
    }

    Ok(None)
}

/// Get IMAP credentials for an account by decrypting from the Config DB.
pub fn get_credentials(
    _pool: &DbPool,
    account_id: &str,
) -> Result<Option<Credentials>, EddieError> {
    let config = match get_connection_config(account_id)? {
        Some(c) => c,
        None => return Ok(None),
    };

    // Parse IMAP config JSON
    let imap_json = config
        .imap_config
        .as_deref()
        .ok_or_else(|| EddieError::Config("No IMAP config for account".to_string()))?;
    let imap_config: ImapConfig = serde_json::from_str(imap_json)
        .map_err(|e| EddieError::Config(format!("Invalid IMAP config JSON: {}", e)))?;

    // Decrypt password
    let encrypted = config
        .encrypted_password
        .as_deref()
        .ok_or_else(|| EddieError::Auth("No password stored for account".to_string()))?;
    let encryption = DeviceEncryption::new()
        .map_err(|e| EddieError::Credential(format!("Encryption init failed: {}", e)))?;
    let password = encryption
        .decrypt(encrypted)
        .map_err(|e| EddieError::Credential(format!("Password decryption failed: {}", e)))?;

    info!(account_id = %account_id, host = %imap_config.host, "Retrieved IMAP credentials");

    Ok(Some(Credentials {
        host: imap_config.host,
        port: imap_config.port,
        email: config.email,
        password,
    }))
}

/// List account IDs that have completed onboarding (all tasks are 'done').
pub fn list_onboarded_account_ids(pool: &DbPool) -> Result<Vec<String>, EddieError> {
    // Get all configured accounts from Config DB
    let configs = get_all_connection_configs()?;
    let conn = pool.get()?;

    let mut onboarded = Vec::new();
    for config in &configs {
        let account_id = &config.account_id;

        // Ensure account exists in sync DB
        ensure_account(pool, account_id)?;

        // Check that tasks exist and all are done
        let has_tasks: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM onboarding_tasks WHERE account_id = ?1)",
                rusqlite::params![account_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_tasks {
            continue; // No tasks = not yet started onboarding
        }

        let all_done: bool = conn
            .query_row(
                "SELECT NOT EXISTS(
                    SELECT 1 FROM onboarding_tasks
                    WHERE account_id = ?1 AND status != 'done'
                )",
                rusqlite::params![account_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if all_done {
            onboarded.push(account_id.clone());
        }
    }

    Ok(onboarded)
}
