use rusqlite::params;
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
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_deserializing)]
    pub has_model: bool,
}

pub fn list_skills(pool: &DbPool, account_id: &str) -> Result<Vec<Skill>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, name, icon, icon_bg, enabled, prompt, modifiers, settings, created_at, updated_at
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
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
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
        "SELECT id, account_id, name, icon, icon_bg, enabled, prompt, modifiers, settings, created_at, updated_at
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
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
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

    conn.execute(
        "INSERT INTO skills (id, account_id, name, icon, icon_bg, enabled, prompt, modifiers, settings, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10)",
        params![id, account_id, name, icon, icon_bg, prompt, modifiers, settings, now, now],
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

    conn.execute(
        "UPDATE skills SET name = ?1, icon = ?2, icon_bg = ?3, prompt = ?4, modifiers = ?5, settings = ?6, updated_at = ?7
         WHERE id = ?8",
        params![name, icon, icon_bg, prompt, modifiers, settings, now, id],
    )?;

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

    conn.execute("DELETE FROM skills WHERE id = ?1", params![skill_id])?;

    Ok(())
}
