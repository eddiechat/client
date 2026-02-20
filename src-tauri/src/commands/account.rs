use serde::Serialize;

use crate::adapters::{imap, sqlite};
use crate::error::EddieError;
use tokio::sync::mpsc;
use tracing::{info, debug};

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
    aliases: Option<String>,
) -> Result<String, EddieError> {
    let use_tls = imap_tls.unwrap_or(true);
    info!(email = %email, imap_host = %imap_host, imap_tls = use_tls, "Connecting account");

    // Verify IMAP credentials before saving the account
    let mut conn = imap::connection::connect_with_tls(&imap_host, imap_port, use_tls, &email, &password).await?;
    conn.session.logout().await.ok();
    info!("IMAP credentials verified");

    let id = sqlite::accounts::insert_account(
        &pool, &email, &password, &imap_host, imap_port, use_tls, &smtp_host, smtp_port,
    )?;
    sqlite::entities::insert_entity(&pool, &id, &email, "account", "user")?;

    if let Some(alias_str) = &aliases {
        debug!(aliases = %alias_str, "Registering aliases");
        for alias in alias_str.split(&[',', ' '][..]) {
            let trimmed = alias.trim();
            if !trimmed.is_empty() {
                sqlite::entities::insert_entity(&pool, &id, trimmed, "account", "alias")?;
            }
        }
    }

    let _ = wake_tx.send(()).await;
    info!(account_id = %id, "Account connected, engine woken");
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
