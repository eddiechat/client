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
    smtp_tls: bool,
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
            id, email, password, imap_host, imap_port, imap_tls, smtp_host, smtp_port, smtp_tls, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, email, password, imap_host, imap_port, imap_tls, smtp_host, smtp_port, smtp_tls, now],
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

pub struct SmtpCredentials {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub email: String,
    pub password: String,
}

pub fn get_smtp_credentials(pool: &DbPool, account_id: &str) -> Result<Option<SmtpCredentials>, EddieError> {
    let conn = pool.get()?;

    let result = conn.query_row(
        "SELECT smtp_host, smtp_port, smtp_tls, email, password FROM accounts
         WHERE id = ?1",
        rusqlite::params![account_id],
        |row| {
            Ok(SmtpCredentials {
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

pub fn get_account_display_name(pool: &DbPool, account_id: &str) -> Result<Option<String>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT display_name FROM accounts WHERE id = ?1",
        rusqlite::params![account_id],
        |row| row.get(0),
    );
    match result {
        Ok(name) => Ok(name),
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

pub fn get_account_details(
    pool: &DbPool,
    account_id: &str,
) -> Result<crate::commands::account::AccountDetails, EddieError> {
    let conn = pool.get()?;
    let details = conn.query_row(
        "SELECT id, email, display_name, imap_host, imap_port, imap_tls, smtp_host, smtp_port, smtp_tls
         FROM accounts WHERE id = ?1",
        rusqlite::params![account_id],
        |row| {
            Ok(crate::commands::account::AccountDetails {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
                imap_host: row.get(3)?,
                imap_port: row.get(4)?,
                imap_tls: row.get(5)?,
                smtp_host: row.get(6)?,
                smtp_port: row.get(7)?,
                smtp_tls: row.get(8)?,
                aliases: vec![], // populated below
            })
        },
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => EddieError::AccountNotFound(account_id.to_string()),
        _ => EddieError::Database(e.to_string()),
    })?;

    // Fetch aliases
    let mut stmt = conn.prepare(
        "SELECT email FROM entities WHERE account_id = ?1 AND trust_level = 'alias'"
    )?;
    let aliases: Vec<String> = stmt.query_map(params![account_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(crate::commands::account::AccountDetails { aliases, ..details })
}

pub fn update_account(
    pool: &DbPool,
    account_id: &str,
    display_name: Option<&str>,
    password: Option<&str>,
    imap_host: Option<&str>,
    imap_port: Option<u16>,
    imap_tls: Option<bool>,
    smtp_host: Option<&str>,
    smtp_port: Option<u16>,
    smtp_tls: Option<bool>,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(v) = display_name { sets.push("display_name = ?"); values.push(Box::new(v.to_string())); }
    if let Some(v) = password { sets.push("password = ?"); values.push(Box::new(v.to_string())); }
    if let Some(v) = imap_host { sets.push("imap_host = ?"); values.push(Box::new(v.to_string())); }
    if let Some(v) = imap_port { sets.push("imap_port = ?"); values.push(Box::new(v as i64)); }
    if let Some(v) = imap_tls { sets.push("imap_tls = ?"); values.push(Box::new(v)); }
    if let Some(v) = smtp_host { sets.push("smtp_host = ?"); values.push(Box::new(v.to_string())); }
    if let Some(v) = smtp_port { sets.push("smtp_port = ?"); values.push(Box::new(v as i64)); }
    if let Some(v) = smtp_tls { sets.push("smtp_tls = ?"); values.push(Box::new(v)); }

    if sets.is_empty() {
        return Ok(());
    }

    values.push(Box::new(account_id.to_string()));
    let sql = format!(
        "UPDATE accounts SET {} WHERE id = ?",
        sets.join(", ")
    );

    let params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    conn.execute(&sql, params.as_slice())?;
    Ok(())
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
