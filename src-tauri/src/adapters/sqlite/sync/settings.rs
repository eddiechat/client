use rusqlite::params;

use super::DbPool;
use crate::error::EddieError;

pub fn get_setting(pool: &DbPool, key: &str) -> Result<Option<String>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    );

    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}

pub fn set_setting(pool: &DbPool, key: &str, value: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params![key, value, now],
    )?;

    Ok(())
}
