
use super::DbPool;
use crate::types::error::EddieError;

pub const ONBOARDING_TASKS: &[&str] = &[
    "trust_network",
    "historical_fetch",
    "connection_history",
];

pub struct Task {
    pub name: String,
    pub status: String,
}

pub fn get_tasks(pool: &DbPool, account_id: &str) -> Result<Vec<Task>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT task, status FROM onboarding_tasks
             WHERE account_id = ?1 ORDER BY rowid",
        )?;

    let rows = stmt
        .query_map(rusqlite::params![account_id], |row| {
            Ok(Task {
                name: row.get(0)?,
                status: row.get(1)?,
            })
        })?;

    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row?);
    }
    Ok(tasks)
}

pub fn seed_tasks(pool: &DbPool, account_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();

    for task in ONBOARDING_TASKS {
        conn.execute(
            "INSERT OR IGNORE INTO onboarding_tasks (account_id, task, status, updated_at)
             VALUES (?1, ?2, 'pending', ?3)",
            rusqlite::params![account_id, task, now],
        )?;
    }
    Ok(())
}

pub fn mark_task_done(pool: &DbPool, account_id: &str, task: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE onboarding_tasks SET status = 'done', updated_at = ?1
         WHERE account_id = ?2 AND task = ?3",
        rusqlite::params![now, account_id, task],
    )?;
    Ok(())
}
