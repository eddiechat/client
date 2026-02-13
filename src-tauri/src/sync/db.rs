//! Config Database for account settings and credentials
//!
//! This module manages the Config DB (config.db) which stores:
//! - Connection configurations (IMAP/SMTP settings, encrypted passwords)
//! - Application settings (read-only mode, etc.)
//!
//! This is separate from the sync DB (sync.db) which caches email data.

use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tracing::info;

use crate::types::error::EddieError;

/// Database connection pool type
pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConnection = r2d2::PooledConnection<SqliteConnectionManager>;

/// Connection configuration stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConnectionConfig {
    pub account_id: String,  // Primary key - stores email address
    pub active: bool,
    pub email: String,
    pub display_name: Option<String>,
    pub aliases: Option<String>, // Comma-separated list of email aliases
    pub imap_config: Option<String>, // JSON serialized ImapConfig
    pub smtp_config: Option<String>, // JSON serialized SmtpConfig
    pub encrypted_password: Option<String>, // Device-encrypted password (base64)
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========== Global Config Database ==========

/// Global config database instance
static CONFIG_DB: OnceCell<RwLock<ConfigDatabase>> = OnceCell::new();

/// Get the config database directory path
fn get_config_db_dir() -> PathBuf {
    // On mobile platforms (iOS/Android), always use data_dir() even in debug mode
    // because the current directory is read-only
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        dirs::data_dir()
            .expect("Failed to determine data directory for iOS/Android")
            .join("eddie.chat")
            .join("config")
    }

    // On desktop, use ../.sqlite in debug mode for easier debugging
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        if cfg!(debug_assertions) {
            PathBuf::from("../.sqlite")
        } else {
            dirs::data_local_dir()
                .expect("Failed to determine data directory for desktop")
                .join("eddie.chat")
                .join("config")
        }
    }
}

/// Get the config database file path
fn get_config_db_path() -> PathBuf {
    get_config_db_dir().join("config.db")
}

/// Configuration database for storing connection configs
pub struct ConfigDatabase {
    pool: DbPool,
}

impl ConfigDatabase {
    /// Create a new config database at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, EddieError> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder().max_size(5).build(manager).map_err(|e| {
            EddieError::Backend(format!("Failed to create config database pool: {}", e))
        })?;

        let db = Self { pool };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Get a connection from the pool
    fn connection(&self) -> Result<DbConnection, EddieError> {
        self.pool.get().map_err(|e| {
            EddieError::Backend(format!("Failed to get config database connection: {}", e))
        })
    }

    /// Initialize the config database schema
    fn initialize_schema(&self) -> Result<(), EddieError> {
        let conn = self.connection()?;

        conn.execute_batch(
            r#"
            -- Enable foreign keys and WAL mode
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            -- Connection configuration (for account switching)
            CREATE TABLE IF NOT EXISTS connection_configs (
                account_id TEXT PRIMARY KEY,  -- Stores email address
                active INTEGER DEFAULT 0,
                email TEXT NOT NULL,
                display_name TEXT,
                aliases TEXT,  -- Comma-separated list of email aliases
                imap_config TEXT,  -- JSON serialized ImapConfig
                smtp_config TEXT,  -- JSON serialized SmtpConfig
                encrypted_password TEXT,  -- Device-encrypted password (base64)
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- Index for quickly finding the active account
            CREATE INDEX IF NOT EXISTS idx_connection_configs_active ON connection_configs(active);

            -- Application settings (global settings)
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
        "#,
        )
        .map_err(|e| {
            EddieError::Backend(format!("Failed to initialize config schema: {}", e))
        })?;

        // Migrate old schema if needed
        self.migrate_schema(&conn)?;

        Ok(())
    }

    /// Migrate database schema from old versions
    fn migrate_schema(&self, conn: &DbConnection) -> Result<(), EddieError> {
        // Check if 'name' column exists (old schema)
        let has_name_column: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('connection_configs') WHERE name = 'name'",
                [],
                |row| row.get::<_, i32>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if has_name_column {
            info!("Migrating connection_configs table: removing 'name' column and updating account_id to use email");

            conn.execute_batch(
                r#"
                CREATE TABLE connection_configs_new (
                    account_id TEXT PRIMARY KEY,
                    active INTEGER DEFAULT 0,
                    email TEXT NOT NULL,
                    display_name TEXT,
                    imap_config TEXT,
                    smtp_config TEXT,
                    encrypted_password TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                INSERT INTO connection_configs_new (account_id, active, email, display_name, imap_config, smtp_config, created_at, updated_at)
                SELECT email, active, email, display_name, imap_config, smtp_config, created_at, updated_at
                FROM connection_configs;

                DROP TABLE connection_configs;
                ALTER TABLE connection_configs_new RENAME TO connection_configs;
                CREATE INDEX idx_connection_configs_active ON connection_configs(active);
                "#,
            )
            .map_err(|e| {
                EddieError::Backend(format!("Failed to migrate connection_configs table: {}", e))
            })?;

            info!("Migration complete");
        }

        // Migration: Add encrypted_password column if it doesn't exist
        let has_encrypted_password: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('connection_configs') WHERE name = 'encrypted_password'",
                [],
                |row| row.get::<_, i32>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_encrypted_password {
            info!("Adding encrypted_password column to connection_configs table");
            conn.execute(
                "ALTER TABLE connection_configs ADD COLUMN encrypted_password TEXT",
                [],
            )
            .map_err(|e| {
                EddieError::Backend(format!("Failed to add encrypted_password column: {}", e))
            })?;
        }

        // Migration: Add aliases column if it doesn't exist
        let has_aliases: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('connection_configs') WHERE name = 'aliases'",
                [],
                |row| row.get::<_, i32>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_aliases {
            info!("Adding aliases column to connection_configs table");
            conn.execute(
                "ALTER TABLE connection_configs ADD COLUMN aliases TEXT",
                [],
            )
            .map_err(|e| {
                EddieError::Backend(format!("Failed to add aliases column: {}", e))
            })?;
        }

        Ok(())
    }

    /// Save or update a connection configuration
    pub fn upsert_connection_config(&self, config: &EmailConnectionConfig) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO connection_configs (account_id, active, email, display_name, aliases, imap_config, smtp_config, encrypted_password, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
             ON CONFLICT(account_id) DO UPDATE SET
                active = excluded.active,
                email = excluded.email,
                display_name = excluded.display_name,
                aliases = excluded.aliases,
                imap_config = excluded.imap_config,
                smtp_config = excluded.smtp_config,
                encrypted_password = excluded.encrypted_password,
                updated_at = datetime('now')",
            params![
                config.account_id,
                config.active as i32,
                config.email,
                config.display_name,
                config.aliases,
                config.imap_config,
                config.smtp_config,
                config.encrypted_password,
            ],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get a connection configuration by account_id
    pub fn get_connection_config(
        &self,
        account_id: &str,
    ) -> Result<Option<EmailConnectionConfig>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT account_id, active, email, display_name, aliases, imap_config, smtp_config, encrypted_password, created_at, updated_at
                 FROM connection_configs WHERE account_id = ?1",
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![account_id], Self::row_to_connection_config)
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Get all connection configurations
    pub fn get_all_connection_configs(&self) -> Result<Vec<EmailConnectionConfig>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT account_id, active, email, display_name, aliases, imap_config, smtp_config, encrypted_password, created_at, updated_at
                 FROM connection_configs ORDER BY COALESCE(display_name, email) ASC",
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let configs = stmt
            .query_map([], Self::row_to_connection_config)
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(configs)
    }

    /// Get the currently active connection configuration
    pub fn get_active_connection_config(&self) -> Result<Option<EmailConnectionConfig>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT account_id, active, email, display_name, aliases, imap_config, smtp_config, encrypted_password, created_at, updated_at
                 FROM connection_configs WHERE active = 1 LIMIT 1",
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row([], Self::row_to_connection_config)
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Set an account as active (deactivates all others)
    pub fn set_active_account(&self, account_id: &str) -> Result<(), EddieError> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        tx.execute("UPDATE connection_configs SET active = 0", [])
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        tx.execute(
            "UPDATE connection_configs SET active = 1, updated_at = datetime('now') WHERE account_id = ?1",
            params![account_id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        tx.commit()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Delete a connection configuration
    pub fn delete_connection_config(&self, account_id: &str) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM connection_configs WHERE account_id = ?1",
            params![account_id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    fn row_to_connection_config(row: &Row) -> Result<EmailConnectionConfig, rusqlite::Error> {
        Ok(EmailConnectionConfig {
            account_id: row.get(0)?,
            active: row.get::<_, i32>(1)? != 0,
            email: row.get(2)?,
            display_name: row.get(3)?,
            aliases: row.get(4)?,
            imap_config: row.get(5)?,
            smtp_config: row.get(6)?,
            encrypted_password: row.get(7)?,
            created_at: row
                .get::<_, String>(8)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: row
                .get::<_, String>(9)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        })
    }

    /// Get an application setting by key
    pub fn get_app_setting(&self, key: &str) -> Result<Option<String>, EddieError> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| EddieError::Database(e.to_string()))
    }

    /// Set an application setting
    pub fn set_app_setting(&self, key: &str, value: &str) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = datetime('now')",
            params![key, value],
        )
        .map_err(|e| EddieError::Database(e.to_string()))?;
        Ok(())
    }
}

// ========== Global Config Database Functions ==========

/// Initialize the global config database
pub fn init_config_db() -> Result<(), EddieError> {
    let db_dir = get_config_db_dir();
    std::fs::create_dir_all(&db_dir).map_err(|e| {
        EddieError::Backend(format!("Failed to create config database directory: {}", e))
    })?;

    let db_path = get_config_db_path();
    info!("Initializing config database at: {:?}", db_path);

    let db = ConfigDatabase::new(&db_path)?;

    match CONFIG_DB.get() {
        Some(lock) => {
            let mut guard = lock
                .write()
                .map_err(|e| EddieError::Backend(format!("Failed to lock config db: {}", e)))?;
            *guard = db;
        }
        None => {
            CONFIG_DB.set(RwLock::new(db)).ok();
        }
    }

    Ok(())
}

/// Check if config database is initialized
pub fn is_config_db_initialized() -> bool {
    CONFIG_DB.get().is_some()
}

/// Get the global config database (initializes if needed)
pub fn get_config_db() -> Result<std::sync::RwLockReadGuard<'static, ConfigDatabase>, EddieError>
{
    if !is_config_db_initialized() {
        init_config_db()?;
    }

    CONFIG_DB
        .get()
        .ok_or_else(|| EddieError::Backend("Config database not initialized".to_string()))?
        .read()
        .map_err(|e| EddieError::Backend(format!("Failed to lock config db for read: {}", e)))
}

/// Save a connection config to the global database
pub fn save_connection_config(config: &EmailConnectionConfig) -> Result<(), EddieError> {
    let db = get_config_db()?;
    db.upsert_connection_config(config)
}

/// Get a connection config from the global database
pub fn get_connection_config(account_id: &str) -> Result<Option<EmailConnectionConfig>, EddieError> {
    let db = get_config_db()?;
    db.get_connection_config(account_id)
}

/// Get all connection configs from the global database
pub fn get_all_connection_configs() -> Result<Vec<EmailConnectionConfig>, EddieError> {
    let db = get_config_db()?;
    db.get_all_connection_configs()
}

/// Get the active connection config from the global database
pub fn get_active_connection_config() -> Result<Option<EmailConnectionConfig>, EddieError> {
    let db = get_config_db()?;
    db.get_active_connection_config()
}

/// Set an account as active in the global database
pub fn set_active_account(account_id: &str) -> Result<(), EddieError> {
    let db = get_config_db()?;
    db.set_active_account(account_id)
}

/// Delete a connection config from the global database
pub fn delete_connection_config(account_id: &str) -> Result<(), EddieError> {
    let db = get_config_db()?;
    db.delete_connection_config(account_id)
}

/// Get an application setting from the global database
pub fn get_app_setting(key: &str) -> Result<String, EddieError> {
    let db = get_config_db()?;
    match db.get_app_setting(key)? {
        Some(value) => Ok(value),
        None => {
            // Default to "true" for read_only_mode if not set
            if key == "read_only_mode" {
                info!("Read-only mode not set, defaulting to true");
                db.set_app_setting(key, "true")?;
                Ok("true".to_string())
            } else {
                Err(EddieError::Config(format!("Setting not found: {}", key)))
            }
        }
    }
}

/// Set an application setting in the global database
pub fn set_app_setting(key: &str, value: &str) -> Result<(), EddieError> {
    let db = get_config_db()?;
    db.set_app_setting(key, value)
}

/// Check if read-only mode is enabled
pub fn is_read_only_mode() -> Result<bool, EddieError> {
    get_app_setting("read_only_mode").map(|s| s == "true")
}
