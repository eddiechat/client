//! SQLite database for IMAP sync cache
//!
//! This module provides all database operations for the sync engine.
//! The database is a cache of server state - all data can be rebuilt from IMAP.

use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tracing::{debug, info};

use crate::types::error::EddieError;

/// Database connection pool type
pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConnection = PooledConnection<SqliteConnectionManager>;

/// Sync state for a folder
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct FolderSyncState {
    pub account_id: String,
    pub folder_name: String,
    pub uidvalidity: Option<u32>,
    pub highestmodseq: Option<u64>,
    pub last_seen_uid: Option<u32>,
    pub last_sync_timestamp: Option<DateTime<Utc>>,
    pub sync_in_progress: bool,
}

/// Cached message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedChatMessage {
    pub id: i64,
    pub account_id: String,
    pub folder_name: String,
    pub uid: u32,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: String,         // JSON array
    pub cc_addresses: Option<String>, // JSON array
    pub subject: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub flags: String, // JSON array of flags
    pub has_attachment: bool,
    pub body_cached: bool,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub raw_size: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Conversation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedConversation {
    pub id: i64,
    pub account_id: String,
    pub participant_key: String, // Normalized, sorted participant emails
    pub participants: String,    // JSON array of participant info
    pub last_message_date: Option<DateTime<Utc>>,
    pub last_message_preview: Option<String>,
    pub last_message_from: Option<String>,
    pub message_count: u32,
    pub unread_count: u32,
    pub is_outgoing: bool, // Last message direction
    pub classification: Option<String>, // NULL until a 'chat' message is found, then 'chat'
    pub canonical_subject: Option<String>, // Meaningful subject line for the conversation (stripped of "via Eddie")
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Message-to-conversation mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub conversation_id: i64,
    pub message_id: i64,
}

/// Queued action for offline support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedActionRecord {
    pub id: i64,
    pub account_id: String,
    pub action_type: String, // add_flags, remove_flags, delete, move, send
    pub folder_name: Option<String>,
    pub uid: Option<u32>,
    pub payload: String, // JSON payload
    pub created_at: DateTime<Utc>,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub status: String, // pending, processing, failed, completed
}

/// Message classification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageClassification {
    pub message_id: i64,
    pub classification: String, // chat, newsletter, automated, transactional
    pub confidence: f32,
    pub is_hidden_from_chat: bool,
    pub classified_at: DateTime<Utc>,
}

/// Sync progress tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub account_id: String,
    pub folder_name: String,
    pub phase: String, // initial, incremental, complete
    pub total_messages: Option<u32>,
    pub synced_messages: u32,
    pub oldest_synced_date: Option<DateTime<Utc>>,
    pub last_batch_uid: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Entity (participant) for autocomplete and connection tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: i64,
    pub account_id: String,
    pub email: String,
    pub name: Option<String>,
    pub is_connection: bool,          // True if user has sent email to this entity
    pub latest_contact: DateTime<Utc>, // Most recent interaction timestamp
    pub contact_count: u32,           // Number of interactions
}

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

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self, EddieError> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).map_err(|e| {
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
        "#,
        )
        .map_err(|e| {
            EddieError::Backend(format!("Failed to initialize config schema: {}", e))
        })?;

        // Migrate old schema if needed (remove 'name' column if it exists)
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

            // Migrate data: update account_id to use email instead of name
            conn.execute_batch(
                r#"
                -- Create new table with correct schema
                CREATE TABLE connection_configs_new (
                    account_id TEXT PRIMARY KEY,  -- Now stores email address
                    active INTEGER DEFAULT 0,
                    email TEXT NOT NULL,
                    display_name TEXT,
                    imap_config TEXT,
                    smtp_config TEXT,
                    encrypted_password TEXT,  -- Device-encrypted password
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                -- Copy data, using email as the new account_id
                INSERT INTO connection_configs_new (account_id, active, email, display_name, imap_config, smtp_config, created_at, updated_at)
                SELECT email, active, email, display_name, imap_config, smtp_config, created_at, updated_at
                FROM connection_configs;

                -- Drop old table
                DROP TABLE connection_configs;

                -- Rename new table
                ALTER TABLE connection_configs_new RENAME TO connection_configs;

                -- Recreate index
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
            info!("encrypted_password column added successfully");
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
            info!("aliases column added successfully");
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

        // Deactivate all accounts
        tx.execute("UPDATE connection_configs SET active = 0", [])
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Activate the specified account
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

/// SQLite database for sync cache
pub struct SyncDatabase {
    pool: DbPool,
}

impl SyncDatabase {
    /// Create a new database at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, EddieError> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder().max_size(10).build(manager).map_err(|e| {
            EddieError::Backend(format!("Failed to create database pool: {}", e))
        })?;

        let db = Self { pool };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self, EddieError> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).map_err(|e| {
            EddieError::Backend(format!("Failed to create database pool: {}", e))
        })?;

        let db = Self { pool };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Get a connection from the pool
    pub fn connection(&self) -> Result<DbConnection, EddieError> {
        self.pool.get().map_err(|e| {
            EddieError::Backend(format!("Failed to get database connection: {}", e))
        })
    }

    /// Initialize the database schema
    fn initialize_schema(&self) -> Result<(), EddieError> {
        let conn = self.connection()?;

        conn.execute_batch(r#"
            -- Enable foreign keys and WAL mode for better concurrency
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = -64000;  -- 64MB cache

            -- Folder sync state table
            CREATE TABLE IF NOT EXISTS folder_sync_state (
                account_id TEXT NOT NULL,
                folder_name TEXT NOT NULL,
                uidvalidity INTEGER,
                highestmodseq INTEGER,
                last_seen_uid INTEGER,
                last_sync_timestamp TEXT,
                sync_in_progress INTEGER DEFAULT 0,
                PRIMARY KEY (account_id, folder_name)
            );

            -- Cached messages table
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL,
                folder_name TEXT NOT NULL,
                uid INTEGER NOT NULL,
                message_id TEXT,
                in_reply_to TEXT,
                references_header TEXT,
                from_address TEXT NOT NULL,
                from_name TEXT,
                to_addresses TEXT NOT NULL,  -- JSON array
                cc_addresses TEXT,  -- JSON array
                subject TEXT,
                date TEXT,
                flags TEXT NOT NULL DEFAULT '[]',  -- JSON array
                has_attachment INTEGER DEFAULT 0,
                body_cached INTEGER DEFAULT 0,
                text_body TEXT,
                html_body TEXT,
                raw_size INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(account_id, folder_name, uid)
            );

            -- Index for efficient message lookups
            CREATE INDEX IF NOT EXISTS idx_messages_account_folder ON messages(account_id, folder_name);
            CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(date DESC);
            CREATE INDEX IF NOT EXISTS idx_messages_message_id ON messages(message_id);
            CREATE INDEX IF NOT EXISTS idx_messages_from ON messages(from_address);

            -- Partial UNIQUE index to prevent duplicate messages with same Message-ID header
            -- Only enforces uniqueness when message_id is NOT NULL and not empty
            -- This prevents duplicate sent messages while allowing null message_ids for edge cases
            CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_unique_message_id
            ON messages(account_id, message_id)
            WHERE message_id IS NOT NULL AND message_id != '';

            -- Conversations table
            CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL,
                participant_key TEXT NOT NULL,  -- Normalized sorted participant emails
                participants TEXT NOT NULL,  -- JSON array of {email, name}
                last_message_date TEXT,
                last_message_preview TEXT,
                last_message_from TEXT,
                message_count INTEGER DEFAULT 0,
                unread_count INTEGER DEFAULT 0,
                is_outgoing INTEGER DEFAULT 0,
                classification TEXT,  -- NULL until a 'chat' message is found, then set to 'chat'
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(account_id, participant_key)
            );

            -- Index for conversation lookups
            CREATE INDEX IF NOT EXISTS idx_conversations_account ON conversations(account_id);
            CREATE INDEX IF NOT EXISTS idx_conversations_last_date ON conversations(last_message_date DESC);
            CREATE INDEX IF NOT EXISTS idx_conversations_participant_key ON conversations(participant_key);
            CREATE INDEX IF NOT EXISTS idx_conversations_classification ON conversations(classification);

            -- Migration: Add canonical_subject column if it doesn't exist
            -- This stores the meaningful subject line for the conversation (stripped of "via Eddie")
        "#).map_err(|e| EddieError::Backend(e.to_string()))?;

        // Check if canonical_subject column exists
        let column_exists = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name='canonical_subject'",
                [],
                |row| row.get::<_, i32>(0),
            )
            .unwrap_or(0)
            > 0;

        if !column_exists {
            info!("Adding canonical_subject column to conversations table");
            conn.execute("ALTER TABLE conversations ADD COLUMN canonical_subject TEXT", [])
                .map_err(|e| EddieError::Backend(e.to_string()))?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_conversations_canonical_subject ON conversations(canonical_subject)",
                [],
            ).map_err(|e| EddieError::Backend(e.to_string()))?;
        }

        conn.execute_batch(r#"

            -- Message to conversation mapping
            CREATE TABLE IF NOT EXISTS conversation_messages (
                conversation_id INTEGER NOT NULL,
                message_id INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, message_id),
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
                FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_conv_msg_conversation ON conversation_messages(conversation_id);
            CREATE INDEX IF NOT EXISTS idx_conv_msg_message ON conversation_messages(message_id);

            -- Action queue for offline support
            CREATE TABLE IF NOT EXISTS action_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL,
                action_type TEXT NOT NULL,  -- add_flags, remove_flags, delete, move, send
                folder_name TEXT,
                uid INTEGER,
                payload TEXT NOT NULL,  -- JSON payload
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                retry_count INTEGER DEFAULT 0,
                last_error TEXT,
                status TEXT NOT NULL DEFAULT 'pending'  -- pending, processing, failed, completed
            );

            CREATE INDEX IF NOT EXISTS idx_action_queue_status ON action_queue(status, created_at);
            CREATE INDEX IF NOT EXISTS idx_action_queue_account ON action_queue(account_id);

            -- Message classification
            CREATE TABLE IF NOT EXISTS message_classifications (
                message_id INTEGER PRIMARY KEY,
                classification TEXT NOT NULL,  -- chat, newsletter, automated, transactional
                confidence REAL NOT NULL,
                is_hidden_from_chat INTEGER DEFAULT 0,
                classified_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
            );

            -- Sync progress tracking (for resumable initial sync)
            CREATE TABLE IF NOT EXISTS sync_progress (
                account_id TEXT NOT NULL,
                folder_name TEXT NOT NULL,
                phase TEXT NOT NULL,  -- initial, incremental, complete
                total_messages INTEGER,
                synced_messages INTEGER DEFAULT 0,
                oldest_synced_date TEXT,
                last_batch_uid INTEGER,
                started_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (account_id, folder_name)
            );

            -- Server capabilities cache
            CREATE TABLE IF NOT EXISTS server_capabilities (
                account_id TEXT PRIMARY KEY,
                capabilities TEXT NOT NULL,  -- JSON array
                supports_qresync INTEGER DEFAULT 0,
                supports_condstore INTEGER DEFAULT 0,
                supports_idle INTEGER DEFAULT 0,
                detected_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- Entities table for participant tracking and autocomplete
            CREATE TABLE IF NOT EXISTS entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL,
                email TEXT NOT NULL,
                name TEXT,
                is_connection INTEGER DEFAULT 0,  -- 1 if user has sent email to this entity
                latest_contact TEXT NOT NULL DEFAULT (datetime('now')),
                contact_count INTEGER DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(account_id, email)
            );

            -- Indexes for entity lookups and autocomplete
            CREATE INDEX IF NOT EXISTS idx_entities_account ON entities(account_id);
            CREATE INDEX IF NOT EXISTS idx_entities_email ON entities(account_id, email);
            CREATE INDEX IF NOT EXISTS idx_entities_connection ON entities(account_id, is_connection);
            CREATE INDEX IF NOT EXISTS idx_entities_latest_contact ON entities(account_id, latest_contact DESC);
        "#).map_err(|e| EddieError::Backend(format!("Failed to initialize schema: {}", e)))?;

        Ok(())
    }

    // ========== Folder Sync State Operations ==========

    /// Get sync state for a folder
    pub fn get_folder_sync_state(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<Option<FolderSyncState>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT account_id, folder_name, uidvalidity, highestmodseq, last_seen_uid, last_sync_timestamp, sync_in_progress
             FROM folder_sync_state WHERE account_id = ?1 AND folder_name = ?2"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![account_id, folder_name], |row| {
                Ok(FolderSyncState {
                    account_id: row.get(0)?,
                    folder_name: row.get(1)?,
                    uidvalidity: row.get(2)?,
                    highestmodseq: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                    last_seen_uid: row.get(4)?,
                    last_sync_timestamp: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    sync_in_progress: row.get::<_, i32>(6)? != 0,
                })
            })
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Update or insert folder sync state
    pub fn upsert_folder_sync_state(&self, state: &FolderSyncState) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO folder_sync_state (account_id, folder_name, uidvalidity, highestmodseq, last_seen_uid, last_sync_timestamp, sync_in_progress)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(account_id, folder_name) DO UPDATE SET
                uidvalidity = excluded.uidvalidity,
                highestmodseq = excluded.highestmodseq,
                last_seen_uid = excluded.last_seen_uid,
                last_sync_timestamp = excluded.last_sync_timestamp,
                sync_in_progress = excluded.sync_in_progress",
            params![
                state.account_id,
                state.folder_name,
                state.uidvalidity,
                state.highestmodseq.map(|v| v as i64),
                state.last_seen_uid,
                state.last_sync_timestamp.map(|dt| dt.to_rfc3339()),
                state.sync_in_progress as i32,
            ],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get the stored UIDVALIDITY for a folder
    pub fn get_folder_uidvalidity(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<Option<u32>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT uidvalidity FROM folder_sync_state WHERE account_id = ?1 AND folder_name = ?2",
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![account_id, folder_name], |row| row.get(0))
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Set/update the UIDVALIDITY for a folder
    pub fn set_folder_uidvalidity(
        &self,
        account_id: &str,
        folder_name: &str,
        uidvalidity: u32,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO folder_sync_state (account_id, folder_name, uidvalidity)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(account_id, folder_name) DO UPDATE SET
                uidvalidity = excluded.uidvalidity",
            params![account_id, folder_name, uidvalidity],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Invalidate folder cache (when UIDVALIDITY changes)
    pub fn invalidate_folder_cache(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<(), EddieError> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Delete all messages in this folder
        tx.execute(
            "DELETE FROM messages WHERE account_id = ?1 AND folder_name = ?2",
            params![account_id, folder_name],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Reset sync state
        tx.execute(
            "DELETE FROM folder_sync_state WHERE account_id = ?1 AND folder_name = ?2",
            params![account_id, folder_name],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Reset sync progress
        tx.execute(
            "DELETE FROM sync_progress WHERE account_id = ?1 AND folder_name = ?2",
            params![account_id, folder_name],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        tx.commit()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    // ========== Message Operations ==========

    /// Insert or update a message
    pub fn upsert_message(&self, msg: &CachedChatMessage) -> Result<i64, EddieError> {
        // Check for duplicate message_id before inserting
        // This prevents duplicate sent messages from appearing in conversations
        if let Some(ref message_id) = msg.message_id {
            if !message_id.is_empty() {
                if let Some(existing_id) = self.find_message_by_message_id(&msg.account_id, message_id)? {
                    debug!(
                        "🔍 Duplicate message_id detected: {} (existing row: {}), skipping insert",
                        message_id, existing_id
                    );
                    return Ok(existing_id);
                }
            }
        }

        debug!(
            "💾 Inserting/updating message: message_id={:?}, uid={}, folder={}",
            msg.message_id, msg.uid, msg.folder_name
        );

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO messages (account_id, folder_name, uid, message_id, in_reply_to, references_header,
                from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                has_attachment, body_cached, text_body, html_body, raw_size, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, datetime('now'))
             ON CONFLICT(account_id, folder_name, uid) DO UPDATE SET
                message_id = excluded.message_id,
                in_reply_to = excluded.in_reply_to,
                references_header = excluded.references_header,
                from_address = excluded.from_address,
                from_name = excluded.from_name,
                to_addresses = excluded.to_addresses,
                cc_addresses = excluded.cc_addresses,
                subject = excluded.subject,
                date = excluded.date,
                flags = excluded.flags,
                has_attachment = excluded.has_attachment,
                body_cached = CASE WHEN excluded.body_cached = 1 THEN 1 ELSE messages.body_cached END,
                text_body = CASE WHEN excluded.body_cached = 1 THEN excluded.text_body ELSE messages.text_body END,
                html_body = CASE WHEN excluded.body_cached = 1 THEN excluded.html_body ELSE messages.html_body END,
                raw_size = excluded.raw_size,
                updated_at = datetime('now')",
            params![
                msg.account_id,
                msg.folder_name,
                msg.uid,
                msg.message_id,
                msg.in_reply_to,
                msg.references,
                msg.from_address,
                msg.from_name,
                msg.to_addresses,
                msg.cc_addresses,
                msg.subject,
                msg.date.map(|dt| dt.to_rfc3339()),
                msg.flags,
                msg.has_attachment as i32,
                msg.body_cached as i32,
                msg.text_body,
                msg.html_body,
                msg.raw_size,
            ],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        // Get the row ID
        let id = conn
            .query_row(
                "SELECT id FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3",
                params![msg.account_id, msg.folder_name, msg.uid],
                |row| row.get(0),
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(id)
    }

    /// Get a message by account/folder/uid
    pub fn get_message(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
    ) -> Result<Option<CachedChatMessage>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(
                params![account_id, folder_name, uid],
                Self::row_to_cached_message,
            )
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Get a message by its internal ID
    pub fn get_message_by_id(&self, id: i64) -> Result<Option<CachedChatMessage>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE id = ?1"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![id], Self::row_to_cached_message)
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Find a message by Message-ID header (for deduplication)
    /// Returns the database row ID if a message with this Message-ID already exists
    pub fn find_message_by_message_id(
        &self,
        account_id: &str,
        message_id: &str,
    ) -> Result<Option<i64>, EddieError> {
        let conn = self.connection()?;
        let result = conn
            .query_row(
                "SELECT id FROM messages WHERE account_id = ?1 AND message_id = ?2 LIMIT 1",
                params![account_id, message_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Get all messages for an account
    pub fn get_all_messages_for_account(&self, account_id: &str) -> Result<Vec<CachedChatMessage>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE account_id = ?1
             ORDER BY date ASC"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages = stmt
            .query_map(params![account_id], Self::row_to_cached_message)
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    /// Update flags for a message (replaces all flags)
    pub fn update_message_flags(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags: &str,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE messages SET flags = ?1, updated_at = datetime('now')
             WHERE account_id = ?2 AND folder_name = ?3 AND uid = ?4",
            params![flags, account_id, folder_name, uid],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Add flags to a message (preserves existing flags)
    pub fn add_message_flags(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags_to_add: &[String],
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;

        // Get current flags
        let current_flags: Option<String> = conn
            .query_row(
                "SELECT flags FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3",
                params![account_id, folder_name, uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .flatten();

        let mut flags: Vec<String> = current_flags
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        // Add new flags (avoid duplicates)
        for flag in flags_to_add {
            if !flags.contains(flag) {
                flags.push(flag.clone());
            }
        }

        let flags_json =
            serde_json::to_string(&flags).map_err(|e| EddieError::Backend(e.to_string()))?;

        conn.execute(
            "UPDATE messages SET flags = ?1, updated_at = datetime('now')
             WHERE account_id = ?2 AND folder_name = ?3 AND uid = ?4",
            params![flags_json, account_id, folder_name, uid],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Remove flags from a message
    pub fn remove_message_flags(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags_to_remove: &[String],
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;

        // Get current flags
        let current_flags: Option<String> = conn
            .query_row(
                "SELECT flags FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3",
                params![account_id, folder_name, uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .flatten();

        let mut flags: Vec<String> = current_flags
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        // Remove specified flags
        flags.retain(|f| !flags_to_remove.contains(f));

        let flags_json =
            serde_json::to_string(&flags).map_err(|e| EddieError::Backend(e.to_string()))?;

        conn.execute(
            "UPDATE messages SET flags = ?1, updated_at = datetime('now')
             WHERE account_id = ?2 AND folder_name = ?3 AND uid = ?4",
            params![flags_json, account_id, folder_name, uid],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Set flags on a message (replaces existing flags with new set)
    pub fn set_message_flags_vec(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags: &[String],
    ) -> Result<(), EddieError> {
        let flags_json =
            serde_json::to_string(flags).map_err(|e| EddieError::Backend(e.to_string()))?;

        self.update_message_flags(account_id, folder_name, uid, &flags_json)
    }

    /// Delete messages by UIDs
    pub fn delete_messages_by_uids(
        &self,
        account_id: &str,
        folder_name: &str,
        uids: &[u32],
    ) -> Result<(), EddieError> {
        if uids.is_empty() {
            return Ok(());
        }

        let conn = self.connection()?;
        let placeholders: Vec<String> = (0..uids.len()).map(|i| format!("?{}", i + 3)).collect();
        let sql = format!(
            "DELETE FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid IN ({})",
            placeholders.join(",")
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(account_id.to_string()),
            Box::new(folder_name.to_string()),
        ];
        for uid in uids {
            params.push(Box::new(*uid));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get all UIDs in a folder (for deletion detection)
    pub fn get_folder_uids(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<HashSet<u32>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare("SELECT uid FROM messages WHERE account_id = ?1 AND folder_name = ?2")
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let uids = stmt
            .query_map(params![account_id, folder_name], |row| row.get(0))
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(uids)
    }

    /// Get messages for a folder, sorted by date
    pub fn get_folder_messages(
        &self,
        account_id: &str,
        folder_name: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<CachedChatMessage>, EddieError> {
        let conn = self.connection()?;
        let sql = format!(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE account_id = ?1 AND folder_name = ?2
             ORDER BY date DESC
             LIMIT ?3 OFFSET ?4"
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages = stmt
            .query_map(
                params![
                    account_id,
                    folder_name,
                    limit.unwrap_or(1000),
                    offset.unwrap_or(0)
                ],
                Self::row_to_cached_message,
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    fn row_to_cached_message(row: &Row) -> Result<CachedChatMessage, rusqlite::Error> {
        Ok(CachedChatMessage {
            id: row.get(0)?,
            account_id: row.get(1)?,
            folder_name: row.get(2)?,
            uid: row.get(3)?,
            message_id: row.get(4)?,
            in_reply_to: row.get(5)?,
            references: row.get(6)?,
            from_address: row.get(7)?,
            from_name: row.get(8)?,
            to_addresses: row.get(9)?,
            cc_addresses: row.get(10)?,
            subject: row.get(11)?,
            date: row
                .get::<_, Option<String>>(12)?
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            flags: row.get(13)?,
            has_attachment: row.get::<_, i32>(14)? != 0,
            body_cached: row.get::<_, i32>(15)? != 0,
            text_body: row.get(16)?,
            html_body: row.get(17)?,
            raw_size: row.get(18)?,
            created_at: row
                .get::<_, String>(19)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: row
                .get::<_, String>(20)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        })
    }

    // ========== Conversation Operations ==========

    /// Insert or update a conversation
    pub fn upsert_conversation(&self, conv: &CachedConversation) -> Result<i64, EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO conversations (account_id, participant_key, participants, last_message_date,
                last_message_preview, last_message_from, message_count, unread_count, is_outgoing, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
             ON CONFLICT(account_id, participant_key) DO UPDATE SET
                participants = excluded.participants,
                last_message_date = excluded.last_message_date,
                last_message_preview = excluded.last_message_preview,
                last_message_from = excluded.last_message_from,
                message_count = excluded.message_count,
                unread_count = excluded.unread_count,
                is_outgoing = excluded.is_outgoing,
                updated_at = datetime('now')",
            params![
                conv.account_id,
                conv.participant_key,
                conv.participants,
                conv.last_message_date.map(|dt| dt.to_rfc3339()),
                conv.last_message_preview,
                conv.last_message_from,
                conv.message_count,
                conv.unread_count,
                conv.is_outgoing as i32,
            ],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let id = conn
            .query_row(
                "SELECT id FROM conversations WHERE account_id = ?1 AND participant_key = ?2",
                params![conv.account_id, conv.participant_key],
                |row| row.get(0),
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(id)
    }

    /// Get conversation by participant key
    pub fn get_conversation_by_key(
        &self,
        account_id: &str,
        participant_key: &str,
    ) -> Result<Option<CachedConversation>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, participant_key, participants, last_message_date, last_message_preview,
                    last_message_from, message_count, unread_count, is_outgoing, classification, canonical_subject, created_at, updated_at
             FROM conversations WHERE account_id = ?1 AND participant_key = ?2"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(
                params![account_id, participant_key],
                Self::row_to_conversation,
            )
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Get all conversations for an account, sorted by last message date
    ///
    /// Filter options:
    /// - Some("chat"): Only show conversations classified as 'chat' (Connections tab)
    /// - None: Show all conversations regardless of classification (All tab)
    pub fn get_conversations(
        &self,
        account_id: &str,
        classification_filter: Option<&str>,
    ) -> Result<Vec<CachedConversation>, EddieError> {
        self.get_conversations_with_connection_filter(account_id, classification_filter, None)
    }

    /// Get conversations with optional classification and connection filtering
    ///
    /// connection_filter options:
    /// - None: No connection filtering (all conversations)
    /// - Some("connections"): Only conversations where at least one participant is a connection
    /// - Some("others"): Only conversations where NO participants are connections
    pub fn get_conversations_with_connection_filter(
        &self,
        account_id: &str,
        classification_filter: Option<&str>,
        connection_filter: Option<&str>,
    ) -> Result<Vec<CachedConversation>, EddieError> {
        let conn = self.connection()?;

        // Build the WHERE clause based on filters
        let mut where_clauses = vec!["account_id = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(account_id.to_string())];

        if let Some(classification) = classification_filter {
            where_clauses.push("classification = ?".to_string());
            params.push(Box::new(classification.to_string()));
        }

        // Add connection filter if specified
        match connection_filter {
            Some("connections") => {
                // Only conversations where at least one participant is a connection
                where_clauses.push(
                    "EXISTS (
                        SELECT 1 FROM entities e
                        WHERE e.account_id = conversations.account_id
                        AND e.is_connection = 1
                        AND (',' || conversations.participant_key || ',') LIKE ('%,' || e.email || ',%')
                    )".to_string()
                );
            }
            Some("others") => {
                // Only conversations where NO participants are connections
                where_clauses.push(
                    "NOT EXISTS (
                        SELECT 1 FROM entities e
                        WHERE e.account_id = conversations.account_id
                        AND e.is_connection = 1
                        AND (',' || conversations.participant_key || ',') LIKE ('%,' || e.email || ',%')
                    )".to_string()
                );
            }
            _ => {} // No connection filter
        }

        let where_clause = where_clauses.join(" AND ");
        let sql = format!(
            "SELECT id, account_id, participant_key, participants, last_message_date, last_message_preview,
                    last_message_from, message_count, unread_count, is_outgoing, classification, canonical_subject, created_at, updated_at
             FROM conversations
             WHERE {}
             ORDER BY last_message_date DESC",
            where_clause
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Convert params to references
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let conversations = stmt
            .query_map(param_refs.as_slice(), Self::row_to_conversation)
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(conversations)
    }

    /// Link a message to a conversation
    pub fn link_message_to_conversation(
        &self,
        conversation_id: i64,
        message_id: i64,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT OR IGNORE INTO conversation_messages (conversation_id, message_id) VALUES (?1, ?2)",
            params![conversation_id, message_id],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get messages for a conversation
    pub fn get_conversation_messages(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<CachedChatMessage>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.account_id, m.folder_name, m.uid, m.message_id, m.in_reply_to, m.references_header,
                    m.from_address, m.from_name, m.to_addresses, m.cc_addresses, m.subject, m.date, m.flags,
                    m.has_attachment, m.body_cached, m.text_body, m.html_body, m.raw_size, m.created_at, m.updated_at
             FROM messages m
             JOIN conversation_messages cm ON cm.message_id = m.id
             WHERE cm.conversation_id = ?1
             ORDER BY m.date ASC"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages = stmt
            .query_map(params![conversation_id], Self::row_to_cached_message)
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    /// Insert a new conversation if it doesn't exist (atomic, no-op if exists)
    /// Returns the conversation ID
    pub fn insert_conversation_if_not_exists(
        &self,
        conv: &CachedConversation,
    ) -> Result<i64, EddieError> {
        let conn = self.connection()?;

        // INSERT OR IGNORE - creates if not exists, does nothing if exists
        conn.execute(
            "INSERT OR IGNORE INTO conversations
             (account_id, participant_key, participants, last_message_date, last_message_preview,
              last_message_from, message_count, unread_count, is_outgoing, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, datetime('now'), datetime('now'))",
            params![
                conv.account_id,
                conv.participant_key,
                conv.participants,
                conv.last_message_date.map(|dt| dt.to_rfc3339()),
                conv.last_message_preview,
                conv.last_message_from,
                conv.is_outgoing as i32,
            ],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Get the ID (works whether we just inserted or it already existed)
        let id = conn
            .query_row(
                "SELECT id FROM conversations WHERE account_id = ?1 AND participant_key = ?2",
                params![conv.account_id, conv.participant_key],
                |row| row.get(0),
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(id)
    }

    /// Atomically increment conversation counters using SQL
    /// This avoids read-modify-write race conditions
    pub fn increment_conversation_counters(
        &self,
        conversation_id: i64,
        increment_unread: bool,
        new_last_message_date: Option<DateTime<Utc>>,
        new_last_message_preview: Option<&str>,
        new_last_message_from: Option<&str>,
        new_is_outgoing: bool,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;

        // Atomically increment counters and conditionally update last message info
        // The last_message fields are only updated if the new date is >= current date
        conn.execute(
            "UPDATE conversations SET
                message_count = message_count + 1,
                unread_count = unread_count + CASE WHEN ?1 THEN 1 ELSE 0 END,
                last_message_date = CASE
                    WHEN ?2 IS NOT NULL AND (?2 >= COALESCE(last_message_date, '') OR last_message_date IS NULL)
                    THEN ?2
                    ELSE last_message_date
                END,
                last_message_preview = CASE
                    WHEN ?2 IS NOT NULL AND (?2 >= COALESCE(last_message_date, '') OR last_message_date IS NULL)
                    THEN ?3
                    ELSE last_message_preview
                END,
                last_message_from = CASE
                    WHEN ?2 IS NOT NULL AND (?2 >= COALESCE(last_message_date, '') OR last_message_date IS NULL)
                    THEN ?4
                    ELSE last_message_from
                END,
                is_outgoing = CASE
                    WHEN ?2 IS NOT NULL AND (?2 >= COALESCE(last_message_date, '') OR last_message_date IS NULL)
                    THEN ?5
                    ELSE is_outgoing
                END,
                updated_at = datetime('now')
             WHERE id = ?6",
            params![
                increment_unread as i32,
                new_last_message_date.map(|dt| dt.to_rfc3339()),
                new_last_message_preview,
                new_last_message_from,
                new_is_outgoing as i32,
                conversation_id,
            ],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Atomically adjust unread count (for flag changes)
    pub fn adjust_conversation_unread_count(
        &self,
        conversation_id: i64,
        delta: i32,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;

        // Use MAX(0, ...) to prevent negative unread counts
        conn.execute(
            "UPDATE conversations SET
                unread_count = MAX(0, CAST(unread_count AS INTEGER) + ?1),
                updated_at = datetime('now')
             WHERE id = ?2",
            params![delta, conversation_id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Delete empty conversations
    pub fn delete_empty_conversations(&self, account_id: &str) -> Result<u64, EddieError> {
        let conn = self.connection()?;
        let deleted = conn
            .execute(
                "DELETE FROM conversations WHERE account_id = ?1 AND id NOT IN (
                SELECT DISTINCT conversation_id FROM conversation_messages
            )",
                params![account_id],
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(deleted as u64)
    }

    /// Get canonical subject for a conversation
    pub fn get_conversation_canonical_subject(
        &self,
        conversation_id: i64,
    ) -> Result<Option<String>, EddieError> {
        let conn = self.connection()?;
        let result = conn
            .query_row(
                "SELECT canonical_subject FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Set canonical subject for a conversation (if not already set)
    /// Only sets the subject if canonical_subject is currently NULL
    /// This ensures we don't overwrite an existing meaningful subject
    pub fn set_conversation_canonical_subject(
        &self,
        conversation_id: i64,
        subject: &str,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE conversations SET canonical_subject = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND canonical_subject IS NULL",
            params![subject, conversation_id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    fn row_to_conversation(row: &Row) -> Result<CachedConversation, rusqlite::Error> {
        Ok(CachedConversation {
            id: row.get(0)?,
            account_id: row.get(1)?,
            participant_key: row.get(2)?,
            participants: row.get(3)?,
            last_message_date: row
                .get::<_, Option<String>>(4)?
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            last_message_preview: row.get(5)?,
            last_message_from: row.get(6)?,
            message_count: row.get(7)?,
            unread_count: row.get(8)?,
            is_outgoing: row.get::<_, i32>(9)? != 0,
            classification: row.get(10)?,
            canonical_subject: row.get(11)?,
            created_at: row
                .get::<_, String>(12)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: row
                .get::<_, String>(13)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        })
    }

    // ========== Action Queue Operations ==========

    /// Queue an action for later execution
    pub fn queue_action(&self, action: &QueuedActionRecord) -> Result<i64, EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO action_queue (account_id, action_type, folder_name, uid, payload, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                action.account_id,
                action.action_type,
                action.folder_name,
                action.uid,
                action.payload,
                action.status,
            ],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(conn.last_insert_rowid())
    }

    /// Get pending actions in order
    pub fn get_pending_actions(
        &self,
        account_id: &str,
    ) -> Result<Vec<QueuedActionRecord>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, action_type, folder_name, uid, payload, created_at, retry_count, last_error, status
             FROM action_queue WHERE account_id = ?1 AND status = 'pending'
             ORDER BY created_at ASC"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let actions = stmt
            .query_map(params![account_id], Self::row_to_action)
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(actions)
    }

    /// Update action status (increments retry_count)
    pub fn update_action_status(
        &self,
        id: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE action_queue SET status = ?1, last_error = ?2, retry_count = retry_count + 1
             WHERE id = ?3",
            params![status, error, id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Update action status without incrementing retry_count (for marking as processing)
    pub fn update_action_status_no_retry_increment(
        &self,
        id: i64,
        status: &str,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE action_queue SET status = ?1 WHERE id = ?2",
            params![status, id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Delete completed actions
    pub fn delete_completed_actions(&self, account_id: &str) -> Result<u64, EddieError> {
        let conn = self.connection()?;
        let deleted = conn
            .execute(
                "DELETE FROM action_queue WHERE account_id = ?1 AND status = 'completed'",
                params![account_id],
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(deleted as u64)
    }

    fn row_to_action(row: &Row) -> Result<QueuedActionRecord, rusqlite::Error> {
        Ok(QueuedActionRecord {
            id: row.get(0)?,
            account_id: row.get(1)?,
            action_type: row.get(2)?,
            folder_name: row.get(3)?,
            uid: row.get(4)?,
            payload: row.get(5)?,
            created_at: row
                .get::<_, String>(6)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            retry_count: row.get(7)?,
            last_error: row.get(8)?,
            status: row.get(9)?,
        })
    }

    // ========== Message Classification Operations ==========

    /// Set classification for a message
    pub fn set_message_classification(
        &self,
        classification: &MessageClassification,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO message_classifications (message_id, classification, confidence, is_hidden_from_chat, classified_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(message_id) DO UPDATE SET
                classification = excluded.classification,
                confidence = excluded.confidence,
                is_hidden_from_chat = excluded.is_hidden_from_chat,
                classified_at = excluded.classified_at",
            params![
                classification.message_id,
                classification.classification,
                classification.confidence,
                classification.is_hidden_from_chat as i32,
                classification.classified_at.to_rfc3339(),
            ],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        // If this is a 'chat' classification, update the conversation classification
        if classification.classification == "chat" {
            conn.execute(
                r#"
                UPDATE conversations
                SET classification = 'chat',
                    updated_at = datetime('now')
                WHERE id IN (
                    SELECT conversation_id
                    FROM conversation_messages
                    WHERE message_id = ?1
                )
                AND classification IS NULL
                "#,
                params![classification.message_id],
            ).map_err(|e| EddieError::Backend(e.to_string()))?;
        }

        Ok(())
    }

    /// Update conversation classifications based on existing message classifications
    /// This is called after rebuilding conversations to ensure they have the correct classification
    pub fn update_conversation_classifications(
        &self,
        account_id: &str,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;

        // Update conversations to 'chat' if any of their messages are classified as 'chat'
        conn.execute(
            r#"
            UPDATE conversations
            SET classification = 'chat',
                updated_at = datetime('now')
            WHERE account_id = ?1
              AND classification IS NULL
              AND id IN (
                SELECT DISTINCT cm.conversation_id
                FROM conversation_messages cm
                JOIN message_classifications mc ON cm.message_id = mc.message_id
                WHERE mc.classification = 'chat'
              )
            "#,
            params![account_id],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get classification for a message
    pub fn get_message_classification(
        &self,
        message_id: i64,
    ) -> Result<Option<MessageClassification>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT message_id, classification, confidence, is_hidden_from_chat, classified_at
             FROM message_classifications WHERE message_id = ?1",
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![message_id], |row| {
                Ok(MessageClassification {
                    message_id: row.get(0)?,
                    classification: row.get(1)?,
                    confidence: row.get(2)?,
                    is_hidden_from_chat: row.get::<_, i32>(3)? != 0,
                    classified_at: row
                        .get::<_, String>(4)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            })
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    // ========== Sync Progress Operations ==========

    /// Update sync progress
    pub fn update_sync_progress(&self, progress: &SyncProgress) -> Result<(), EddieError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO sync_progress (account_id, folder_name, phase, total_messages, synced_messages,
                oldest_synced_date, last_batch_uid, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(account_id, folder_name) DO UPDATE SET
                phase = excluded.phase,
                total_messages = excluded.total_messages,
                synced_messages = excluded.synced_messages,
                oldest_synced_date = excluded.oldest_synced_date,
                last_batch_uid = excluded.last_batch_uid,
                updated_at = datetime('now')",
            params![
                progress.account_id,
                progress.folder_name,
                progress.phase,
                progress.total_messages,
                progress.synced_messages,
                progress.oldest_synced_date.map(|dt| dt.to_rfc3339()),
                progress.last_batch_uid,
            ],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get sync progress
    pub fn get_sync_progress(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<Option<SyncProgress>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT account_id, folder_name, phase, total_messages, synced_messages,
                    oldest_synced_date, last_batch_uid, started_at, updated_at
             FROM sync_progress WHERE account_id = ?1 AND folder_name = ?2",
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![account_id, folder_name], |row| {
                Ok(SyncProgress {
                    account_id: row.get(0)?,
                    folder_name: row.get(1)?,
                    phase: row.get(2)?,
                    total_messages: row.get(3)?,
                    synced_messages: row.get(4)?,
                    oldest_synced_date: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    last_batch_uid: row.get(6)?,
                    started_at: row
                        .get::<_, String>(7)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    updated_at: row
                        .get::<_, String>(8)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            })
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    // ========== Server Capabilities Operations ==========

    /// Store server capabilities
    pub fn store_capabilities(
        &self,
        account_id: &str,
        capabilities: &[String],
        supports_qresync: bool,
        supports_condstore: bool,
        supports_idle: bool,
    ) -> Result<(), EddieError> {
        let conn = self.connection()?;
        let capabilities_json = serde_json::to_string(capabilities)
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        conn.execute(
            "INSERT INTO server_capabilities (account_id, capabilities, supports_qresync, supports_condstore, supports_idle, detected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
             ON CONFLICT(account_id) DO UPDATE SET
                capabilities = excluded.capabilities,
                supports_qresync = excluded.supports_qresync,
                supports_condstore = excluded.supports_condstore,
                supports_idle = excluded.supports_idle,
                detected_at = datetime('now')",
            params![
                account_id,
                capabilities_json,
                supports_qresync as i32,
                supports_condstore as i32,
                supports_idle as i32,
            ],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get cached server capabilities
    pub fn get_capabilities(
        &self,
        account_id: &str,
    ) -> Result<Option<(Vec<String>, bool, bool, bool)>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT capabilities, supports_qresync, supports_condstore, supports_idle
             FROM server_capabilities WHERE account_id = ?1",
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![account_id], |row| {
                let caps_json: String = row.get(0)?;
                let caps: Vec<String> = serde_json::from_str(&caps_json).unwrap_or_default();
                Ok((
                    caps,
                    row.get::<_, i32>(1)? != 0,
                    row.get::<_, i32>(2)? != 0,
                    row.get::<_, i32>(3)? != 0,
                ))
            })
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }

    // ========== Entity Operations ==========

    /// Upsert an entity (participant) - updates if exists, creates if not
    /// If is_connection is true, it will be set to true (never reverted to false)
    pub fn upsert_entity(
        &self,
        account_id: &str,
        email: &str,
        name: Option<&str>,
        is_connection: bool,
        contact_timestamp: DateTime<Utc>,
    ) -> Result<i64, EddieError> {
        let conn = self.connection()?;

        // Use INSERT OR REPLACE pattern with special handling for is_connection
        // is_connection should only be upgraded to true, never downgraded to false
        conn.execute(
            "INSERT INTO entities (account_id, email, name, is_connection, latest_contact, contact_count)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT(account_id, email) DO UPDATE SET
                name = COALESCE(excluded.name, entities.name),
                is_connection = CASE WHEN excluded.is_connection = 1 THEN 1 ELSE entities.is_connection END,
                latest_contact = CASE WHEN excluded.latest_contact > entities.latest_contact THEN excluded.latest_contact ELSE entities.latest_contact END,
                contact_count = entities.contact_count + 1,
                updated_at = datetime('now')",
            params![
                account_id,
                email.to_lowercase(),
                name,
                is_connection as i32,
                contact_timestamp.to_rfc3339(),
            ],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Get the ID
        let id = conn
            .query_row(
                "SELECT id FROM entities WHERE account_id = ?1 AND email = ?2",
                params![account_id, email.to_lowercase()],
                |row| row.get(0),
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(id)
    }

    /// Search entities for autocomplete
    /// Prioritizes: 1) connections, 2) recent contacts
    /// Returns up to `limit` results matching the query
    pub fn search_entities(
        &self,
        account_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<Entity>, EddieError> {
        let conn = self.connection()?;

        // Search by email or name prefix, prioritizing connections and recent contacts
        let search_pattern = format!("{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, account_id, email, name, is_connection, latest_contact, contact_count
             FROM entities
             WHERE account_id = ?1 AND (email LIKE ?2 OR LOWER(name) LIKE ?2)
             ORDER BY is_connection DESC, latest_contact DESC
             LIMIT ?3"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let entities = stmt
            .query_map(params![account_id, search_pattern, limit], |row| {
                Ok(Entity {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    email: row.get(2)?,
                    name: row.get(3)?,
                    is_connection: row.get::<_, i32>(4)? != 0,
                    latest_contact: row
                        .get::<_, String>(5)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    contact_count: row.get(6)?,
                })
            })
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entities)
    }

    /// Get an entity by email
    pub fn get_entity(
        &self,
        account_id: &str,
        email: &str,
    ) -> Result<Option<Entity>, EddieError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, email, name, is_connection, latest_contact, contact_count
             FROM entities WHERE account_id = ?1 AND email = ?2"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![account_id, email.to_lowercase()], |row| {
                Ok(Entity {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    email: row.get(2)?,
                    name: row.get(3)?,
                    is_connection: row.get::<_, i32>(4)? != 0,
                    latest_contact: row
                        .get::<_, String>(5)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    contact_count: row.get(6)?,
                })
            })
            .optional()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation() {
        let db = SyncDatabase::in_memory().expect("Failed to create in-memory database");
        let conn = db.connection().expect("Failed to get connection");

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"folder_sync_state".to_string()));
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"conversations".to_string()));
        assert!(tables.contains(&"action_queue".to_string()));
    }

    #[test]
    fn test_folder_sync_state() {
        let db = SyncDatabase::in_memory().expect("Failed to create database");

        let state = FolderSyncState {
            account_id: "test@example.com".to_string(),
            folder_name: "INBOX".to_string(),
            uidvalidity: Some(12345),
            highestmodseq: Some(67890),
            last_seen_uid: Some(100),
            last_sync_timestamp: Some(Utc::now()),
            sync_in_progress: false,
        };

        db.upsert_folder_sync_state(&state)
            .expect("Failed to upsert");

        let retrieved = db
            .get_folder_sync_state("test@example.com", "INBOX")
            .expect("Failed to get")
            .expect("State not found");

        assert_eq!(retrieved.uidvalidity, Some(12345));
        assert_eq!(retrieved.highestmodseq, Some(67890));
    }

    #[test]
    fn test_uidvalidity_operations() {
        let db = SyncDatabase::in_memory().expect("Failed to create database");

        // Initially, no UIDVALIDITY stored
        let uidvalidity = db
            .get_folder_uidvalidity("test@example.com", "INBOX")
            .expect("Failed to get uidvalidity");
        assert_eq!(uidvalidity, None);

        // Set UIDVALIDITY
        db.set_folder_uidvalidity("test@example.com", "INBOX", 12345)
            .expect("Failed to set uidvalidity");

        // Verify it was stored
        let uidvalidity = db
            .get_folder_uidvalidity("test@example.com", "INBOX")
            .expect("Failed to get uidvalidity");
        assert_eq!(uidvalidity, Some(12345));

        // Update UIDVALIDITY
        db.set_folder_uidvalidity("test@example.com", "INBOX", 67890)
            .expect("Failed to update uidvalidity");

        // Verify update
        let uidvalidity = db
            .get_folder_uidvalidity("test@example.com", "INBOX")
            .expect("Failed to get uidvalidity");
        assert_eq!(uidvalidity, Some(67890));
    }

    #[test]
    fn test_invalidate_folder_cache() {
        let db = SyncDatabase::in_memory().expect("Failed to create database");

        // Set up UIDVALIDITY
        db.set_folder_uidvalidity("test@example.com", "INBOX", 12345)
            .expect("Failed to set uidvalidity");

        // Verify it exists
        let uidvalidity = db
            .get_folder_uidvalidity("test@example.com", "INBOX")
            .expect("Failed to get uidvalidity");
        assert_eq!(uidvalidity, Some(12345));

        // Invalidate the cache
        db.invalidate_folder_cache("test@example.com", "INBOX")
            .expect("Failed to invalidate cache");

        // UIDVALIDITY should be cleared (folder_sync_state deleted)
        let uidvalidity = db
            .get_folder_uidvalidity("test@example.com", "INBOX")
            .expect("Failed to get uidvalidity");
        assert_eq!(uidvalidity, None);
    }
}
