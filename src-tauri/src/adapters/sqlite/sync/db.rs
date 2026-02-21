use std::path::PathBuf;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

use crate::error::EddieError;

// This defines a type alias — a shorthand so we don't have to write
// the full Pool<SqliteConnectionManager> everywhere.
pub type DbPool = Pool<SqliteConnectionManager>;

/// Creates the sync database directory, connection pool, and initializes the schema.
pub fn initialize(app: &tauri::AppHandle) -> Result<DbPool, EddieError> {
    let db_dir = get_sync_db_dir(app);
    std::fs::create_dir_all(&db_dir)
        .map_err(|e| EddieError::Database(format!("Failed to create sync db dir: {e}")))?;

    let db_path = db_dir.join("sync.db");
    let pool = create_pool(&db_path)?;

    let conn = pool.get()
        .map_err(|e| EddieError::Database(e.to_string()))?;
    super::db_schema::initialize_schema(&conn)?;

    Ok(pool)
}

/// Get the sync database directory path
/// On mobile, uses Tauri's path API which correctly resolves the app's sandboxed data dir.
/// On desktop debug, uses a local relative path; on desktop release, uses the system data dir.
fn get_sync_db_dir(app: &tauri::AppHandle) -> PathBuf {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        use tauri::Manager;
        app.path()
            .app_data_dir()
            .expect("Failed to determine app data directory")
            .join("sync")
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        let _ = app; // suppress unused warning on desktop
        if cfg!(debug_assertions) {
            PathBuf::from("../.sqlite")
        } else {
            dirs::data_local_dir()
                .expect("Failed to determine data directory for desktop")
                .join("eddie.chat")
                .join("sync")
        }
    }
}

fn create_pool(db_path: &Path) -> Result<DbPool, EddieError> {
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
