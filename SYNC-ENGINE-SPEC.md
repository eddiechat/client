# Sync Engine — System Design

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Client App                           │
│                                                             │
│  ┌───────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │    UI      │  │  ViewModel   │  │   Action Queue       │  │
│  │  Layer     │◄─┤  / Store     │◄─┤  (Optimistic Writes) │  │
│  └───────────┘  └──────┬───────┘  └──────────┬───────────┘  │
│                        │                      │              │
│                 ┌──────▼──────────────────────▼───────┐      │
│                 │          Sync Engine                │      │
│                 │                                     │      │
│                 │  ┌────────────┐  ┌───────────────┐  │      │
│                 │  │  Ingestion │  │  Processor    │  │      │
│                 │  │  Pipeline  │  │  Pipeline     │  │      │
│                 │  └─────┬──────┘  └──────┬────────┘  │      │
│                 │        │                │           │      │
│                 │  ┌─────▼────────────────▼────────┐  │      │
│                 │  │       SQLite Database         │  │      │
│                 │  │         (Cache)               │  │      │
│                 │  └──────────────────────────────-┘  │      │
│                 └────────────────┬────────────────────┘      │
│                                  │                           │
└──────────────────────────────────┼───────────────────────────┘
                                   │
                    ┌──────────────▼──────────────┐
                    │     IMAP / SMTP Server       │
                    │   (Single Source of Truth)    │
                    │                              │
                    │  ┌────────┐  ┌────────────┐  │
                    │  │ Mail   │  │  Drafts     │  │
                    │  │ Boxes  │  │  (Sync Obj) │  │
                    │  └────────┘  └────────────┘  │
                    │                              │
                    │  ┌────────────────────────┐   │
                    │  │  CardDAV (Contacts)    │   │
                    │  └────────────────────────┘   │
                    └──────────────────────────────┘
```

The system is composed of five major subsystems:

1. **IMAP Client** — manages connections, folder listing, fetching, IDLE, and flag/metadata writes.
2. **Ingestion Pipeline** — transforms raw IMAP messages into normalised database records.
3. **Processor Pipeline** — runs classification, distillation, and trust-network derivation.
4. **Sync Object Store** — reads and writes a serialised state blob stored as an IMAP draft, enabling cross-device sync without a server.
5. **Action Queue** — buffers user-initiated mutations, applies them optimistically to the local database, and reconciles after the server round-trip.

---

## 2. Data Model (SQLite)

### 2.1 Core Tables

```sql
-- Accounts & identity
CREATE TABLE accounts (
    id              TEXT PRIMARY KEY,   -- UUID
    email           TEXT NOT NULL UNIQUE,
    display_name    TEXT,
    imap_host       TEXT NOT NULL,
    imap_port       INTEGER NOT NULL DEFAULT 993,
    smtp_host       TEXT NOT NULL,
    smtp_port       INTEGER NOT NULL DEFAULT 587,
    carddav_url     TEXT,               -- nullable, contacts are optional
    created_at      INTEGER NOT NULL,   -- unix epoch ms
    last_full_sync  INTEGER             -- unix epoch ms, NULL until onboarding completes
);

CREATE TABLE aliases (
    id              TEXT PRIMARY KEY,
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    email           TEXT NOT NULL UNIQUE
);

-- Raw message store (cache of IMAP data)
CREATE TABLE messages (
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

    -- Sync metadata
    imap_flags      TEXT DEFAULT '[]',  -- JSON array (\Seen, \Flagged, etc.)
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

CREATE INDEX idx_messages_conversation   ON messages(conversation_id, date DESC);
CREATE INDEX idx_messages_date           ON messages(account_id, date DESC);
CREATE INDEX idx_messages_classification ON messages(classification);
CREATE INDEX idx_messages_from           ON messages(from_address);
CREATE INDEX idx_messages_message_id     ON messages(message_id);

-- Conversations (derived / materialised)
CREATE TABLE conversations (
    id                  TEXT PRIMARY KEY,   -- hash(participant_key)
    account_id          TEXT NOT NULL REFERENCES accounts(id),
    participant_key     TEXT NOT NULL,
    participant_names   TEXT,               -- JSON object { email: display_name }
    classification      TEXT NOT NULL,      -- 'connections' | 'others' | 'important'
    last_message_date   INTEGER NOT NULL,
    last_message_preview TEXT,
    unread_count        INTEGER DEFAULT 0,
    is_muted            INTEGER DEFAULT 0,
    is_pinned           INTEGER DEFAULT 0,
    updated_at          INTEGER NOT NULL
);

CREATE INDEX idx_conversations_class ON conversations(account_id, classification, last_message_date DESC);
CREATE INDEX idx_conversations_date  ON conversations(account_id, last_message_date DESC);

-- Trust network
CREATE TABLE entities (
    id              TEXT PRIMARY KEY,   -- UUID
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    email           TEXT NOT NULL,
    display_name    TEXT,
    trust_level     TEXT NOT NULL,      -- 'user' | 'alias' | 'contact' | 'connection'
    source          TEXT,               -- 'carddav' | 'sent_scan' | 'manual'
    first_seen      INTEGER NOT NULL,
    last_seen       INTEGER,
    metadata        TEXT DEFAULT '{}',  -- JSON blob for CardDAV vCard fields etc.

    UNIQUE(account_id, email)
);

CREATE INDEX idx_entities_trust ON entities(account_id, trust_level);
CREATE INDEX idx_entities_email ON entities(email);

-- Action queue for optimistic updates
CREATE TABLE action_queue (
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

CREATE INDEX idx_action_queue_status ON action_queue(status, created_at);

-- Cross-device sync object version tracking
CREATE TABLE sync_state (
    account_id      TEXT PRIMARY KEY REFERENCES accounts(id),
    draft_uid       INTEGER,            -- IMAP UID of the sync-object draft
    draft_version   INTEGER DEFAULT 0,
    last_pushed     INTEGER,            -- epoch ms
    last_pulled     INTEGER             -- epoch ms
);

-- Per-folder IMAP sync cursors
CREATE TABLE folder_sync (
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    folder          TEXT NOT NULL,
    uid_validity    INTEGER NOT NULL,
    highest_uid     INTEGER DEFAULT 0,
    last_sync       INTEGER,
    PRIMARY KEY (account_id, folder)
);
```

### 2.2 Key Design Decisions

- **`participant_key`** is computed as: take all addresses from `from`, `to`, `cc`; remove the user's own email and any aliases; lowercase and trim each; sort lexicographically; join with `\n`. This deterministically groups messages by their participant set.
- **`conversation_id`** is `SHA-256(participant_key)`, truncated to 16 hex characters for compactness.
- **`messages` is a cache**. Any row can be deleted and re-fetched from IMAP. Processing outputs are also written back to IMAP as custom keywords or X-headers where the server supports them, so re-ingestion on another device can skip reprocessing.
- **`conversations` is a materialised view** rebuilt from `messages`. It is not authoritative; it is recomputed during sync.

---

## 3. IMAP Sync Protocol

### 3.1 Connection Management

```
┌──────────────────────────────────────────────┐
│             IMAP Connection Pool              │
│                                               │
│  ┌─────────────┐  ┌─────────────────────────┐ │
│  │  Primary     │  │  Background             │ │
│  │  Connection  │  │  Connection(s)          │ │
│  │  (IDLE)      │  │  (FETCH / STORE / etc.) │ │
│  └─────────────┘  └─────────────────────────┘ │
└──────────────────────────────────────────────┘
```

- Maintain **one persistent IDLE connection** on INBOX for real-time push. Cycle IDLE every 25 minutes (RFC 2177 recommends < 29 min).
- Maintain **one or more worker connections** for fetching, storing flags, and sync operations.
- On mobile, IDLE is replaced by OS push notifications where available (e.g. via the provider's push gateway). When the app is backgrounded, the IDLE connection is dropped and a lightweight periodic poll is scheduled.

### 3.2 Folder Strategy

Not all folders are equally useful. The engine categorises them:

| Priority | Folders | Behaviour |
|----------|---------|-----------|
| High | INBOX, Sent, Drafts | Synced first. IDLE on INBOX. |
| Medium | Archive, All Mail | Synced after high-priority folders complete. |
| Low | Custom/user folders | Synced lazily or on demand. |
| Excluded | Spam, Trash | Never synced unless the user explicitly requests it. |

### 3.3 Incremental Sync (Steady State)

```
1. SELECT folder
2. Compare UIDVALIDITY with stored value
     → If changed: full re-sync of that folder (UIDs have been renumbered)
     → If unchanged: continue
3. FETCH uid_range (highest_stored_uid+1):* (FLAGS ENVELOPE BODYSTRUCTURE)
     → Insert new messages into the database
4. FETCH 1:highest_stored_uid (FLAGS)
     → Diff flags against stored flags; update changed rows
5. Detect deletions via EXPUNGE responses or UID comparison
6. Update folder_sync cursor
```

### 3.4 Writing Metadata Back to IMAP

Many IMAP servers support custom keywords (flags). The engine writes processing results back as keywords on the original message:

- `$Chat`, `$Newsletter`, `$Promotion`, `$Update`, `$Transactional` — classification
- `$Important` — importance flag
- `$Processed` — signals that distillation/classification has been completed

On re-ingestion (e.g. new device), the engine checks for `$Processed` before running expensive classification. If the keyword is present, the classification is read from the other keywords. The distilled text, being too long for a keyword, is stored in the sync object (see §6).

Servers that do not support custom keywords fall back to the sync-object approach entirely.

---

## 4. Ingestion Pipeline

```
  IMAP FETCH
      │
      ▼
┌─────────────┐    ┌────────────────┐    ┌──────────────────┐
│  Parse       │───►│  Normalise     │───►│  Compute         │
│  Envelope &  │    │  Addresses     │    │  participant_key  │
│  Body        │    │  & Names       │    │  & conversation_id│
└─────────────┘    └────────────────┘    └────────┬─────────┘
                                                   │
                                                   ▼
                                         ┌──────────────────┐
                                         │  Upsert into     │
                                         │  messages table   │
                                         └────────┬─────────┘
                                                   │
                                                   ▼
                                         ┌──────────────────┐
                                         │  Enqueue for      │
                                         │  Processing       │
                                         └──────────────────┘
```

### 4.1 Parsing

- Decode MIME structure. Extract `text/plain` and `text/html` parts. Prefer `text/plain` for distillation; store `text/html` for the full-message viewer.
- Parse all address headers (`From`, `To`, `Cc`, `Bcc`, `Reply-To`, `Sender`).
- Extract threading headers (`In-Reply-To`, `References`) for future thread-reconstruction use.

### 4.2 Address Normalisation

Addresses are normalised before any comparison or key computation:
1. Lowercase the entire address.
2. Strip leading/trailing whitespace.
3. For known providers (Gmail, Outlook), strip subaddress tags (the `+tag` portion before `@`).
4. For Gmail, strip dots from the local part (e.g. `j.doe@gmail.com` → `jdoe@gmail.com`).

This normalisation is applied everywhere: participant keys, trust network lookups, and entity deduplication.

### 4.3 Participant Key & Conversation ID

```python
def compute_participant_key(from_addr, to_addrs, cc_addrs, self_emails):
    """
    self_emails: set of user's own email + all aliases, normalised.
    """
    all_participants = set()
    all_participants.add(normalise(from_addr))
    for addr in to_addrs + cc_addrs:
        all_participants.add(normalise(addr))

    # Remove self
    others = sorted(all_participants - self_emails)

    # Edge case: message to self (e.g. notes)
    if not others:
        return "__self__"

    return "\n".join(others)

def compute_conversation_id(participant_key):
    return hashlib.sha256(participant_key.encode()).hexdigest()[:16]
```

### 4.4 Batch Ingestion

During onboarding, messages are fetched in reverse-chronological order (newest first) so that the UI can render a meaningful conversation list as early as possible. Batch size: 50 envelopes per FETCH command, with bodies fetched in a second pass or lazily on demand.

---

## 5. Processor Pipeline

Processing is divided into three stages, each of which can run independently:

```
┌──────────────────────────────────────────────────────────┐
│                    Processor Pipeline                     │
│                                                          │
│  ┌────────────┐   ┌────────────────┐   ┌──────────────┐  │
│  │  Stage 1   │   │   Stage 2      │   │   Stage 3    │  │
│  │  Classify  │──►│   Distill      │──►│   Derive     │  │
│  │  (fast)    │   │   (moderate)   │   │   (rebuild)  │  │
│  └────────────┘   └────────────────┘   └──────────────┘  │
└──────────────────────────────────────────────────────────┘
```

### 5.1 Stage 1 — Classification (Fast)

**Goal**: assign `classification` and `is_important` to each message.

**Heuristics-first approach** (runs on all devices, including mobile):

1. Check for IMAP keywords (`$Chat`, `$Newsletter`, etc.) — if present, accept them and skip.
2. Check headers for known automated-sender signals:
   - `List-Unsubscribe` header → `newsletter` or `promotion`
   - `X-Mailer` matching known transactional senders → `transactional`
   - `Auto-Submitted: auto-generated` → `update`
   - `Precedence: bulk` → `newsletter`
3. Check the `From` address against known no-reply patterns (`noreply@`, `no-reply@`, `donotreply@`).
4. Check if the sender is in the trust network as a `connection` or `contact` — strong signal for `chat`.
5. Fall back to a lightweight on-device text classifier (TF-IDF or a small model) that scores the body.

**Importance** is derived from:
- The IMAP `\Flagged` flag.
- A `Priority` or `X-Priority` header with value 1 or 2.
- Heuristic: message is from a `connection`, is `chat`, and contains question marks or action-oriented language.

### 5.2 Stage 2 — Distillation (Moderate Cost)

**Goal**: extract a short, chat-style preview from the message body.

**Algorithm**:

```
Input: body_text (plain text)

1. Split into lines.
2. Strip leading greeting block:
   - Remove lines matching /^(hi|hey|hello|dear|good (morning|afternoon|evening))/i
     until a non-empty, non-greeting line is found.
3. Strip trailing signature block:
   - Detect "-- \n" (standard sig separator) or heuristic patterns
     (lines starting with "Sent from", "Best regards", phone numbers, etc.)
   - Detect quoted reply blocks (lines starting with ">") and remove them.
4. Strip trailing whitespace and empty lines.
5. Take the first N characters (default: 300) of the remaining text.
6. If truncated, append "…"
```

On desktop with AI capabilities, a local LLM (or API call to a privacy-respecting model) can produce a higher-quality distillation: a 1–2 sentence summary that captures the intent. This runs asynchronously and updates `distilled_text` when complete.

### 5.3 Stage 3 — Derivation (Rebuild)

After new messages are ingested and processed, the engine rebuilds affected conversations:

```python
def rebuild_conversation(conversation_id):
    messages = db.query(
        "SELECT * FROM messages WHERE conversation_id = ? ORDER BY date DESC",
        conversation_id
    )

    if not messages:
        db.execute("DELETE FROM conversations WHERE id = ?", conversation_id)
        return

    has_chat = any(m.classification == 'chat' for m in messages)
    has_trusted = any(is_trusted(m.from_address) for m in messages)
    has_important = any(m.is_important for m in messages)

    if has_chat and has_trusted:
        classification = 'connections'
    elif has_important:
        classification = 'important'
    else:
        classification = 'others'

    latest = messages[0]
    unread = sum(1 for m in messages if '\\Seen' not in m.imap_flags)

    db.upsert("conversations", {
        'id': conversation_id,
        'classification': classification,
        'last_message_date': latest.date,
        'last_message_preview': latest.distilled_text or latest.subject,
        'unread_count': unread,
        'participant_key': latest.participant_key,
        # ... other fields
    })
```

---

## 6. Cross-Device Sync Object

Since there is no server, device-to-device synchronisation relies on a **draft message stored via IMAP** as a shared state blob.

### 6.1 Structure

The sync object is stored as a draft email message:

```
From: <user's email>
To: <user's email>
Subject: __sync_engine_state_v1__
X-Sync-Version: <monotonically increasing integer>
Content-Type: application/json; charset=utf-8

{
  "version": 42,
  "updated_at": "2026-02-07T10:30:00Z",
  "device_id": "device-abc123",

  "distillations": {
    "<message_id>": "Hey, are you free for lunch tomorrow?",
    ...
  },

  "classifications": {
    "<message_id>": {
      "classification": "chat",
      "is_important": false
    },
    ...
  },

  "conversation_overrides": {
    "<conversation_id>": {
      "is_muted": true,
      "is_pinned": false
    },
    ...
  },

  "entity_overrides": {
    "<email>": {
      "trust_level": "connection",
      "display_name": "Alice"
    },
    ...
  }
}
```

### 6.2 Read/Write Protocol

```
PULL (on startup or periodic interval):
  1. Search Drafts folder for Subject = "__sync_engine_state_v1__"
  2. Fetch the body
  3. Parse JSON
  4. If remote version > local version:
       → Merge remote state into local DB (see §6.3)
       → Update sync_state.last_pulled

PUSH (after local processing produces new data):
  1. Read current local state
  2. Increment version
  3. Serialise to JSON
  4. If draft_uid exists:
       → STORE the updated body by deleting old draft + appending new one
         (IMAP does not support in-place edits of message bodies)
  5. Else:
       → APPEND new draft to Drafts folder
  6. Update sync_state.last_pushed and sync_state.draft_version
```

### 6.3 Merge Strategy

The sync object uses a **last-writer-wins (LWW) per key** strategy:

- Each entry carries the `updated_at` timestamp of the device that wrote it.
- On merge, for each key the entry with the later timestamp wins.
- Conflicts are rare because keys are per-message or per-conversation, and two devices are unlikely to process the same message simultaneously.

For `conversation_overrides` (mute, pin), user-initiated actions always take priority over derived values because they carry a later timestamp.

### 6.4 Size Management

The sync object can grow large over time. Mitigations:

- **TTL pruning**: entries older than 12 months are dropped (the corresponding messages would also have been pruned from the local cache).
- **Deduplication with IMAP keywords**: any classification that has been successfully written back as an IMAP keyword is removed from the sync object, since the next device can read it from the message directly.
- **Compression**: the JSON body is gzip-compressed before storing as the draft body (using `Content-Transfer-Encoding: base64` with a compressed payload).
- **Sharding**: if the object exceeds 1 MB even after compression, shard into multiple drafts keyed by date range (`__sync_engine_state_v1__2025__`, `__sync_engine_state_v1__2024__`, etc.).

---

## 7. Trust Network Management

### 7.1 Initial Build (Onboarding)

```
1. Fetch Sent folder message envelopes (all time, headers only).
2. For each sent message:
     → Extract all To/Cc/Bcc addresses
     → Upsert into entities with trust_level = 'connection'
3. If CardDAV is configured:
     → Fetch all contacts
     → Upsert into entities with trust_level = 'contact'
       (promote to 'connection' if already present from sent scan)
4. User's own address → trust_level = 'user'
5. Configured aliases → trust_level = 'alias'
```

### 7.2 Incremental Updates

- Every time a new message is sent (via the app or detected in the Sent folder), all recipients are upserted as `connection` if not already present.
- CardDAV sync runs periodically (e.g. once per day) to pick up new contacts.
- Users can manually promote or demote entities through the UI, stored as `entity_overrides` in the sync object.

### 7.3 Lookup

```python
def is_trusted(email, account_id):
    """Returns True if the email belongs to a user, alias, contact, or connection."""
    normalised = normalise(email)
    entity = db.query(
        "SELECT trust_level FROM entities WHERE account_id = ? AND email = ?",
        account_id, normalised
    )
    return entity is not None  # any trust_level counts as trusted
```

---

## 8. Conversation List & Filtering

### 8.1 Queries

The UI presents a filterable conversation list. Each filter maps to a simple query:

```sql
-- "Connections" tab
SELECT * FROM conversations
WHERE account_id = ? AND classification = 'connections'
ORDER BY last_message_date DESC;

-- "Others" tab
SELECT * FROM conversations
WHERE account_id = ? AND classification = 'others'
ORDER BY last_message_date DESC;

-- "Important" tab (cross-cutting: any conversation with at least one important message)
SELECT * FROM conversations
WHERE account_id = ? AND classification = 'important'
ORDER BY last_message_date DESC;

-- "All" tab
SELECT * FROM conversations
WHERE account_id = ?
ORDER BY last_message_date DESC;
```

### 8.2 Pagination

Conversations are paginated with keyset pagination using `(last_message_date, id)` as the cursor, which avoids the performance pitfalls of `OFFSET`:

```sql
SELECT * FROM conversations
WHERE account_id = ?
  AND classification = 'connections'
  AND (last_message_date, id) < (?, ?)
ORDER BY last_message_date DESC, id DESC
LIMIT 30;
```

---

## 9. Action Queue & Optimistic Updates

### 9.1 Lifecycle

```
User Action
    │
    ▼
┌────────────────────┐
│  1. Create action   │
│     in action_queue │
│     (status=pending)│
└─────────┬──────────┘
          │
          ▼
┌─────────────────────────┐
│  2. Apply optimistic     │
│     update to local DB   │
│     (immediate UI change)│
└─────────┬───────────────┘
          │
          ▼
┌─────────────────────────┐
│  3. Execute against IMAP │
│     (background thread)  │
│     status → in_progress │
└─────────┬───────────────┘
          │
    ┌─────┴──────┐
    │            │
    ▼            ▼
 Success      Failure
    │            │
    ▼            ▼
 status →    retry_count++
 completed   if < max_retries:
    │          re-enqueue
    │        else:
    │          status → failed
    │          ROLLBACK optimistic
    │          update & notify user
    ▼
  Remove from
  queue
```

### 9.2 Action Types

| Action | Optimistic Update | IMAP Operation |
|--------|-------------------|----------------|
| `mark_read` | Set `\Seen` in local flags, decrement `unread_count` | `STORE +FLAGS (\Seen)` |
| `mark_unread` | Remove `\Seen`, increment `unread_count` | `STORE -FLAGS (\Seen)` |
| `archive` | Move message to archive locally, update conversation | `MOVE` or `COPY + STORE \Deleted` |
| `delete` | Remove from local DB, update conversation | `STORE +FLAGS (\Deleted)` then `EXPUNGE` |
| `flag` | Set `\Flagged` locally | `STORE +FLAGS (\Flagged)` |
| `mute` | Set `is_muted` on conversation | Write to sync object |
| `pin` | Set `is_pinned` on conversation | Write to sync object |
| `send` | Insert into conversation immediately | `SMTP SEND` + `APPEND` to Sent |

### 9.3 Rollback on Failure

When an action fails after all retries:
1. The optimistic change is reversed in the local database (e.g. mark as unread again if `mark_read` failed).
2. The affected conversation is rebuilt.
3. The UI is notified via a reactive data flow (observable query or event bus), causing it to re-render.
4. A transient error banner is shown to the user.

---

## 10. Onboarding Flow

Onboarding is the most latency-sensitive phase. The goal is to show a usable conversation list within seconds, while full processing continues in the background.

```
Time ──────────────────────────────────────────────────────►

Phase 1: Bootstrap (seconds)
├─ Connect IMAP
├─ LIST folders
├─ SELECT INBOX
├─ FETCH last 200 envelopes (headers only, newest first)
├─ Compute participant_keys → insert conversations (unclassified)
├─ ██ UI: render preliminary conversation list (sorted by date) ██
│
Phase 2: Trust Network (parallel, ~30s)
├─ FETCH Sent folder envelopes (all time, headers only)
├─ Build entity table (connections)
├─ Fetch CardDAV contacts (if configured)
├─ ██ UI: reclassify conversations as trust data arrives ██
│
Phase 3: Deep Fetch (background, minutes)
├─ FETCH bodies for last 12 months of messages
├─ Run classification heuristics (Stage 1)
├─ Run distillation (Stage 2)
├─ Rebuild conversations (Stage 3)
├─ ██ UI: progressively update previews and classifications ██
│
Phase 4: Expansion
├─ For conversations classified as "connections":
│    FETCH all historical messages for those participant groups
├─ Continue processing
│
Phase 5: Finalise
├─ Push sync object to Drafts
├─ Set last_full_sync timestamp
└─ Start IDLE for real-time updates
```

### Interruption Handling

Each phase records progress in `folder_sync` (highest UID processed). If the app is closed mid-onboarding, it resumes from the last checkpoint without re-fetching already-processed messages.

---

## 11. Real-Time Updates (Post-Onboarding)

```
┌──────────────┐
│  IMAP IDLE   │
│  on INBOX    │
└──────┬───────┘
       │ EXISTS notification
       ▼
┌──────────────────────┐
│  FETCH new UIDs      │
│  (envelope + body)   │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  Ingestion Pipeline  │
│  (parse, normalise,  │
│   compute keys)      │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  Processor Pipeline  │
│  (classify, distill) │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  Rebuild affected    │
│  conversation        │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  Notify UI (reactive │
│  query / event)      │
└──────────────────────┘
```

A periodic full sync (every 5–15 minutes) catches changes that IDLE may miss, such as messages arriving in folders other than INBOX, flag changes made by other clients, and deletions.

---

## 12. Error Handling & Resilience

### 12.1 IMAP Connection Errors

| Scenario | Response |
|----------|----------|
| Connection lost | Exponential backoff reconnect (1s, 2s, 4s… max 5 min). Queue actions. |
| Authentication failure | Surface to user immediately. Pause all sync. |
| UIDVALIDITY changed | Full re-sync of affected folder. Remap UIDs. |
| Server timeout during FETCH | Retry with smaller batch size. |
| Folder not found | Skip gracefully. Log warning. |

### 12.2 Data Consistency

- **Duplicate detection**: messages are deduplicated by `message_id` (RFC 5322 header). If the same message appears in multiple folders, the INBOX copy takes precedence for conversation assignment; additional copies are noted but not duplicated.
- **Orphaned conversations**: during periodic maintenance (every 24h), conversations with zero messages are pruned.
- **Sync object conflicts**: if two devices push simultaneously, the later APPEND wins (the first draft is deleted). The losing device detects this on next pull and re-merges.

### 12.3 Database Recovery

Since the database is a cache, the nuclear recovery option is always available: delete the SQLite file entirely and re-run onboarding. The sync object in Drafts preserves user overrides (mute, pin, manual trust adjustments), so these survive a full cache rebuild.

---

## 13. Performance Considerations

### 13.1 SQLite Tuning

```sql
PRAGMA journal_mode = WAL;          -- concurrent reads during writes
PRAGMA synchronous = NORMAL;        -- safe for WAL mode, faster than FULL
PRAGMA cache_size = -8000;          -- 8 MB page cache
PRAGMA mmap_size = 268435456;       -- 256 MB memory-mapped I/O
PRAGMA temp_store = MEMORY;         -- temp tables in memory
PRAGMA foreign_keys = ON;
```

### 13.2 Write Batching

During bulk ingestion (onboarding), writes are batched inside explicit transactions of 100–500 rows. This avoids per-row transaction overhead, which can otherwise dominate ingestion time.

### 13.3 Processing Budget

On mobile, the processor pipeline is given a **time budget per sync cycle** (e.g. 2 seconds of CPU time). Messages that exceed the budget are deferred to the next cycle or left for a desktop device to process. Priority order:

1. Messages in INBOX from the last 24 hours.
2. Messages in INBOX from the last 7 days.
3. Messages classified as `chat` from the last 30 days.
4. Everything else.

### 13.4 Memory Management

- Message bodies are not held in memory beyond the processing stage. After distillation, only the `distilled_text` and metadata remain in the active working set.
- The full `body_html` is fetched on demand when the user opens a message's full view.
- On low-memory devices, `body_text` and `body_html` can be evicted from the cache and re-fetched from IMAP when needed.

### 13.5 Estimated Scale

For a typical user with 50,000 messages over 10 years:

| Metric | Estimate |
|--------|----------|
| Messages in cache (12 months) | ~5,000 |
| Expanded connection messages | ~2,000 additional |
| SQLite DB size | ~50–80 MB |
| Sync object size (compressed) | ~200 KB |
| Onboarding Phase 1 (to first render) | < 5 seconds |
| Onboarding full completion | 5–15 minutes |
| Incremental sync cycle | < 2 seconds |

---

## 14. Security & Privacy

Since the core principle is zero additional servers:

- **No data leaves the device** except through IMAP/SMTP to the user's own mail server, and optionally CardDAV for contacts.
- **No telemetry, analytics, or crash reporting** that transmits message content.
- **AI processing** (classification, distillation) runs entirely on-device. If a cloud LLM is used for higher-quality distillation, this must be explicit opt-in, with a clear disclosure, and ideally routed through a privacy-preserving API that does not retain inputs.
- **The sync object** stored as a draft is only as secure as the user's email account. It does not contain message bodies — only short distillations and metadata. Users with heightened security needs can disable the sync object entirely and accept per-device reprocessing.
- **SQLite database** should be encrypted at rest using SQLCipher or the platform's native full-disk encryption.
- **Credentials** (IMAP/SMTP passwords or OAuth tokens) are stored in the platform's secure keychain, never in the database or sync object.