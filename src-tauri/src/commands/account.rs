use serde::Serialize;

use crate::adapters::{imap, sqlite};
use crate::error::EddieError;
use tokio::sync::mpsc;
use crate::services::logger;

#[tauri::command]
pub async fn connect_account(
    pool: tauri::State<'_, sqlite::DbPool>,
    wake_tx: tauri::State<'_, mpsc::Sender<()>>,
    email: String,
    password: String,
    imap_host: String,
    imap_port: u16,
    imap_tls: Option<bool>,
    smtp_host: String,
    smtp_port: u16,
    smtp_tls: Option<bool>,
    aliases: Option<String>,
) -> Result<String, EddieError> {
    let use_tls = imap_tls.unwrap_or(true);
    let use_smtp_tls = smtp_tls.unwrap_or(true);
    logger::info(&format!("Connecting account: email={}, imap_host={}, imap_tls={}", email, imap_host, use_tls));

    // Verify IMAP credentials before saving the account
    let mut conn = imap::connection::connect_with_tls(&imap_host, imap_port, use_tls, &email, &password, true).await?;
    conn.session.logout().await.ok();
    logger::info("IMAP credentials verified");

    let id = sqlite::accounts::insert_account(
        &pool, &email, &password, &imap_host, imap_port, use_tls, &smtp_host, smtp_port, use_smtp_tls,
    )?;
    sqlite::entities::insert_entity(&pool, &id, &email, "account", "user")?;

    if let Some(alias_str) = &aliases {
        logger::debug(&format!("Registering aliases: {}", alias_str));
        for alias in alias_str.split(&[',', ' '][..]) {
            let trimmed = alias.trim();
            if !trimmed.is_empty() {
                sqlite::entities::insert_entity(&pool, &id, trimmed, "account", "alias")?;
            }
        }
    }

    logger::set_source(&email);
    logger::set_host(&imap_host);
    let _ = wake_tx.send(()).await;
    logger::info(&format!("Account connected, engine woken: account_id={}", id));
    Ok(id)
}

#[derive(Debug, Serialize)]
pub struct ExistingAccount {
    pub id: String,
    pub email: String,
}

/// Returns the first existing account, or null if none exist.
/// Used on app startup to auto-login returning users.
#[tauri::command]
pub async fn get_existing_account(
    pool: tauri::State<'_, sqlite::DbPool>,
) -> Result<Option<ExistingAccount>, EddieError> {
    let result = sqlite::accounts::get_first_account(&pool)?;
    Ok(result.map(|(id, email)| ExistingAccount { id, email }))
}

/// Full account details for the edit screen.
#[derive(Debug, Serialize)]
pub struct AccountDetails {
    pub id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    pub aliases: Vec<String>,
}

#[tauri::command]
pub async fn get_account(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
) -> Result<AccountDetails, EddieError> {
    sqlite::accounts::get_account_details(&pool, &account_id)
}

#[tauri::command]
pub async fn update_account(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    display_name: Option<String>,
    password: Option<String>,
    imap_host: Option<String>,
    imap_port: Option<u16>,
    imap_tls: Option<bool>,
    smtp_host: Option<String>,
    smtp_port: Option<u16>,
    smtp_tls: Option<bool>,
    aliases: Option<String>,
) -> Result<(), EddieError> {
    sqlite::accounts::update_account(
        &pool, &account_id,
        display_name.as_deref(), password.as_deref(),
        imap_host.as_deref(), imap_port, imap_tls,
        smtp_host.as_deref(), smtp_port, smtp_tls,
    )?;

    // Update aliases if provided
    if let Some(alias_str) = aliases {
        // Remove old aliases
        let existing = sqlite::entities::get_self_emails(&pool, &account_id)?;
        let account_email = sqlite::accounts::get_credentials(&pool, &account_id)?
            .map(|c| c.email)
            .unwrap_or_default();

        for e in &existing {
            if e != &account_email {
                sqlite::entities::delete_entity(&pool, &account_id, e)?;
            }
        }
        // Add new aliases
        for alias in alias_str.split(&[',', ' '][..]) {
            let trimmed = alias.trim();
            if !trimmed.is_empty() && trimmed != account_email {
                sqlite::entities::insert_entity(&pool, &account_id, trimmed, "account", "alias")?;
            }
        }
    }

    logger::info(&format!("Account updated: {}", account_id));
    Ok(())
}
