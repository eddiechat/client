use rusqlite::params;
use super::DbPool;
use crate::error::EddieError;

pub struct FolderState {
    pub name: String,
    pub highest_uid: u32,
    pub lowest_uid: u32,
}

pub fn ensure_folder(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT OR IGNORE INTO folder_sync (account_id, folder)
         VALUES (?1, ?2)",
        params![account_id, folder],
    )?;
    Ok(())
}

pub fn next_pending_folder(
    pool: &DbPool,
    account_id: &str,
) -> Result<Option<FolderState>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT folder, highest_uid, lowest_uid
            FROM folder_sync
            WHERE account_id = ?1 AND sync_status != 'done'
            ORDER BY
                CASE WHEN last_sync IS NULL THEN 0 ELSE 1 END,
                last_sync ASC,
                CASE WHEN folder = 'INBOX' THEN 0
                    WHEN folder LIKE '%Sent%' THEN 1
                    ELSE 2
                END
            LIMIT 1",
        params![account_id],
        |row| {
            Ok(FolderState {
                name: row.get(0)?,
                highest_uid: row.get(1)?,
                lowest_uid: row.get(2)?,
            })
        },
    );

    match result {
        Ok(state) => Ok(Some(state)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}

pub fn set_status(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
    status: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE folder_sync SET sync_status = ?1, last_sync = ?2
         WHERE account_id = ?3 AND folder = ?4",
        params![status, now, account_id, folder],
    )?;
    Ok(())
}

pub fn update_lowest_uid(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE folder_sync SET lowest_uid = ?1
         WHERE account_id = ?2 AND folder = ?3
         AND (lowest_uid = 0 OR lowest_uid > ?1)",
        params![uid as i64, account_id, folder],
    )?;
    Ok(())
}

pub fn update_highest_uid(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE folder_sync SET highest_uid = ?1
         WHERE account_id = ?2 AND folder = ?3
         AND highest_uid < ?1",
        params![uid as i64, account_id, folder],
    )?;
    Ok(())
}

pub fn get_folder(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
) -> Result<Option<FolderState>, EddieError> {
    let conn = pool.get()?;
    let result = conn.query_row(
        "SELECT folder, highest_uid, lowest_uid
         FROM folder_sync
         WHERE account_id = ?1 AND folder = ?2",
        params![account_id, folder],
        |row| Ok(FolderState {
            name: row.get(0)?,
            highest_uid: row.get(1)?,
            lowest_uid: row.get(2)?,
        }),
    );
    match result {
        Ok(state) => Ok(Some(state)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EddieError::Database(e.to_string())),
    }
}
