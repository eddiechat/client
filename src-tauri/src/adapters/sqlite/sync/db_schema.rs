use rusqlite::Connection;

use crate::error::EddieError;

const SCHEMA_VERSION: &str = "2";

pub fn initialize_schema(conn: &Connection) -> Result<(), EddieError> {
    // Ensure accounts and settings tables exist first (they survive resets).
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS accounts (
            id              TEXT PRIMARY KEY,
            email           TEXT NOT NULL UNIQUE,
            password        TEXT,
            display_name    TEXT,
            imap_host       TEXT NOT NULL,
            imap_port       INTEGER NOT NULL DEFAULT 993,
            imap_tls        INTEGER NOT NULL DEFAULT 1,
            smtp_host       TEXT NOT NULL,
            smtp_port       INTEGER NOT NULL DEFAULT 587,
            carddav_url     TEXT,
            created_at      INTEGER NOT NULL,
            last_full_sync  INTEGER
        );

        CREATE TABLE IF NOT EXISTS settings (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
    ")?;

    // Check schema version — if missing or outdated, drop everything else and rebuild.
    let current_version: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .ok();

    if current_version.as_deref() != Some(SCHEMA_VERSION) {
        drop_data_tables(conn)?;
        // Also reset onboarding so accounts re-sync from scratch.
        conn.execute_batch("UPDATE accounts SET last_full_sync = NULL;")?;
    }

    create_data_tables(conn)?;

    // Stamp the version
    conn.execute_batch(&format!(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) \
         VALUES ('schema_version', '{}', strftime('%s','now') * 1000);",
        SCHEMA_VERSION
    ))?;

    Ok(())
}

/// Drop all tables except `accounts` and `settings`.
fn drop_data_tables(conn: &Connection) -> Result<(), EddieError> {
    conn.execute_batch("
        DROP TABLE IF EXISTS messages;
        DROP TABLE IF EXISTS conversations;
        DROP TABLE IF EXISTS entities;
        DROP TABLE IF EXISTS action_queue;
        DROP TABLE IF EXISTS sync_state;
        DROP TABLE IF EXISTS folder_sync;
        DROP TABLE IF EXISTS onboarding_tasks;
    ")?;
    Ok(())
}

/// Create all data tables (idempotent via IF NOT EXISTS).
fn create_data_tables(conn: &Connection) -> Result<(), EddieError> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            account_id      TEXT NOT NULL REFERENCES accounts(id),
            message_id      TEXT NOT NULL,
            imap_uid        INTEGER NOT NULL,
            imap_folder     TEXT NOT NULL,
            date            INTEGER NOT NULL,
            from_address    TEXT NOT NULL,
            from_name       TEXT,
            to_addresses    TEXT NOT NULL,
            cc_addresses    TEXT DEFAULT '[]',
            bcc_addresses   TEXT DEFAULT '[]',
            subject         TEXT,
            body_text       TEXT,
            body_html       TEXT,
            size_bytes      INTEGER,
            has_attachments  INTEGER DEFAULT 0,

            in_reply_to     TEXT,
            references_ids  TEXT DEFAULT '[]',
            participant_changes TEXT,

            imap_flags      TEXT DEFAULT '[]',
            gmail_labels    TEXT DEFAULT '[]',
            fetched_at      INTEGER NOT NULL,

            classification  TEXT,
            classification_source TEXT,
            classification_confidence REAL,
            classification_reason TEXT,
            classification_headers TEXT DEFAULT '{}',
            is_important    INTEGER DEFAULT 0,
            distilled_text  TEXT,
            processed_at    INTEGER,

            participant_key TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            thread_id       TEXT,

            UNIQUE(account_id, imap_folder, imap_uid)
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conversation   ON messages(conversation_id, date DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_date           ON messages(account_id, date DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_classification ON messages(classification);
        CREATE INDEX IF NOT EXISTS idx_messages_from           ON messages(from_address);
        CREATE INDEX IF NOT EXISTS idx_messages_message_id     ON messages(message_id);
        CREATE INDEX IF NOT EXISTS idx_messages_thread          ON messages(account_id, thread_id);

        CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_dedup
        ON messages(account_id, message_id) WHERE message_id != '';

        CREATE TABLE IF NOT EXISTS conversations (
            id                  TEXT PRIMARY KEY,
            account_id          TEXT NOT NULL REFERENCES accounts(id),
            participant_key     TEXT NOT NULL,
            participant_names   TEXT,
            classification      TEXT NOT NULL,
            last_message_date   INTEGER NOT NULL,
            last_message_preview TEXT,
            unread_count        INTEGER DEFAULT 0,
            total_count         INTEGER DEFAULT 0,
            is_muted            INTEGER DEFAULT 0,
            is_pinned           INTEGER DEFAULT 0,
            is_important        INTEGER DEFAULT 0,
            initial_sender_email TEXT,
            updated_at          INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_conversations_class ON conversations(account_id, classification, last_message_date DESC);
        CREATE INDEX IF NOT EXISTS idx_conversations_date  ON conversations(account_id, last_message_date DESC);

        CREATE TABLE IF NOT EXISTS entities (
            id              TEXT PRIMARY KEY,
            account_id      TEXT NOT NULL REFERENCES accounts(id),
            email           TEXT NOT NULL,
            display_name    TEXT,
            trust_level     TEXT NOT NULL,
            source          TEXT,
            first_seen      INTEGER NOT NULL,
            last_seen       INTEGER,
            sent_count      INTEGER DEFAULT 0,
            metadata        TEXT DEFAULT '{}',

            UNIQUE(account_id, email)
        );

        CREATE INDEX IF NOT EXISTS idx_entities_trust ON entities(account_id, trust_level);
        CREATE INDEX IF NOT EXISTS idx_entities_email ON entities(email);

        CREATE TABLE IF NOT EXISTS action_queue (
            id              TEXT PRIMARY KEY,
            account_id      TEXT NOT NULL REFERENCES accounts(id),
            action_type     TEXT NOT NULL,
            payload         TEXT NOT NULL,
            status          TEXT NOT NULL DEFAULT 'pending',
            retry_count     INTEGER DEFAULT 0,
            max_retries     INTEGER DEFAULT 3,
            created_at      INTEGER NOT NULL,
            completed_at    INTEGER,
            error           TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_action_queue_status ON action_queue(status, created_at);

        CREATE TABLE IF NOT EXISTS sync_state (
            account_id      TEXT PRIMARY KEY REFERENCES accounts(id),
            draft_uid       INTEGER,
            draft_version   INTEGER DEFAULT 0,
            last_pushed     INTEGER,
            last_pulled     INTEGER
        );

        CREATE TABLE IF NOT EXISTS folder_sync (
            account_id    TEXT NOT NULL,
            folder        TEXT NOT NULL,
            uid_validity  INTEGER NOT NULL DEFAULT 0,
            highest_uid   INTEGER DEFAULT 0,
            lowest_uid    INTEGER DEFAULT 0,
            sync_status   TEXT DEFAULT 'pending',
            last_sync     INTEGER,
            PRIMARY KEY (account_id, folder)
        );

        CREATE TABLE IF NOT EXISTS onboarding_tasks (
            account_id  TEXT NOT NULL REFERENCES accounts(id),
            task        TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'pending',
            cursor      TEXT,
            updated_at  INTEGER,
            PRIMARY KEY (account_id, task)
        );
    ")?;

    Ok(())
}
