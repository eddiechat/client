use rusqlite::params;
use uuid::Uuid;
use crate::services::logger;

use super::DbPool;
use crate::error::EddieError;

pub fn insert_account(
    pool: &DbPool,
    email: &str,
    password: &str,
    imap_host: &str,
    imap_port: u16,
    imap_tls: bool,
    smtp_host: &str,
    smtp_port: u16,
) -> Result<String, EddieError> {
    let conn = pool.get()?;

    // Check for existing account first
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM accounts WHERE email = ?1",
            params![email],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        logger::debug(&format!("Account already exists: email={}, id={}", email, id));
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO accounts (
            id, email, password, imap_host, imap_port, imap_tls, smtp_host, smtp_port, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, email, password, imap_host, imap_port, imap_tls, smtp_host, smtp_port, now],
    )?;

    logger::info(&format!("New account created: email={}, id={}", email, id));
    Ok(id)
}

pub fn find_account_for_onboarding(pool: &DbPool) -> Result<Option<String>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT a.id FROM accounts a
         WHERE NOT EXISTS (
             SELECT 1 FROM onboarding_tasks ot WHERE ot.account_id = a.id
         )
         UNION
         SELECT DISTINCT ot.account_id FROM onboarding_tasks ot
         WHERE ot.status != 'done'
         LIMIT 1",
        [],
        |row| row.get(0),
    );

    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}

pub struct Credentials {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub email: String,
    pub password: String,
}

pub fn get_credentials(pool: &DbPool, account_id: &str) -> Result<Option<Credentials>, EddieError> {
    let conn = pool.get()?;

    let result = conn.query_row(
        "SELECT imap_host, imap_port, imap_tls, email, password FROM accounts
         WHERE id = ?1",
        rusqlite::params![account_id],
        |row| {
            Ok(Credentials {
                host: row.get(0)?,
                port: row.get(1)?,
                tls: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
            })
        },
    );

    match result {
        Ok(creds) => Ok(Some(creds)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}

/// Returns the first account's (id, email) if any exist.
pub fn get_first_account(pool: &DbPool) -> Result<Option<(String, String)>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT id, email FROM accounts LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    match result {
        Ok(pair) => Ok(Some(pair)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}

pub fn get_first_imap_host(pool: &DbPool) -> Result<Option<String>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT imap_host FROM accounts LIMIT 1",
        [],
        |row| row.get(0),
    );
    match result {
        Ok(host) => Ok(Some(host)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}

pub fn list_account_emails(pool: &DbPool) -> Result<Vec<String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT email FROM accounts")?;
    let emails = stmt.query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(emails)
}

pub fn list_onboarded_account_ids(pool: &DbPool) -> Result<Vec<String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT a.id FROM accounts a
         WHERE NOT EXISTS (
             SELECT 1 FROM onboarding_tasks t
             WHERE t.account_id = a.id AND t.status != 'done'
         )"
    )?;

    let ids = stmt.query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(ids)
}
