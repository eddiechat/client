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
}

pub fn enqueue(
    pool: &DbPool,
    account_id: &str,
    action_type: &str,
    payload: &str,
) -> Result<String, EddieError> {
    let conn = pool.get()?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO action_queue (id, account_id, action_type, payload, status, retry_count, max_retries, created_at)
         VALUES (?1, ?2, ?3, ?4, 'pending', 0, 5, ?5)",
        params![id, account_id, action_type, payload, now],
    )?;

    Ok(id)
}

pub fn get_pending(pool: &DbPool, account_id: &str) -> Result<Vec<QueuedAction>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, action_type, payload, status, retry_count, max_retries, created_at, error
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

pub fn delete_completed(pool: &DbPool, account_id: &str) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let count = conn.execute(
        "DELETE FROM action_queue WHERE account_id = ?1 AND status = 'completed'",
        params![account_id],
    )?;
    Ok(count)
}
