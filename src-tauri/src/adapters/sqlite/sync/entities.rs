use rusqlite::params;
use uuid::Uuid;
use crate::services::logger;

use super::DbPool;
use crate::error::EddieError;

pub struct NewEntity {
    pub account_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub trust_level: String,
    pub source: Option<String>,
    pub first_seen: i64,
    pub last_seen: Option<i64>,
    pub sent_count: Option<i32>,
    pub metadata: Option<String>,
}

pub fn upsert_entities(pool: &DbPool, entities: &[NewEntity]) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;

    let mut count = 0;

    for entity in entities {
        let result = tx.execute(
            "INSERT INTO entities (
                id, account_id, email, display_name, trust_level,
                source, first_seen, last_seen, sent_count, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(account_id, email) DO UPDATE SET
                trust_level = CASE
                    WHEN excluded.trust_level = 'connection'
                        AND entities.trust_level = 'contact'
                    THEN 'connection'
                    ELSE entities.trust_level
                END,
                last_seen = MAX(COALESCE(entities.last_seen, 0), COALESCE(excluded.last_seen, 0)),
                display_name = COALESCE(entities.display_name, excluded.display_name),
                sent_count = COALESCE(excluded.sent_count, entities.sent_count)",
            params![
                Uuid::new_v4().to_string(),
                entity.account_id,
                entity.email,
                entity.display_name,
                entity.trust_level,
                entity.source,
                entity.first_seen,
                entity.last_seen,
                entity.sent_count.unwrap_or(0),
                entity.metadata.as_deref().unwrap_or("{}"),
            ],
        );

        match result {
            Ok(_) => count += 1,
            Err(e) => logger::warn(&format!("Failed to upsert entity {}: {}", entity.email, e)),
        }
    }

    tx.commit()?;
    Ok(count)
}

pub fn insert_entity(pool: &DbPool, account_id: &str, email: &str, source: &str, trust_level: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| EddieError::Config("System clock error".into()))?
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO entities (id, account_id, email, trust_level, source, first_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(account_id, email) DO UPDATE SET trust_level = excluded.trust_level",
        params![Uuid::new_v4().to_string(), account_id, email, trust_level, source, now],
    )?;

    Ok(())
}

pub fn delete_entity(pool: &DbPool, account_id: &str, email: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM entities WHERE account_id = ?1 AND email = ?2",
        params![account_id, email],
    )?;
    Ok(())
}

pub fn get_self_emails(pool: &DbPool, account_id: &str) -> Result<Vec<String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT email FROM entities
             WHERE account_id = ?1 AND trust_level IN ('user', 'alias')"
        )?;

    let rows = stmt
        .query_map(rusqlite::params![account_id], |row| row.get(0))?;

    let mut emails = Vec::new();
    for row in rows {
        emails.push(row?);
    }
    Ok(emails)
}
