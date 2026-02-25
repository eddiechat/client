use rusqlite::params;
use sha2::{Sha256, Digest};
use uuid::Uuid;

use super::DbPool;
use crate::error::EddieError;

#[derive(serde::Serialize)]
pub struct Skill {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub icon: String,
    pub icon_bg: String,
    pub enabled: bool,
    pub prompt: String,
    pub modifiers: String,
    pub settings: String,
    pub revision_hash: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_deserializing)]
    pub has_model: bool,
}

/// Compute a revision hash from the fields that affect classification output.
/// Returns 16 hex chars (first 8 bytes of SHA-256).
pub fn compute_revision_hash(prompt: &str, settings_json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(settings_json)
        .unwrap_or(serde_json::Value::Object(Default::default()));
    let model = parsed.get("ollamaModel")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let temperature = parsed.get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let input = format!("{}\0{}\0{}", prompt, model, temperature);
    let digest = Sha256::digest(input.as_bytes());
    digest[..8].iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn list_skills(pool: &DbPool, account_id: &str) -> Result<Vec<Skill>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, name, icon, icon_bg, enabled, prompt, modifiers, settings, revision_hash, created_at, updated_at
         FROM skills WHERE account_id = ?1 ORDER BY created_at DESC",
    )?;

    let rows = stmt.query_map(params![account_id], |row| {
        Ok(Skill {
            id: row.get(0)?,
            account_id: row.get(1)?,
            name: row.get(2)?,
            icon: row.get(3)?,
            icon_bg: row.get(4)?,
            enabled: row.get::<_, i32>(5)? != 0,
            prompt: row.get(6)?,
            modifiers: row.get(7)?,
            settings: row.get(8)?,
            revision_hash: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
            has_model: true,
        })
    })?;

    let mut skills = Vec::new();
    for row in rows {
        skills.push(row?);
    }
    Ok(skills)
}

pub fn get_skill(pool: &DbPool, skill_id: &str) -> Result<Skill, EddieError> {
    let conn = pool.get()?;
    conn.query_row(
        "SELECT id, account_id, name, icon, icon_bg, enabled, prompt, modifiers, settings, revision_hash, created_at, updated_at
         FROM skills WHERE id = ?1",
        params![skill_id],
        |row| {
            Ok(Skill {
                id: row.get(0)?,
                account_id: row.get(1)?,
                name: row.get(2)?,
                icon: row.get(3)?,
                icon_bg: row.get(4)?,
                enabled: row.get::<_, i32>(5)? != 0,
                prompt: row.get(6)?,
                modifiers: row.get(7)?,
                settings: row.get(8)?,
                revision_hash: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
                has_model: true,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            EddieError::InvalidInput(format!("Skill not found: {}", skill_id))
        }
        _ => EddieError::Database(e.to_string()),
    })
}

pub fn create_skill(
    pool: &DbPool,
    account_id: &str,
    name: &str,
    icon: &str,
    icon_bg: &str,
    prompt: &str,
    modifiers: &str,
    settings: &str,
) -> Result<String, EddieError> {
    let conn = pool.get()?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let rev_hash = compute_revision_hash(prompt, settings);

    conn.execute(
        "INSERT INTO skills (id, account_id, name, icon, icon_bg, enabled, prompt, modifiers, settings, revision_hash, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![id, account_id, name, icon, icon_bg, prompt, modifiers, settings, rev_hash, now, now],
    )?;

    Ok(id)
}

pub fn update_skill(
    pool: &DbPool,
    id: &str,
    name: &str,
    icon: &str,
    icon_bg: &str,
    prompt: &str,
    modifiers: &str,
    settings: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    let new_rev = compute_revision_hash(prompt, settings);

    // Check if revision changed — if so, reset cursors and matches
    let old_rev: String = conn.query_row(
        "SELECT revision_hash FROM skills WHERE id = ?1",
        params![id],
        |row| row.get(0),
    ).unwrap_or_default();

    conn.execute(
        "UPDATE skills SET name = ?1, icon = ?2, icon_bg = ?3, prompt = ?4, modifiers = ?5, settings = ?6, revision_hash = ?7, updated_at = ?8
         WHERE id = ?9",
        params![name, icon, icon_bg, prompt, modifiers, settings, new_rev, now, id],
    )?;

    if new_rev != old_rev {
        super::skill_classify::reset_skill_cursors(pool, id, &new_rev)?;
    }

    Ok(())
}

pub fn toggle_skill(pool: &DbPool, skill_id: &str, enabled: bool) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "UPDATE skills SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
        params![enabled as i32, now, skill_id],
    )?;

    Ok(())
}

pub fn delete_skill(pool: &DbPool, skill_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;

    conn.execute("DELETE FROM skill_matches WHERE skill_id = ?1", params![skill_id])?;
    conn.execute("DELETE FROM folder_classify WHERE skill_id = ?1", params![skill_id])?;
    conn.execute("DELETE FROM skills WHERE id = ?1", params![skill_id])?;

    Ok(())
}
