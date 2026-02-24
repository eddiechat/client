use rusqlite::params;

use super::DbPool;
use crate::error::EddieError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

pub struct ClassifyCursor {
    pub skill_rev: String,
    pub highest_classified_uid: u32,
    pub lowest_classified_uid: u32,
}

pub struct ClassifyCandidate {
    pub id: String,
    pub imap_uid: u32,
    pub subject: Option<String>,
    pub body_text: Option<String>,
}

#[derive(Default)]
pub struct Modifiers {
    pub exclude_newsletters: bool,
    pub only_known_senders: bool,
    pub has_attachments: bool,
    pub recent_six_months: bool,
    pub exclude_auto_replies: bool,
}

impl Modifiers {
    pub fn from_json(json_str: &str) -> Self {
        let parsed: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        Self {
            exclude_newsletters: parsed.get("excludeNewsletters")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            only_known_senders: parsed.get("onlyKnownSenders")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            has_attachments: parsed.get("hasAttachments")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            recent_six_months: parsed.get("recentSixMonths")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            exclude_auto_replies: parsed.get("excludeAutoReplies")
                .and_then(|v| v.as_bool()).unwrap_or(false),
        }
    }
}

// ---------------------------------------------------------------------------
// Cursor CRUD
// ---------------------------------------------------------------------------

pub fn ensure_cursor(
    pool: &DbPool,
    skill_id: &str,
    account_id: &str,
    folder: &str,
    skill_rev: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    // Seed highest_classified_uid to the current max UID so forward only handles
    // new arrivals and backward fills history from newest to oldest.
    let max_uid: u32 = conn.query_row(
        "SELECT COALESCE(MAX(imap_uid), 0) FROM messages
         WHERE account_id = ?1 AND imap_folder = ?2",
        params![account_id, folder],
        |row| row.get(0),
    ).unwrap_or(0);
    conn.execute(
        "INSERT OR IGNORE INTO folder_classify
         (skill_id, account_id, folder, skill_rev, highest_classified_uid, lowest_classified_uid)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![skill_id, account_id, folder, skill_rev, max_uid as i64],
    )?;
    Ok(())
}

pub fn get_cursor(
    pool: &DbPool,
    skill_id: &str,
    account_id: &str,
    folder: &str,
) -> Result<Option<ClassifyCursor>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT skill_rev, highest_classified_uid, lowest_classified_uid
         FROM folder_classify
         WHERE skill_id = ?1 AND account_id = ?2 AND folder = ?3",
        params![skill_id, account_id, folder],
        |row| Ok(ClassifyCursor {
            skill_rev: row.get(0)?,
            highest_classified_uid: row.get(1)?,
            lowest_classified_uid: row.get(2)?,
        }),
    );
    match result {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}

pub fn reset_skill_cursors(
    pool: &DbPool,
    skill_id: &str,
    _new_rev: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM skill_matches WHERE skill_id = ?1",
        params![skill_id],
    )?;
    // Delete cursor rows so ensure_cursor re-creates them with proper max UID seeding
    conn.execute(
        "DELETE FROM folder_classify WHERE skill_id = ?1",
        params![skill_id],
    )?;
    Ok(())
}

pub fn update_highest_classified_uid(
    pool: &DbPool,
    skill_id: &str,
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE folder_classify
         SET highest_classified_uid = ?1, last_classify = ?2
         WHERE skill_id = ?3 AND account_id = ?4 AND folder = ?5
         AND highest_classified_uid < ?1",
        params![uid as i64, now, skill_id, account_id, folder],
    )?;
    Ok(())
}

pub fn update_lowest_classified_uid(
    pool: &DbPool,
    skill_id: &str,
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE folder_classify
         SET lowest_classified_uid = ?1, last_classify = ?2
         WHERE skill_id = ?3 AND account_id = ?4 AND folder = ?5
         AND (lowest_classified_uid = 0 OR lowest_classified_uid > ?1)",
        params![uid as i64, now, skill_id, account_id, folder],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Batch queries
// ---------------------------------------------------------------------------

/// Build the modifier WHERE clause fragment. All conditions are literal SQL
/// (no bind params) so the fragment can be interpolated into the query string.
fn build_modifier_clause(mods: &Modifiers, account_id: &str) -> String {
    let mut clauses = Vec::new();

    if mods.exclude_newsletters {
        clauses.push("AND classification != 'newsletter'".to_string());
    }
    if mods.exclude_auto_replies {
        clauses.push("AND classification != 'automated'".to_string());
    }
    if mods.has_attachments {
        clauses.push("AND has_attachments = 1".to_string());
    }
    if mods.recent_six_months {
        let six_months_ago = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(180))
            .map(|d| d.timestamp_millis())
            .unwrap_or(0);
        clauses.push(format!("AND date >= {}", six_months_ago));
    }
    if mods.only_known_senders {
        // SQL-escape single quotes in account_id
        let safe_id = account_id.replace('\'', "''");
        clauses.push(format!(
            "AND from_address IN (SELECT email FROM entities WHERE account_id = '{}' AND trust_level IN ('connection', 'contact'))",
            safe_id
        ));
    }

    clauses.join(" ")
}

pub fn get_forward_batch(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
    above_uid: u32,
    modifiers: &Modifiers,
    limit: u32,
) -> Result<Vec<ClassifyCandidate>, EddieError> {
    let conn = pool.get()?;
    let modifier_clause = build_modifier_clause(modifiers, account_id);
    let query = format!(
        "SELECT id, imap_uid, subject, body_text FROM messages
         WHERE account_id = ?1 AND imap_folder = ?2
           AND imap_uid > ?3
           AND processed_at IS NOT NULL
           AND body_text IS NOT NULL
           {}
         ORDER BY imap_uid ASC
         LIMIT ?4",
        modifier_clause
    );

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(params![account_id, folder, above_uid as i64, limit as i64], |row| {
        Ok(ClassifyCandidate {
            id: row.get(0)?,
            imap_uid: row.get(1)?,
            subject: row.get(2)?,
            body_text: row.get(3)?,
        })
    })?;

    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }
    Ok(candidates)
}

pub fn get_backward_batch(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
    below_uid: u32,
    modifiers: &Modifiers,
    limit: u32,
) -> Result<Vec<ClassifyCandidate>, EddieError> {
    let conn = pool.get()?;
    let modifier_clause = build_modifier_clause(modifiers, account_id);
    let query = format!(
        "SELECT id, imap_uid, subject, body_text FROM messages
         WHERE account_id = ?1 AND imap_folder = ?2
           AND imap_uid < ?3
           AND processed_at IS NOT NULL
           AND body_text IS NOT NULL
           {}
         ORDER BY imap_uid DESC
         LIMIT ?4",
        modifier_clause
    );

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(params![account_id, folder, below_uid as i64, limit as i64], |row| {
        Ok(ClassifyCandidate {
            id: row.get(0)?,
            imap_uid: row.get(1)?,
            subject: row.get(2)?,
            body_text: row.get(3)?,
        })
    })?;

    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }
    Ok(candidates)
}

// ---------------------------------------------------------------------------
// Match CRUD
// ---------------------------------------------------------------------------

pub fn insert_matches_batch(
    pool: &DbPool,
    skill_id: &str,
    message_ids: &[String],
) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    let mut count = 0;
    for msg_id in message_ids {
        count += conn.execute(
            "INSERT OR IGNORE INTO skill_matches (skill_id, message_id, matched_at)
             VALUES (?1, ?2, ?3)",
            params![skill_id, msg_id, now],
        )?;
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get distinct folders that contain messages for an account.
pub fn get_message_folders(
    pool: &DbPool,
    account_id: &str,
) -> Result<Vec<String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT imap_folder FROM messages WHERE account_id = ?1",
    )?;
    let rows = stmt.query_map(params![account_id], |row| row.get(0))?;
    let mut folders = Vec::new();
    for row in rows {
        folders.push(row?);
    }
    Ok(folders)
}
