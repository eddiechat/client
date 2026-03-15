use rusqlite::params;
use uuid::Uuid;

use super::DbPool;
use crate::error::EddieError;

pub struct QueuedAction {
    pub id: String,
    pub account_id: String,
    pub action_type: String,
    pub payload: String,
    pub status: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub created_at: i64,
    pub error: Option<String>,
    pub message_id: Option<String>,
}

pub fn enqueue(
    pool: &DbPool,
    account_id: &str,
    action_type: &str,
    payload: &str,
    message_id: Option<&str>,
) -> Result<String, EddieError> {
    let conn = pool.get()?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO action_queue (id, account_id, action_type, payload, status, retry_count, max_retries, created_at, message_id)
         VALUES (?1, ?2, ?3, ?4, 'pending', 0, 5, ?5, ?6)",
        params![id, account_id, action_type, payload, now, message_id],
    )?;

    Ok(id)
}

pub fn get_pending(pool: &DbPool, account_id: &str) -> Result<Vec<QueuedAction>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, action_type, payload, status, retry_count, max_retries, created_at, error, message_id
         FROM action_queue
         WHERE account_id = ?1 AND status IN ('pending', 'failed')
           AND retry_count < max_retries
         ORDER BY created_at ASC",
    )?;

    let rows = stmt.query_map(params![account_id], |row| {
        Ok(QueuedAction {
            id: row.get(0)?,
            account_id: row.get(1)?,
            action_type: row.get(2)?,
            payload: row.get(3)?,
            status: row.get(4)?,
            retry_count: row.get(5)?,
            max_retries: row.get(6)?,
            created_at: row.get(7)?,
            error: row.get(8)?,
            message_id: row.get(9)?,
        })
    })?;

    let mut actions = Vec::new();
    for row in rows {
        actions.push(row.map_err(|e| EddieError::Database(e.to_string()))?);
    }
    Ok(actions)
}

pub fn mark_in_progress(pool: &DbPool, action_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE action_queue SET status = 'in_progress' WHERE id = ?1",
        params![action_id],
    )?;
    Ok(())
}

pub fn mark_completed(pool: &DbPool, action_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE action_queue SET status = 'completed', completed_at = ?1 WHERE id = ?2",
        params![now, action_id],
    )?;
    Ok(())
}

pub fn mark_failed(pool: &DbPool, action_id: &str, error: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE action_queue SET status = 'failed', retry_count = retry_count + 1, error = ?1 WHERE id = ?2",
        params![error, action_id],
    )?;
    Ok(())
}

pub fn mark_done(pool: &DbPool, action_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE action_queue SET status = 'done', completed_at = ?1 WHERE id = ?2",
        params![now, action_id],
    )?;
    Ok(())
}

/// Find completed send actions matching a message_id (for server confirmation).
pub fn get_completed_by_message_id(
    pool: &DbPool,
    account_id: &str,
    message_id: &str,
) -> Result<Vec<String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id FROM action_queue
         WHERE account_id = ?1 AND message_id = ?2 AND status = 'completed'",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![account_id, message_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

/// Find completed mark_read actions for a given folder (for server confirmation).
/// Returns (action_id, payload) pairs.
pub fn get_completed_mark_read(
    pool: &DbPool,
    account_id: &str,
) -> Result<Vec<(String, String)>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, payload FROM action_queue
         WHERE account_id = ?1 AND action_type = 'mark_read' AND status = 'completed'",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map(params![account_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Startup cleanup: delete actions older than 72 hours and
/// reset any in_progress actions back to pending (interrupted by shutdown).
pub fn purge_old(pool: &DbPool) -> Result<usize, EddieError> {
    let conn = pool.get()?;

    // Reset interrupted actions so they get retried
    conn.execute(
        "UPDATE action_queue SET status = 'pending' WHERE status = 'in_progress'",
        [],
    )?;

    let cutoff = chrono::Utc::now().timestamp_millis() - 72 * 60 * 60 * 1000;
    let count = conn.execute(
        "DELETE FROM action_queue WHERE created_at < ?1",
        params![cutoff],
    )?;
    Ok(count)
}
