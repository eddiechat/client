use std::collections::HashMap;

use rusqlite::params;

use super::DbPool;
use crate::error::EddieError;

/// Group multiple domains under a named group.
///
/// The `name` is used as the `group_id`. Giving two groups the same name
/// effectively merges them. If any of the provided domains already belong
/// to other groups, those domains are moved into this group.
pub fn group_domains(
    pool: &DbPool,
    account_id: &str,
    name: &str,
    domains: &[String],
) -> Result<String, EddieError> {
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;

    // Insert/update all domains into this named group
    for domain in domains {
        tx.execute(
            "INSERT OR REPLACE INTO line_groups (group_id, account_id, domain) VALUES (?1, ?2, ?3)",
            params![name, account_id, domain],
        )?;
    }

    tx.commit()?;
    Ok(name.to_string())
}

/// Remove a group, releasing all domains back to individual clusters.
pub fn ungroup_domains(pool: &DbPool, account_id: &str, group_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM line_groups WHERE account_id = ?1 AND group_id = ?2",
        params![account_id, group_id],
    )?;
    Ok(())
}

/// Returns a mapping of domain → group_id for all joined domains in an account.
pub fn get_domain_to_group(
    pool: &DbPool,
    account_id: &str,
) -> Result<HashMap<String, String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT domain, group_id FROM line_groups WHERE account_id = ?1",
    )?;

    let map: HashMap<String, String> = stmt
        .query_map(params![account_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(map)
}

/// Returns all domains belonging to a specific join group.
pub fn get_domains_for_group(
    pool: &DbPool,
    account_id: &str,
    group_id: &str,
) -> Result<Vec<String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT domain FROM line_groups WHERE account_id = ?1 AND group_id = ?2 ORDER BY domain",
    )?;

    let domains: Vec<String> = stmt
        .query_map(params![account_id, group_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(domains)
}
