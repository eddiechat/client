# IMAP Sync Engine

This document describes the sync engine that powers Eddie Chat's email synchronization. The engine runs as an independent async worker inside the Tauri backend, fetching messages via IMAP, classifying them, and materializing conversations for the frontend.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Tauri App (lib.rs)                                     │
│                                                         │
│  ┌──────────────┐    wake channel     ┌──────────────┐  │
│  │   Commands   │ ──────────────────► │  Sync Worker │  │
│  │  (sync_now,  │    mpsc::channel    │  (15s tick)  │  │
│  │   connect)   │                     └──────┬───────┘  │
│  └──────────────┘                            │          │
│                                              ▼          │
│                              ┌───────────────────────┐  │
│                              │  Onboarding or        │  │
│                              │  Recurring Sync       │  │
│                              └───────────┬───────────┘  │
│                                          │              │
│           ┌──────────────────────────────┼──────────┐   │
│           ▼                              ▼          ▼   │
│   ┌──────────────┐  ┌────────────────┐  ┌────────────┐  │
│   │ IMAP Adapter │  │ SQLite Adapter │  │  Helpers   │  │
│   │ (connection, │  │ (messages, DB, │  │ (classify, │  │
│   │  folders,    │  │  folder_sync,  │  │  distill,  │  │
│   │  envelopes)  │  │  conversations)│  │  emit)     │  │
│   └──────────────┘  └────────────────┘  └────────────┘  │
│                              │                          │
│                              ▼                          │
│                     ┌────────────────┐                  │
│                     │  sync.db       │                  │
│                     │  (SQLite/WAL)  │                  │
│                     └────────────────┘                  │
└─────────────────────────────────────────────────────────┘
```

The engine has two phases:

1. **Onboarding** — When a new account is added, three sequential tasks run to build initial state: trust network discovery, historical message fetch, and connection history expansion.
2. **Recurring sync** — Incremental message fetching and flag resynchronization run on **every tick**, even during onboarding, so new mail keeps arriving for already-onboarded accounts while a new account is being set up.

### Key source files

| Area | Files |
|------|-------|
| Worker loop | `services/sync/worker.rs` |
| Onboarding tasks | `services/sync/tasks/trust_network.rs`, `historical_fetch.rs`, `connection_history.rs` |
| Recurring sync | `services/sync/tasks/incremental_sync.rs`, `flag_resync.rs` |
| IMAP protocol | `adapters/imap/connection.rs`, `folders.rs`, `envelopes.rs`, `historical.rs`, `sent_scan.rs` |
| SQLite persistence | `adapters/sqlite/sync/messages.rs`, `conversations.rs`, `folder_sync.rs`, `onboarding_tasks.rs`, `entities.rs` |
| Processing helpers | `services/sync/helpers/entity_extraction.rs`, `message_classification.rs`, `message_distillation.rs`, `message_builder.rs`, `email_normalization.rs`, `status_emit.rs` |

All paths are relative to `src-tauri/src/`.

---

## Worker Tick Loop

The sync worker is spawned as an async task in `lib.rs` and ticks on a 15-second interval. Commands can wake it immediately via an mpsc channel.

```
loop {
    did_work = worker::tick()
    if did_work → continue immediately (no sleep)
    else → tokio::select! {
        wake_rx.recv()                          // woken by command
        sleep(Duration::from_secs(15))          // timeout
    }
}
```

Each tick executes **one unit of work** and returns whether anything was done. If work was completed, the loop continues immediately without sleeping, allowing the engine to churn through onboarding tasks rapidly.

### Tick decision tree

```
tick()
├── Run incremental_sync_all + flag_resync_all (always, for all onboarded accounts)
├── Find account needing onboarding?
│   ├── Yes:
│   │   ├── Tasks seeded? No → seed 3 tasks, return true
│   │   ├── Next pending task? Yes → run it, return true
│   │   └── All tasks done? → return false (incremental sync already ran above)
│   └── No onboarding needed → return false
```

Incremental sync and flag resync run first on every tick, ensuring new mail arrives even during long-running onboarding. The `did_work` return (`true`/`false`) only reflects whether onboarding work was done, controlling whether the loop sleeps or continues immediately.

When the last onboarding task (`connection_history`) completes, the worker clears all onboarding status messages so the UI no longer shows progress indicators.

### Wake channel

Commands like `sync_now` and `connect_account` send a `()` signal on the wake channel, causing the worker to break out of its 15-second sleep and tick immediately. The channel has a buffer of 1, so multiple rapid wake signals coalesce into a single tick.

---

## IMAP Connection

Connections are established in `adapters/imap/connection.rs`:

1. TCP connection to `host:port`
2. TLS handshake using the OS-native TLS stack (`async-native-tls`)
3. IMAP LOGIN with username/password
4. Folder selected via `EXAMINE` (read-only — never modifies mailbox state)

### Gmail detection

Gmail is detected by hostname (`gmail.com` or `googlemail.com`). When detected, `has_gmail_ext` is set to `true` on the `ImapConnection` struct. This enables Gmail-specific behavior throughout the sync engine:

- `X-GM-LABELS` is added to FETCH queries to retrieve Gmail labels
- Labels are stored in the `gmail_labels` column (JSON array) on the messages table
- Leading backslashes are stripped from system labels (`\Important` → `Important`)
- Only `[Gmail]/All Mail` is synced (see Folder Discovery below)
- Flag resync fetches and compares labels alongside flags

---

## Folder Discovery

Folders are fetched from the IMAP server via the LIST command and classified by priority:

| Priority | Match | Folders | Behavior |
|----------|-------|---------|----------|
| High | Name | INBOX | Core folder (matched by name) |
| High | Attribute | Sent, Drafts | Core folders (matched by `\Sent`, `\Drafts` attributes) |
| Medium | Attribute | All Mail | Archive-like (matched by `\All` attribute) |
| Low | — | Custom folders | User-created (no recognized attribute) |
| Excluded | Attribute | Junk, Trash | Skipped (matched by `\Junk`, `\Trash` attributes) |
| NoSelect | Attribute | Structural-only | Cannot be selected |

Classification is **attribute-based** using RFC 6154 special-use mailbox attributes (`\Sent`, `\Drafts`, `\All`, etc.). The only exception is INBOX, which is matched by name. If a server does not advertise special-use attributes, folders other than INBOX will be classified as Low priority. The Sent folder has additional name-based fallback logic (see `find_sent_folder()` below); other folder roles (Drafts, Trash, etc.) remain attribute-only.

### Sync folder filtering

The `folders_to_sync()` function accepts an `is_gmail` flag and returns only folders that should be actively synced:

**Gmail accounts:** Only sync folders with the `\All` attribute (i.e., `[Gmail]/All Mail`). This folder contains every message exactly once regardless of labels, eliminating duplicates that would arise from syncing label-based folders like INBOX, Sent, Important, etc.

**Non-Gmail accounts:** Sync all folders except those with any of these attributes:
- `\Drafts`, `\Trash`, `\Junk` (content not useful for conversations)
- `\All` (would duplicate INBOX/Sent content)
- `\Flagged` (virtual folder)
- `\NoSelect` (structural containers)

Note: folders without recognized attributes are included in sync (they get Low priority). A folder named "Sent Items" without the `\Sent` attribute would still be synced, but would not be recognized as the Sent folder for trust network scanning.

The Sent folder is identified separately via `find_sent_folder()` for trust network scanning, using a multi-tier strategy:

1. **Attribute match** — Search for a folder with the `\Sent` special-use attribute (RFC 6154). Most reliable.
2. **Name-based fallback** — Match the folder's leaf name (last segment after `.` or `/` delimiter) against a list of known Sent folder names across European languages: English (`Sent`, `Sent Items`, `Sent Messages`, `Sent Mail`), German (`Gesendet`, `Gesendete Objekte`), French (`Envoyés`, `Éléments envoyés`), Spanish (`Enviados`), Portuguese (`Enviadas`, `Itens Enviados`), Italian (`Inviata`, `Inviati`), Dutch (`Verzonden`), Swedish (`Skickat`), Danish/Norwegian (`Sendt`), Finnish (`Lähetetyt`), Polish (`Wysłane`), Czech (`Odeslané`), Hungarian (`Elküldött`), Romanian (`Trimise`), Russian (`Отправленные`), Turkish (`Gönderilenler`), Greek (`Απεσταλμένα`).
3. **FROM-user scan** — If no Sent folder is found by attribute or name, all syncable folders are scanned for messages `FROM "user@email.com"`. This catches sent messages regardless of which folder they're in (unrecognized folder name, unusual server layout, or messages scattered across folders). Recipients of those messages are extracted to build the trust network. This runs as a single-pass operation with a larger batch size (5000) since the FROM filter typically yields far fewer messages than a full Sent folder scan.

If the FROM-user scan also finds no messages (e.g., brand-new account), self entities are still seeded and the task is marked done. The trust network will be empty but onboarding continues normally.

### Folder sync state

Each synced folder has a row in the `folder_sync` table tracking:

```
account_id | folder | highest_uid | lowest_uid | sync_status | last_sync
-----------+--------+-------------+------------+-------------+----------
abc123     | INBOX  | 5432        | 100        | done        | 1708270120
abc123     | Sent   | 3210        | 50         | done        | 1708269000
abc123     | Archive| 0           | 0          | pending     | NULL
```

- `highest_uid` — Latest UID fetched (cursor for incremental sync)
- `lowest_uid` — Oldest UID fetched (cursor for historical backfill)
- `sync_status` — `pending` or `done`
- `last_sync` — Timestamp of last sync (used for round-robin ordering)

Folders are processed in priority order: never-synced first (`last_sync IS NULL`), then oldest-synced, with INBOX prioritized over Sent, and Sent over other folders.

---

## Message Deduplication

The engine uses two complementary strategies to prevent duplicate messages:

### Gmail: Single-folder sync

Gmail exposes labels as IMAP folders, so the same message appears in multiple folders (INBOX, Sent, Important, etc.). By syncing only `[Gmail]/All Mail`, each message is fetched exactly once. The original labels are preserved in the `gmail_labels` column via `X-GM-LABELS`.

### Non-Gmail: Message-ID dedup

A partial unique index enforces one row per `(account_id, message_id)`:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_dedup
ON messages(account_id, message_id) WHERE message_id != '';
```

The `WHERE message_id != ''` clause allows messages without a Message-ID header (stored as `""`) to coexist without constraint violations. Messages are inserted with `INSERT OR IGNORE`, so the first-seen folder wins and subsequent duplicates from other folders are silently skipped.

The original per-folder uniqueness constraint `UNIQUE(account_id, imap_folder, imap_uid)` is retained as a secondary safeguard.

---

## Onboarding Phase

When a new account is added, three tasks are seeded into the `onboarding_tasks` table and executed sequentially across worker ticks:

```
1. trust_network       → Scan Sent folder for contacts
2. historical_fetch    → Fetch 12 months of messages from sync-eligible folders
3. connection_history  → Expand threads with known connections
```

Each task is marked `done` when complete. The worker advances to the next pending task on each tick.

### Task 1: Trust Network

**Goal:** Discover who the user communicates with by scanning the Sent folder.

**Steps (per tick):**
1. Connect to IMAP, insert user + alias entities (first tick only)
2. Locate Sent folder via `find_sent_folder()` (attribute match → name fallback)
3. If no Sent folder found → fall back to FROM-user scan of all syncable folders (single pass, see below)
4. Read UID cursor from `onboarding_tasks.cursor` (0 on first tick)
5. `UID SEARCH` for messages above cursor, take first 500 UIDs
6. `UID FETCH` those UIDs for `BODY.PEEK[HEADER.FIELDS (To Cc Bcc)]`
7. Parse recipients via `mailparse`, build entity records with per-batch `sent_count`
8. Upsert entities (additive `sent_count`), persist cursor to max UID processed
9. If UIDs remain → return (next tick continues from cursor)
10. If no UIDs remain → run `process_changes()` and mark task done

**FROM-user fallback (step 3):** When no Sent folder is identified, each syncable folder is selected and searched for `FROM "user@email.com"`. Recipients are extracted from matching messages and upserted as connections. This runs as a single-tick operation (no cursor) since the FROM filter typically yields far fewer results. If no messages are found at all (e.g., brand-new account), the task completes with an empty trust network.

**Batch size:** 500 messages per tick (Sent folder path), 5000 per folder (FROM-user fallback)
**Resumability:** Yes — UID cursor persisted in `onboarding_tasks.cursor` between ticks and app restarts (Sent folder path only; FROM-user fallback runs in one pass)
**IMAP operations:** SELECT folder, UID SEARCH [FROM], UID FETCH recipient headers

### Task 2: Historical Fetch

**Goal:** Fetch 12 months of email history from sync-eligible folders (All Mail for Gmail, INBOX/Sent/custom for non-Gmail).

**Steps (per tick):**
1. Connect to IMAP, list folders, call `ensure_folder()` for each sync folder
2. Pick next pending folder via `next_pending_folder()` (round-robin by `last_sync`, INBOX first)
3. IMAP SEARCH for messages `SINCE <12 months ago>`
4. Filter to UIDs below `lowest_uid` (only fetch older messages not yet seen)
5. Fetch in batches of 200 using the 3-round-trip strategy (see below)
6. For each batch: insert messages, update `lowest_uid`/`highest_uid`, run `process_changes()`
7. If no messages remain → mark folder `done`; otherwise leave `in_progress` for next tick
8. Task is marked done only when all folders are `done`

**Batch size:** 200 messages
**Resumability:** Yes — `lowest_uid` cursor per folder, persisted between app restarts
**Max batches per tick:** 1 (prevents blocking the worker indefinitely)

### Task 3: Connection History

**Goal:** Fetch the complete conversation history with known connections (no date limit).

After the trust network and historical fetch establish who the user talks to, this task searches for any remaining messages involving those people. Processes **one connection email per tick**.

**Steps (per tick):**
1. Read cursor from `onboarding_tasks.cursor` — a JSON list of already-processed emails
2. Get all connection emails via `get_connection_emails()`, find first unprocessed one
3. If none remaining → mark task done, emit `onboarding_complete`, return
4. Connect to IMAP
5. For each sync folder:
   - IMAP SEARCH: `OR FROM "email" TO "email"` (no date restriction)
   - Filter to UIDs not already in the database
   - Fetch in batches of 200 (same 3-round-trip pattern)
   - Insert messages into DB
6. Run `process_changes()` once for this connection
7. Add email to cursor's done list, persist cursor
8. Return (next tick picks the next connection)

**Batch size:** 200 messages (within each connection)
**Resumability:** Yes — JSON cursor in `onboarding_tasks.cursor` tracks completed emails; survives app restarts

---

## Recurring Sync Phase

Every worker tick runs two operations for all onboarded accounts, regardless of whether other accounts are still onboarding:

### Incremental Sync

Checks each synced folder for new messages above `highest_uid`.

**Steps per folder:**
1. Skip if `highest_uid` is 0 (never synced by onboarding)
2. IMAP UID SEARCH for `UID <highest_uid+1>:*`
3. Filter to genuinely new UIDs
4. Fetch using the 3-round-trip strategy
5. Insert messages, update `highest_uid`
6. Run `process_changes()` if any new messages found

**No batching** — fetches all new UIDs in one pass (typically a small number since the last tick was 15 seconds ago).

### Flag Resync

Refreshes IMAP flags (and Gmail labels) for all locally-cached messages to detect read/unread/starred/label changes made by other clients.

**Gmail accounts:**
1. Query DB for all local `(imap_uid, imap_flags, gmail_labels)` tuples
2. Fetch from IMAP: `UID FETCH <uid_list> (UID FLAGS X-GM-LABELS)`
3. Compare both flags and labels to stored values (JSON string comparison)
4. Batch-update changed flags and labels via `update_flags_and_labels_batch()`

**Non-Gmail accounts:**
1. Query DB for all local `(imap_uid, imap_flags)` pairs
2. Fetch from IMAP: `UID FETCH <uid_list> (UID FLAGS)`
3. Compare fetched flags to stored flags (JSON string comparison)
4. Batch-update changed flags via `update_flags_batch()`

**Both paths:** Conversations are rebuilt **once at the end** (after all folders are processed) if any changes were detected, avoiding redundant rebuilds.

**Batch size:** 500 messages per IMAP fetch
**Lightweight:** Only fetches flag/label data (a few bytes per message)

---

## IMAP Fetch Strategy

Historical fetch, connection history, and incremental sync all use the same 3-round-trip pattern to efficiently retrieve message data:

```
Round Trip 1: Envelopes + Structure
  FETCH (UID FLAGS ENVELOPE BODYSTRUCTURE RFC822.SIZE [X-GM-LABELS])
  → Parse sender, recipients, subject, date, flags, attachment detection
  → Identify which MIME parts contain text/plain or text/html bodies

Round Trip 2: Threading Headers (once per batch)
  FETCH (UID BODY.PEEK[HEADER.FIELDS (References)])
  → Extract References header for conversation threading
  → Update envelope with list of referenced message IDs

Round Trip 3: Body Content (grouped by MIME part path)
  FETCH (UID BODY.PEEK[<part_path>])
  → Decode body based on Content-Transfer-Encoding (base64, quoted-printable, 8bit)
  → Convert HTML to plain text via html2text if needed
  → Returns (uid, decoded_text, is_html) tuples
```

Round trip 2 runs once per batch using the full batch UID list. Round trip 3 groups messages by their MIME part path (e.g., all messages with text/plain at part `1` are fetched together, then those at `1.1`, etc.).

This strategy minimizes bandwidth: envelope data is small, References headers are tiny, and body fetches are targeted to specific MIME parts rather than downloading entire RFC822 messages.

### Body decoding

Bodies are decoded based on their Content-Transfer-Encoding:
- `base64` — Standard base64 decode (whitespace stripped)
- `quoted-printable` — RFC 2045 decode
- `7bit`, `8bit`, `binary` — Used as-is (raw bytes to UTF-8)

HTML bodies are converted to plain text using `html2text` and stored in `body_text`. The original HTML is stored in `body_html` for full message rendering.

---

## Message Processing Pipeline

After messages are fetched and inserted, `worker::process_changes()` runs four processing steps:

```
process_changes(app, pool, account_id)
  1. extract_entities()       → Update trust network from new sent messages
  2. classify_messages()      → Categorize each message
  3. distill_messages()       → Extract chat-style preview
  4. rebuild_conversations()  → Materialize conversation records
  5. emit event               → Notify frontend to refresh
```

### Entity Extraction (Inline Trust Network Update)

Before classification runs, new messages from the user are scanned to incrementally update the trust network. This ensures the trust network stays current as new sent messages arrive via incremental sync, not just during the onboarding trust network task.

**How it works:**
1. Query messages where `processed_at IS NULL` (not yet classified) and `from_address` matches a self email (user or alias)
2. Parse `to_addresses` and `cc_addresses` (JSON arrays already stored in the DB)
3. Normalize and deduplicate recipient emails, filtering out self addresses
4. Upsert as connection entities with additive `sent_count`

This must run **before** `classify_messages()` because classification sets `processed_at`, closing the "new message" window. No IMAP calls are needed — all data comes from the local database.

### Classification

Messages are classified into one of five categories using weighted signal aggregation:

| Category | Description |
|----------|-------------|
| `chat` | Human-to-human conversation |
| `newsletter` | Mailing lists, marketing, digests |
| `automated` | GitHub notifications, CI/CD, alerts |
| `transactional` | Receipts, confirmations, password resets |
| `unknown` | No clear signal (defaults to chat) |

The classifier evaluates signals across six tiers, each contributing a weighted vote:

**Tier 1 — RFC Headers** (highest confidence, weight 1.0-1.5)
- `Auto-Submitted`, `List-Id`, `List-Unsubscribe`, `Feedback-ID`, `Precedence`, `X-Mailer`

**Tier 2 — Sender Analysis** (weight 0.3-1.3)
- Known automated senders (e.g., `noreply@github.com`)
- Noreply patterns in the local part
- ESP domain detection (Mailchimp → newsletter, Postmark → transactional, etc.)

**Tier 3 — Trust Network** (weight 1.2-1.5)
- Connection (from sent scan) → strong chat signal
- Contact (from CardDAV) → chat signal

**Tier 4 — Subject Keywords** (weight 0.5-0.9)
- Transactional: receipt, invoice, password reset, shipment
- Automated: build failed, deploy, pull request, incident
- Newsletter: digest, weekly update, top stories

**Tier 5 — Body Content** (weight 0.5-0.6)
- Unsubscribe/opt-out language → newsletter
- "View in browser" → newsletter
- Order/tracking numbers → transactional

**Tier 6 — Threading** (weight 0.5-1.0)
- `In-Reply-To` present → chat
- Deep reference chains (3+) → strong chat signal

Signals are summed per category, and the highest-scoring category wins. Confidence is derived from the margin between the winner and runner-up.

### Distillation

Extracts a short chat-style preview from the message body:

1. Strip email signature (below `--` or `-- ` line)
2. Remove quoted text (lines starting with `>`)
3. Skip attribution lines (ending with `wrote:`)
4. Stop at forwarded message markers
5. Collapse whitespace, truncate to 200 characters with `…`

The result is stored in `distilled_text` for display in conversation lists.

### Conversation Materialization

Conversations are rebuilt from scratch for the account:

1. **Group by participants** — Messages are grouped by `conversation_id` (a deterministic hash of the sorted, normalized participant list excluding self)
2. **Classify conversations:**
   - Has `chat` messages AND a trusted sender → `connections`
   - Has `chat` messages only → `others`
   - Otherwise → `automated`
3. **Compute aggregates** — Unread count (via JSON-parsed `imap_flags`, checking for absence of `"Seen"`), total count, latest message preview, participant names
4. **Upsert** — Replace all conversations for the account

### Unread Counting

Unread status is determined by parsing `imap_flags` as a JSON array and checking for the `"Seen"` flag:

- **In Rust** (conversation rebuild): `serde_json::from_str::<Vec<String>>(&imap_flags)`, then `!flags.iter().any(|f| f == "Seen")`
- **In SQL** (cluster queries): `NOT EXISTS (SELECT 1 FROM json_each(imap_flags) WHERE value = 'Seen')`

### Email Normalization

All email addresses are normalized before participant key computation:

- Lowercase and trim whitespace
- Strip `+tag` subaddressing (`user+spam@gmail.com` → `user@gmail.com`)
- Strip dots from Gmail local parts (`b.rian@gmail.com` → `brian@gmail.com`)
- Non-Gmail dots are preserved (`b.rian@outlook.com` stays as-is)

This ensures the same person with different address variations maps to the same conversation.

---

## Database State

The sync engine persists all state in SQLite (`sync.db`) using WAL mode with an r2d2 connection pool (max 8 connections).

### Key tables for sync state

**`onboarding_tasks`** — Task queue for initial sync

| Column | Description |
|--------|-------------|
| account_id | Account being onboarded |
| task | Task name (`trust_network`, `historical_fetch`, `connection_history`) |
| status | `pending` or `done` |
| cursor | Optional checkpoint for resumable tasks |

**`folder_sync`** — Per-folder IMAP sync cursors

| Column | Description |
|--------|-------------|
| account_id, folder | Composite primary key |
| uid_validity | IMAP UID validity (for detecting folder resets) |
| highest_uid | Latest UID fetched (incremental sync cursor) |
| lowest_uid | Oldest UID fetched (historical backfill cursor) |
| sync_status | `pending` or `done` |
| last_sync | Timestamp for round-robin ordering |

**`messages`** — Cached IMAP messages with processing outputs

Key columns beyond standard email fields:
- `imap_uid`, `imap_folder` — IMAP location (unique with account_id)
- `imap_flags` — JSON array of IMAP flags (`["Seen", "Flagged"]`)
- `gmail_labels` — JSON array of Gmail labels (`["Important", "Inbox"]`), empty `[]` for non-Gmail
- `classification` — Processing output (`chat`, `newsletter`, etc.)
- `distilled_text` — Chat-style body preview
- `participant_key` — Sorted normalized participants (excluding self)
- `conversation_id` — Hash of participant_key
- `processed_at` — NULL until classified and distilled

Uniqueness constraints:
- `UNIQUE(account_id, imap_folder, imap_uid)` — per-folder UID uniqueness
- `UNIQUE(account_id, message_id) WHERE message_id != ''` — cross-folder Message-ID dedup

**`entities`** — Trust network

| Trust Level | Source | Description |
|-------------|--------|-------------|
| `user` | Onboarding | The account owner's email |
| `alias` | Manual | Alternate emails for the owner |
| `contact` | CardDAV | Imported contacts |
| `connection` | Sent scan | People the user has emailed |

**`conversations`** — Materialized view rebuilt after each sync

Computed from messages: participant names, classification (`connections`/`others`/`automated`), unread count, latest preview, user preferences (muted, pinned, important).

### Resumability

The engine is designed to survive app restarts at any point:

- **Onboarding tasks** are persisted with status and an optional `cursor` field. On restart, the worker picks up the first non-done task and resumes from its cursor.
- **Trust network** uses a UID cursor in `onboarding_tasks.cursor`. On restart, it continues scanning Sent messages from the last processed UID.
- **Historical fetch** uses `lowest_uid` and `highest_uid` cursors per folder in `folder_sync`. On restart, it continues from where it left off.
- **Connection history** uses a JSON cursor in `onboarding_tasks.cursor` tracking completed emails. On restart, it skips already-processed connections.
- **Message inserts** use `INSERT OR IGNORE` with unique constraints on both `(account_id, imap_folder, imap_uid)` and `(account_id, message_id)`, so duplicate fetches are harmless.
- **Folder sync round-robin** orders by `CASE WHEN last_sync IS NULL THEN 0 ELSE 1 END, last_sync ASC`, ensuring never-synced folders are prioritized and no folder is starved.

---

## Frontend Integration

The sync engine communicates with the frontend via Tauri events:

**`sync:status`** — Emitted during sync phases with a human-readable message (e.g., "Fetching INBOX...", "Expanding thread 5 with 3/10"). Cleared (empty string) when onboarding completes.

**`sync:conversations-updated`** — Emitted after `process_changes()` completes or after flag resync detects changes. Carries the account ID and count of affected conversations. The frontend listens in `src/tauri/events.ts` and triggers a DataContext refresh, causing the UI to re-render with updated conversations.

Commands like `sync_now` allow the frontend to trigger an immediate sync cycle by sending a wake signal on the mpsc channel.
