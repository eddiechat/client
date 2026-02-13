use rusqlite::Connection;

use crate::types::error::EddieError;

pub fn initialize_schema(conn: &Connection) -> Result<(), EddieError> {
    conn.execute_batch("
        -- Minimal account reference for FK integrity.
        -- Full account data lives in the Config DB (connection_configs table).
        -- The id is the email address (matching Config DB's account_id).
        CREATE TABLE IF NOT EXISTS accounts (
            id              TEXT PRIMARY KEY,   -- email address (matches Config DB account_id)
            email           TEXT NOT NULL UNIQUE,
            created_at      INTEGER NOT NULL    -- unix epoch ms
        );

        -- Raw message store (cache of IMAP data)
        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,   -- UUID
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
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
            fetched_at      INTEGER NOT NULL,

            -- Processing outputs (also written back as IMAP keywords when possible)
            classification  TEXT,               -- 'chat' | 'newsletter' | 'promotion' | 'update' | 'transactional'
            is_important    INTEGER DEFAULT 0,
            distilled_text  TEXT,               -- short chat-style extract
            processed_at    INTEGER,            -- NULL until processed

            -- Conversation assignment
            participant_key TEXT NOT NULL,       -- sorted, normalised participant list (excl. self)
            conversation_id TEXT NOT NULL,       -- hash(participant_key)

            UNIQUE(account_id, imap_folder, imap_uid)
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conversation   ON messages(conversation_id, date DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_date           ON messages(account_id, date DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_classification ON messages(classification);
        CREATE INDEX IF NOT EXISTS idx_messages_from           ON messages(from_address);
        CREATE INDEX IF NOT EXISTS idx_messages_message_id     ON messages(message_id);

        -- Conversations (derived / materialised)
        CREATE TABLE IF NOT EXISTS conversations (
            id                  TEXT PRIMARY KEY,   -- hash(participant_key)
            account_id          TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
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
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
            email           TEXT NOT NULL,
            display_name    TEXT,
            trust_level     TEXT NOT NULL,      -- 'user' | 'alias' | 'contact' | 'connection'
            source          TEXT,               -- 'carddav' | 'sent_scan' | 'manual'
            first_seen      INTEGER NOT NULL,
            last_seen       INTEGER,
            metadata        TEXT DEFAULT '{}',  -- JSON blob for CardDAV vCard fields etc.

            UNIQUE(account_id, email)
        );

        CREATE INDEX IF NOT EXISTS idx_entities_trust ON entities(account_id, trust_level);
        CREATE INDEX IF NOT EXISTS idx_entities_email ON entities(email);

        -- Action queue for optimistic updates
        CREATE TABLE IF NOT EXISTS action_queue (
            id              TEXT PRIMARY KEY,   -- UUID
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
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
            account_id      TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
            draft_uid       INTEGER,            -- IMAP UID of the sync-object draft
            draft_version   INTEGER DEFAULT 0,
            last_pushed     INTEGER,            -- epoch ms
            last_pulled     INTEGER             -- epoch ms
        );

        -- Per-folder IMAP sync cursors
        CREATE TABLE IF NOT EXISTS folder_sync (
            account_id    TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
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
            account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
            task        TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'pending',
            cursor      TEXT,
            updated_at  INTEGER,
            PRIMARY KEY (account_id, task)
        );
    ")?;

    Ok(())
}
