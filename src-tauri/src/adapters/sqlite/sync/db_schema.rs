use rusqlite::Connection;

use crate::error::EddieError;

pub fn initialize_schema(conn: &Connection) -> Result<(), EddieError> {
    conn.execute_batch("
        -- Accounts & identity
        CREATE TABLE IF NOT EXISTS accounts (
            id              TEXT PRIMARY KEY,   -- UUID
            email           TEXT NOT NULL UNIQUE,
            password        TEXT,
            display_name    TEXT,
            imap_host       TEXT NOT NULL,
            imap_port       INTEGER NOT NULL DEFAULT 993,
            smtp_host       TEXT NOT NULL,
            smtp_port       INTEGER NOT NULL DEFAULT 587,
            carddav_url     TEXT,               -- nullable, contacts are optional
            created_at      INTEGER NOT NULL,   -- unix epoch ms
            last_full_sync  INTEGER             -- unix epoch ms, NULL until onboarding completes
        );

        -- Raw message store (cache of IMAP data)
        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,   -- UUID
            account_id      TEXT NOT NULL REFERENCES accounts(id),
            message_id      TEXT NOT NULL,      -- RFC 5322 Message-ID header
            imap_uid        INTEGER NOT NULL,
            imap_folder     TEXT NOT NULL,
            date            INTEGER NOT NULL,   -- unix epoch ms
            from_address    TEXT NOT NULL,
            from_name       TEXT,
            to_addresses    TEXT NOT NULL,      -- JSON array
            cc_addresses    TEXT DEFAULT '[]',  -- JSON array
            bcc_addresses   TEXT DEFAULT '[]',  -- JSON array
            subject         TEXT,
            body_text       TEXT,               -- plain text body
            body_html       TEXT,               -- HTML body (stored for full-view)
            size_bytes      INTEGER,
            has_attachments  INTEGER DEFAULT 0,

            in_reply_to     TEXT,               -- Message-ID of parent
            references_ids  TEXT DEFAULT '[]',  -- JSON array of Message-IDs
            participant_changes TEXT,           -- JSON: {added: [...], removed: [...]}

            -- Sync metadata
            imap_flags      TEXT DEFAULT '[]',  -- JSON array (\\Seen, \\Flagged, etc.)
            gmail_labels    TEXT DEFAULT '[]',  -- JSON array (Gmail labels, empty for non-Gmail)
            fetched_at      INTEGER NOT NULL,

            -- Processing outputs (also written back as IMAP keywords when possible)
            classification  TEXT,               -- 'chat' | 'newsletter' | 'promotion' | 'update' | 'transactional'
            is_important    INTEGER DEFAULT 0,
            distilled_text  TEXT,               -- short chat-style extract
            processed_at    INTEGER,            -- NULL until processed

            -- Conversation assignment
            participant_key TEXT NOT NULL,       -- sorted, normalised participant list (excl. self)
            conversation_id TEXT NOT NULL,       -- hash(participant_key)

            -- Thread assignment (computed during rebuild_conversations)
            thread_id       TEXT,               -- hash of thread root message_id

            UNIQUE(account_id, imap_folder, imap_uid)
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conversation   ON messages(conversation_id, date DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_date           ON messages(account_id, date DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_classification ON messages(classification);
        CREATE INDEX IF NOT EXISTS idx_messages_from           ON messages(from_address);
        CREATE INDEX IF NOT EXISTS idx_messages_message_id     ON messages(message_id);
        CREATE INDEX IF NOT EXISTS idx_messages_thread          ON messages(account_id, thread_id);

        -- Dedup: skip duplicate messages across folders (first-seen folder wins)
        CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_dedup
        ON messages(account_id, message_id) WHERE message_id != '';

        -- Conversations (derived / materialised)
        CREATE TABLE IF NOT EXISTS conversations (
            id                  TEXT PRIMARY KEY,   -- hash(participant_key)
            account_id          TEXT NOT NULL REFERENCES accounts(id),
            participant_key     TEXT NOT NULL,
            participant_names   TEXT,               -- JSON object { email: display_name }
            classification      TEXT NOT NULL,      -- 'connections' | 'others' | 'important'
            last_message_date   INTEGER NOT NULL,
            last_message_preview TEXT,
            unread_count        INTEGER DEFAULT 0,
            total_count         INTEGER DEFAULT 0,
            is_muted            INTEGER DEFAULT 0,
            is_pinned           INTEGER DEFAULT 0,
            is_important        INTEGER DEFAULT 0,
            updated_at          INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_conversations_class ON conversations(account_id, classification, last_message_date DESC);
        CREATE INDEX IF NOT EXISTS idx_conversations_date  ON conversations(account_id, last_message_date DESC);

        -- Trust network
        CREATE TABLE IF NOT EXISTS entities (
            id              TEXT PRIMARY KEY,   -- UUID
            account_id      TEXT NOT NULL REFERENCES accounts(id),
            email           TEXT NOT NULL,
            display_name    TEXT,
            trust_level     TEXT NOT NULL,      -- 'user' | 'alias' | 'contact' | 'connection'
            source          TEXT,               -- 'carddav' | 'sent_scan' | 'manual'
            first_seen      INTEGER NOT NULL,
            last_seen       INTEGER,
            sent_count      INTEGER DEFAULT 0,  -- number of messages sent to this entity
            metadata        TEXT DEFAULT '{}',  -- JSON blob for CardDAV vCard fields etc.

            UNIQUE(account_id, email)
        );

        CREATE INDEX IF NOT EXISTS idx_entities_trust ON entities(account_id, trust_level);
        CREATE INDEX IF NOT EXISTS idx_entities_email ON entities(email);

        -- Action queue for optimistic updates
        CREATE TABLE IF NOT EXISTS action_queue (
            id              TEXT PRIMARY KEY,   -- UUID
            account_id      TEXT NOT NULL REFERENCES accounts(id),
            action_type     TEXT NOT NULL,      -- 'mark_read' | 'archive' | 'delete' | 'move' | 'flag' | 'send' | 'mute' | 'pin'
            payload         TEXT NOT NULL,      -- JSON
            status          TEXT NOT NULL DEFAULT 'pending',  -- 'pending' | 'in_progress' | 'completed' | 'failed'
            retry_count     INTEGER DEFAULT 0,
            max_retries     INTEGER DEFAULT 3,
            created_at      INTEGER NOT NULL,
            completed_at    INTEGER,
            error           TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_action_queue_status ON action_queue(status, created_at);

        -- Cross-device sync object version tracking
        CREATE TABLE IF NOT EXISTS sync_state (
            account_id      TEXT PRIMARY KEY REFERENCES accounts(id),
            draft_uid       INTEGER,            -- IMAP UID of the sync-object draft
            draft_version   INTEGER DEFAULT 0,
            last_pushed     INTEGER,            -- epoch ms
            last_pulled     INTEGER             -- epoch ms
        );

        -- Per-folder IMAP sync cursors
        CREATE TABLE IF NOT EXISTS folder_sync (
            account_id    TEXT NOT NULL,
            folder        TEXT NOT NULL,
            uid_validity  INTEGER NOT NULL DEFAULT 0,
            -- For incremental sync after onboarding
            highest_uid   INTEGER DEFAULT 0,
            -- The cursor for backwards historical fetch, during onboarding
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

        -- User-defined classification skills
        CREATE TABLE IF NOT EXISTS skills (
            id          TEXT PRIMARY KEY,
            account_id  TEXT NOT NULL REFERENCES accounts(id),
            name        TEXT NOT NULL,
            icon        TEXT NOT NULL DEFAULT '⚡',
            icon_bg     TEXT NOT NULL DEFAULT '#5b4fc7',
            enabled     INTEGER NOT NULL DEFAULT 1,
            prompt      TEXT NOT NULL DEFAULT '',
            modifiers   TEXT NOT NULL DEFAULT '{}',
            settings    TEXT NOT NULL DEFAULT '{}',
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_skills_account ON skills(account_id);

        -- Line groups (Lines view)
        CREATE TABLE IF NOT EXISTS line_groups (
            group_id    TEXT NOT NULL,
            account_id  TEXT NOT NULL REFERENCES accounts(id),
            domain      TEXT NOT NULL,
            PRIMARY KEY (account_id, domain)
        );

        CREATE INDEX IF NOT EXISTS idx_line_groups_group ON line_groups(account_id, group_id);

        -- Global key-value settings (shared across accounts)
        CREATE TABLE IF NOT EXISTS settings (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
    ")?;

    // Migrations for existing databases
    // Add sent_count column to entities (ignore error if already exists)
    let _ = conn.execute_batch("ALTER TABLE entities ADD COLUMN sent_count INTEGER DEFAULT 0;");
    // Add imap_tls column to accounts (defaults to 1 = true for existing accounts)
    let _ = conn.execute_batch("ALTER TABLE accounts ADD COLUMN imap_tls INTEGER NOT NULL DEFAULT 1;");
    // Add thread_id column to messages (computed during rebuild_conversations)
    let _ = conn.execute_batch("ALTER TABLE messages ADD COLUMN thread_id TEXT;");

    // Migration: clear domain-based line_groups (Lines now group by sender, not domain).
    // The 'domain' column is reused to store sender emails.
    let needs_lines_migration: bool = conn.query_row(
        "SELECT COUNT(*) = 0 FROM settings WHERE key = 'lines_v2_migrated'",
        [], |row| row.get(0),
    ).unwrap_or(true);
    if needs_lines_migration {
        let _ = conn.execute("DELETE FROM line_groups", []);
        let _ = conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('lines_v2_migrated', '1', 0)",
            [],
        );
    }

    Ok(())
}
