use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

use crate::types::error::EddieError;

// This defines a type alias — a shorthand so we don't have to write
// the full Pool<SqliteConnectionManager> everywhere.
pub type DbPool = Pool<SqliteConnectionManager>;

pub fn create_pool(db_path: &Path) -> Result<DbPool, EddieError> {
    let manager = SqliteConnectionManager::file(db_path);

    let pool = Pool::builder()
        .max_size(8)
        .build(manager)?;

    // Apply SQLite performance tuning (from your spec §13.1)
    let conn = pool.get()?;

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -8000;
         PRAGMA mmap_size = 268435456;
         PRAGMA temp_store = MEMORY;
         PRAGMA foreign_keys = ON;"
    )?;

    Ok(pool)
}
