//! SQLite database for IMAP sync cache
//!
//! This module provides all database operations for the sync engine.
//! The database is a cache of server state - all data can be rebuilt from IMAP.

use chrono::{DateTime, Utc};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::types::error::HimalayaError;

/// Database connection pool type
pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConnection = PooledConnection<SqliteConnectionManager>;

/// Sync state for a folder
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct CachedMessage {
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

/// SQLite database for sync cache
pub struct SyncDatabase {
    pool: DbPool,
}

impl SyncDatabase {
    /// Create a new database at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, HimalayaError> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder().max_size(10).build(manager).map_err(|e| {
            HimalayaError::Backend(format!("Failed to create database pool: {}", e))
        })?;

        let db = Self { pool };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self, HimalayaError> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).map_err(|e| {
            HimalayaError::Backend(format!("Failed to create database pool: {}", e))
        })?;

        let db = Self { pool };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Get a connection from the pool
    pub fn connection(&self) -> Result<DbConnection, HimalayaError> {
        self.pool.get().map_err(|e| {
            HimalayaError::Backend(format!("Failed to get database connection: {}", e))
        })
    }

    /// Initialize the database schema
    fn initialize_schema(&self) -> Result<(), HimalayaError> {
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
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(account_id, participant_key)
            );

            -- Index for conversation lookups
            CREATE INDEX IF NOT EXISTS idx_conversations_account ON conversations(account_id);
            CREATE INDEX IF NOT EXISTS idx_conversations_last_date ON conversations(last_message_date DESC);
            CREATE INDEX IF NOT EXISTS idx_conversations_participant_key ON conversations(participant_key);

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
        "#).map_err(|e| HimalayaError::Backend(format!("Failed to initialize schema: {}", e)))?;

        Ok(())
    }

    // ========== Folder Sync State Operations ==========

    /// Get sync state for a folder
    pub fn get_folder_sync_state(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<Option<FolderSyncState>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT account_id, folder_name, uidvalidity, highestmodseq, last_seen_uid, last_sync_timestamp, sync_in_progress
             FROM folder_sync_state WHERE account_id = ?1 AND folder_name = ?2"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Update or insert folder sync state
    pub fn upsert_folder_sync_state(&self, state: &FolderSyncState) -> Result<(), HimalayaError> {
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
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get the stored UIDVALIDITY for a folder
    pub fn get_folder_uidvalidity(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<Option<u32>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT uidvalidity FROM folder_sync_state WHERE account_id = ?1 AND folder_name = ?2",
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![account_id, folder_name], |row| row.get(0))
            .optional()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Set/update the UIDVALIDITY for a folder
    pub fn set_folder_uidvalidity(
        &self,
        account_id: &str,
        folder_name: &str,
        uidvalidity: u32,
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO folder_sync_state (account_id, folder_name, uidvalidity)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(account_id, folder_name) DO UPDATE SET
                uidvalidity = excluded.uidvalidity",
            params![account_id, folder_name, uidvalidity],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Invalidate folder cache (when UIDVALIDITY changes)
    pub fn invalidate_folder_cache(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<(), HimalayaError> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Delete all messages in this folder
        tx.execute(
            "DELETE FROM messages WHERE account_id = ?1 AND folder_name = ?2",
            params![account_id, folder_name],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Reset sync state
        tx.execute(
            "DELETE FROM folder_sync_state WHERE account_id = ?1 AND folder_name = ?2",
            params![account_id, folder_name],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Reset sync progress
        tx.execute(
            "DELETE FROM sync_progress WHERE account_id = ?1 AND folder_name = ?2",
            params![account_id, folder_name],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        tx.commit()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    // ========== Message Operations ==========

    /// Insert or update a message
    pub fn upsert_message(&self, msg: &CachedMessage) -> Result<i64, HimalayaError> {
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
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Get the row ID
        let id = conn
            .query_row(
                "SELECT id FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3",
                params![msg.account_id, msg.folder_name, msg.uid],
                |row| row.get(0),
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(id)
    }

    /// Get a message by account/folder/uid
    pub fn get_message(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
    ) -> Result<Option<CachedMessage>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(
                params![account_id, folder_name, uid],
                Self::row_to_cached_message,
            )
            .optional()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Get a message by its internal ID
    pub fn get_message_by_id(&self, id: i64) -> Result<Option<CachedMessage>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE id = ?1"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(params![id], Self::row_to_cached_message)
            .optional()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Update flags for a message (replaces all flags)
    pub fn update_message_flags(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags: &str,
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE messages SET flags = ?1, updated_at = datetime('now')
             WHERE account_id = ?2 AND folder_name = ?3 AND uid = ?4",
            params![flags, account_id, folder_name, uid],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Add flags to a message (preserves existing flags)
    pub fn add_message_flags(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags_to_add: &[String],
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;

        // Get current flags
        let current_flags: Option<String> = conn
            .query_row(
                "SELECT flags FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3",
                params![account_id, folder_name, uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
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
            serde_json::to_string(&flags).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        conn.execute(
            "UPDATE messages SET flags = ?1, updated_at = datetime('now')
             WHERE account_id = ?2 AND folder_name = ?3 AND uid = ?4",
            params![flags_json, account_id, folder_name, uid],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Remove flags from a message
    pub fn remove_message_flags(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags_to_remove: &[String],
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;

        // Get current flags
        let current_flags: Option<String> = conn
            .query_row(
                "SELECT flags FROM messages WHERE account_id = ?1 AND folder_name = ?2 AND uid = ?3",
                params![account_id, folder_name, uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
            .flatten();

        let mut flags: Vec<String> = current_flags
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        // Remove specified flags
        flags.retain(|f| !flags_to_remove.contains(f));

        let flags_json =
            serde_json::to_string(&flags).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        conn.execute(
            "UPDATE messages SET flags = ?1, updated_at = datetime('now')
             WHERE account_id = ?2 AND folder_name = ?3 AND uid = ?4",
            params![flags_json, account_id, folder_name, uid],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Set flags on a message (replaces existing flags with new set)
    pub fn set_message_flags_vec(
        &self,
        account_id: &str,
        folder_name: &str,
        uid: u32,
        flags: &[String],
    ) -> Result<(), HimalayaError> {
        let flags_json =
            serde_json::to_string(flags).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        self.update_message_flags(account_id, folder_name, uid, &flags_json)
    }

    /// Delete messages by UIDs
    pub fn delete_messages_by_uids(
        &self,
        account_id: &str,
        folder_name: &str,
        uids: &[u32],
    ) -> Result<(), HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get all UIDs in a folder (for deletion detection)
    pub fn get_folder_uids(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<HashSet<u32>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare("SELECT uid FROM messages WHERE account_id = ?1 AND folder_name = ?2")
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let uids = stmt
            .query_map(params![account_id, folder_name], |row| row.get(0))
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
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
    ) -> Result<Vec<CachedMessage>, HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    fn row_to_cached_message(row: &Row) -> Result<CachedMessage, rusqlite::Error> {
        Ok(CachedMessage {
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
    pub fn upsert_conversation(&self, conv: &CachedConversation) -> Result<i64, HimalayaError> {
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
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let id = conn
            .query_row(
                "SELECT id FROM conversations WHERE account_id = ?1 AND participant_key = ?2",
                params![conv.account_id, conv.participant_key],
                |row| row.get(0),
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(id)
    }

    /// Get conversation by participant key
    pub fn get_conversation_by_key(
        &self,
        account_id: &str,
        participant_key: &str,
    ) -> Result<Option<CachedConversation>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, participant_key, participants, last_message_date, last_message_preview,
                    last_message_from, message_count, unread_count, is_outgoing, created_at, updated_at
             FROM conversations WHERE account_id = ?1 AND participant_key = ?2"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let result = stmt
            .query_row(
                params![account_id, participant_key],
                Self::row_to_conversation,
            )
            .optional()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(result)
    }

    /// Get all conversations for an account, sorted by last message date
    pub fn get_conversations(
        &self,
        account_id: &str,
        include_hidden: bool,
    ) -> Result<Vec<CachedConversation>, HimalayaError> {
        let conn = self.connection()?;

        let sql = if include_hidden {
            "SELECT id, account_id, participant_key, participants, last_message_date, last_message_preview,
                    last_message_from, message_count, unread_count, is_outgoing, created_at, updated_at
             FROM conversations WHERE account_id = ?1
             ORDER BY last_message_date DESC"
        } else {
            // Exclude conversations where all messages are hidden (non-chat)
            "SELECT c.id, c.account_id, c.participant_key, c.participants, c.last_message_date, c.last_message_preview,
                    c.last_message_from, c.message_count, c.unread_count, c.is_outgoing, c.created_at, c.updated_at
             FROM conversations c
             WHERE c.account_id = ?1
               AND EXISTS (
                   SELECT 1 FROM conversation_messages cm
                   JOIN messages m ON m.id = cm.message_id
                   LEFT JOIN message_classifications mc ON mc.message_id = m.id
                   WHERE cm.conversation_id = c.id AND (mc.is_hidden_from_chat IS NULL OR mc.is_hidden_from_chat = 0)
               )
             ORDER BY c.last_message_date DESC"
        };

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let conversations = stmt
            .query_map(params![account_id], Self::row_to_conversation)
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(conversations)
    }

    /// Link a message to a conversation
    pub fn link_message_to_conversation(
        &self,
        conversation_id: i64,
        message_id: i64,
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT OR IGNORE INTO conversation_messages (conversation_id, message_id) VALUES (?1, ?2)",
            params![conversation_id, message_id],
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get messages for a conversation
    pub fn get_conversation_messages(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<CachedMessage>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.account_id, m.folder_name, m.uid, m.message_id, m.in_reply_to, m.references_header,
                    m.from_address, m.from_name, m.to_addresses, m.cc_addresses, m.subject, m.date, m.flags,
                    m.has_attachment, m.body_cached, m.text_body, m.html_body, m.raw_size, m.created_at, m.updated_at
             FROM messages m
             JOIN conversation_messages cm ON cm.message_id = m.id
             WHERE cm.conversation_id = ?1
             ORDER BY m.date ASC"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let messages = stmt
            .query_map(params![conversation_id], Self::row_to_cached_message)
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    /// Insert a new conversation if it doesn't exist (atomic, no-op if exists)
    /// Returns the conversation ID
    pub fn insert_conversation_if_not_exists(
        &self,
        conv: &CachedConversation,
    ) -> Result<i64, HimalayaError> {
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
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Get the ID (works whether we just inserted or it already existed)
        let id = conn
            .query_row(
                "SELECT id FROM conversations WHERE account_id = ?1 AND participant_key = ?2",
                params![conv.account_id, conv.participant_key],
                |row| row.get(0),
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
    ) -> Result<(), HimalayaError> {
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
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Atomically adjust unread count (for flag changes)
    pub fn adjust_conversation_unread_count(
        &self,
        conversation_id: i64,
        delta: i32,
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;

        // Use MAX(0, ...) to prevent negative unread counts
        conn.execute(
            "UPDATE conversations SET
                unread_count = MAX(0, CAST(unread_count AS INTEGER) + ?1),
                updated_at = datetime('now')
             WHERE id = ?2",
            params![delta, conversation_id],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Delete empty conversations
    pub fn delete_empty_conversations(&self, account_id: &str) -> Result<u64, HimalayaError> {
        let conn = self.connection()?;
        let deleted = conn
            .execute(
                "DELETE FROM conversations WHERE account_id = ?1 AND id NOT IN (
                SELECT DISTINCT conversation_id FROM conversation_messages
            )",
                params![account_id],
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(deleted as u64)
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
            created_at: row
                .get::<_, String>(10)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: row
                .get::<_, String>(11)
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        })
    }

    // ========== Action Queue Operations ==========

    /// Queue an action for later execution
    pub fn queue_action(&self, action: &QueuedActionRecord) -> Result<i64, HimalayaError> {
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
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(conn.last_insert_rowid())
    }

    /// Get pending actions in order
    pub fn get_pending_actions(
        &self,
        account_id: &str,
    ) -> Result<Vec<QueuedActionRecord>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, action_type, folder_name, uid, payload, created_at, retry_count, last_error, status
             FROM action_queue WHERE account_id = ?1 AND status = 'pending'
             ORDER BY created_at ASC"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let actions = stmt
            .query_map(params![account_id], Self::row_to_action)
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
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
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE action_queue SET status = ?1, last_error = ?2, retry_count = retry_count + 1
             WHERE id = ?3",
            params![status, error, id],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Update action status without incrementing retry_count (for marking as processing)
    pub fn update_action_status_no_retry_increment(
        &self,
        id: i64,
        status: &str,
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE action_queue SET status = ?1 WHERE id = ?2",
            params![status, id],
        )
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Delete completed actions
    pub fn delete_completed_actions(&self, account_id: &str) -> Result<u64, HimalayaError> {
        let conn = self.connection()?;
        let deleted = conn
            .execute(
                "DELETE FROM action_queue WHERE account_id = ?1 AND status = 'completed'",
                params![account_id],
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
    ) -> Result<(), HimalayaError> {
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
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get classification for a message
    pub fn get_message_classification(
        &self,
        message_id: i64,
    ) -> Result<Option<MessageClassification>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT message_id, classification, confidence, is_hidden_from_chat, classified_at
             FROM message_classifications WHERE message_id = ?1",
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(result)
    }

    // ========== Sync Progress Operations ==========

    /// Update sync progress
    pub fn update_sync_progress(&self, progress: &SyncProgress) -> Result<(), HimalayaError> {
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
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get sync progress
    pub fn get_sync_progress(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<Option<SyncProgress>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT account_id, folder_name, phase, total_messages, synced_messages,
                    oldest_synced_date, last_batch_uid, started_at, updated_at
             FROM sync_progress WHERE account_id = ?1 AND folder_name = ?2",
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
    ) -> Result<(), HimalayaError> {
        let conn = self.connection()?;
        let capabilities_json = serde_json::to_string(capabilities)
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(())
    }

    /// Get cached server capabilities
    pub fn get_capabilities(
        &self,
        account_id: &str,
    ) -> Result<Option<(Vec<String>, bool, bool, bool)>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT capabilities, supports_qresync, supports_condstore, supports_idle
             FROM server_capabilities WHERE account_id = ?1",
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
    ) -> Result<i64, HimalayaError> {
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
        .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Get the ID
        let id = conn
            .query_row(
                "SELECT id FROM entities WHERE account_id = ?1 AND email = ?2",
                params![account_id, email.to_lowercase()],
                |row| row.get(0),
            )
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
    ) -> Result<Vec<Entity>, HimalayaError> {
        let conn = self.connection()?;

        // Search by email or name prefix, prioritizing connections and recent contacts
        let search_pattern = format!("{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, account_id, email, name, is_connection, latest_contact, contact_count
             FROM entities
             WHERE account_id = ?1 AND (email LIKE ?2 OR LOWER(name) LIKE ?2)
             ORDER BY is_connection DESC, latest_contact DESC
             LIMIT ?3"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entities)
    }

    /// Get an entity by email
    pub fn get_entity(
        &self,
        account_id: &str,
        email: &str,
    ) -> Result<Option<Entity>, HimalayaError> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, email, name, is_connection, latest_contact, contact_count
             FROM entities WHERE account_id = ?1 AND email = ?2"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
