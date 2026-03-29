# Eddie Chat - Database Diagram

Eddie Chat uses **two SQLite databases**: a **Config Database** for account settings and an **Account Sync Database** (one per account) for cached email data.

---

## Config Database (`config.db`)

Stores global application settings and account connection configurations.

```
┌─────────────────────────────────────────────┐
│              connection_configs              │
├─────────────────────────────────────────────┤
│ PK  account_id        TEXT                  │
│     active             INTEGER DEFAULT 0    │
│     email              TEXT NOT NULL         │
│     display_name       TEXT                  │
│     aliases            TEXT                  │
│     imap_config        TEXT (JSON)           │
│     smtp_config        TEXT (JSON)           │
│     encrypted_password TEXT                  │
│     created_at         TEXT NOT NULL         │
│     updated_at         TEXT NOT NULL         │
├─────────────────────────────────────────────┤
│ IDX idx_connection_configs_active (active)   │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│                app_settings                 │
├─────────────────────────────────────────────┤
│ PK  key        TEXT                         │
│     value      TEXT NOT NULL                │
│     updated_at TEXT NOT NULL                │
└─────────────────────────────────────────────┘
```

> `connection_configs` and `app_settings` are independent tables with no foreign key relationships.

---

## Sync Database (per account)

Caches email data locally. One database instance per configured account.

### Entity-Relationship Diagram

```
┌──────────────────────┐       ┌─────────────────────────────┐
│   folder_sync_state  │       │        sync_progress        │
├──────────────────────┤       ├─────────────────────────────┤
│ PK account_id  TEXT  │       │ PK account_id     TEXT      │
│ PK folder_name TEXT  │       │ PK folder_name    TEXT      │
│    uidvalidity  INT  │       │    phase           TEXT     │
│    highestmodseq INT │       │    total_messages  INT      │
│    last_seen_uid INT │       │    synced_messages INT      │
│    last_sync_ts TEXT │       │    oldest_synced   TEXT     │
│    sync_in_progress  │       │    last_batch_uid  INT      │
│             INT      │       │    started_at      TEXT     │
└──────────────────────┘       │    updated_at      TEXT     │
                               └─────────────────────────────┘

┌──────────────────────┐
│ server_capabilities  │
├──────────────────────┤
│ PK account_id  TEXT  │
│    capabilities TEXT  │
│    supports_qresync  │
│              INT     │
│    supports_condstore│
│              INT     │
│    supports_idle INT │
│    detected_at  TEXT │
└──────────────────────┘


┌───────────────────────────────┐
│           messages            │
├───────────────────────────────┤
│ PK id              INT (AI)  │
│    account_id      TEXT      │
│    folder_name     TEXT      │
│    uid             INT       │
│    message_id      TEXT      │
│    in_reply_to     TEXT      │
│    references_header TEXT    │
│    from_address    TEXT      │
│    from_name       TEXT      │
│    to_addresses    TEXT(JSON)│
│    cc_addresses    TEXT(JSON)│
│    subject         TEXT      │
│    date            TEXT      │
│    flags           TEXT(JSON)│
│    has_attachment   INT      │
│    body_cached     INT       │
│    text_body       TEXT      │
│    html_body       TEXT      │
│    raw_size        INT       │
│    created_at      TEXT      │
│    updated_at      TEXT      │
├───────────────────────────────┤
│ UQ (account_id, folder, uid) │
└──────────┬──────────┬────────┘
           │          │
           │ 1        │ 1
           │          │
           │ N        │ 1
┌──────────┴──────┐   │   ┌───────────────────────────┐
│conversation_msgs│   │   │  message_classifications  │
├─────────────────┤   │   ├───────────────────────────┤
│PK conversation  │   │   │ PK message_id   INT ──────┤
│   _id    INT ───┤─┐ │   │    classification TEXT     │
│PK message       │ │ │   │    confidence     REAL     │
│   _id    INT ───┤─┼─┘   │    is_hidden_from │       │
├─────────────────┤ │     │       _chat       INT     │
│FK conversation  │ │     │    classified_at  TEXT     │
│   _id→          │ │     └───────────────────────────┘
│  conversations  │ │
│  (id) CASCADE   │ │
│FK message_id→   │ │
│  messages(id)   │ │
│  CASCADE        │ │
└─────────────────┘ │
           ┌────────┘
           │ N
           │
           │ 1
┌──────────┴────────────────────┐
│         conversations         │
├───────────────────────────────┤
│ PK id               INT (AI) │
│    account_id        TEXT     │
│    participant_key   TEXT     │
│    participants      TEXT(JSON)│
│    last_message_date TEXT     │
│    last_message_preview TEXT  │
│    last_message_from TEXT     │
│    message_count     INT      │
│    unread_count      INT      │
│    is_outgoing       INT      │
│    classification    TEXT     │
│    created_at        TEXT     │
│    updated_at        TEXT     │
├───────────────────────────────┤
│ UQ (account_id,               │
│     participant_key)          │
└───────────────────────────────┘


┌───────────────────────────────┐      ┌───────────────────────────────┐
│         action_queue          │      │           entities            │
├───────────────────────────────┤      ├───────────────────────────────┤
│ PK id           INT (AI)     │      │ PK id             INT (AI)   │
│    account_id   TEXT         │      │    account_id     TEXT       │
│    action_type  TEXT         │      │    email          TEXT       │
│    folder_name  TEXT         │      │    name           TEXT       │
│    uid          INT          │      │    is_connection  INT        │
│    payload      TEXT (JSON)  │      │    latest_contact TEXT       │
│    created_at   TEXT         │      │    contact_count  INT        │
│    retry_count  INT          │      │    created_at     TEXT       │
│    last_error   TEXT         │      │    updated_at     TEXT       │
│    status       TEXT         │      ├───────────────────────────────┤
└───────────────────────────────┘      │ UQ (account_id, email)       │
                                       └───────────────────────────────┘
```

---

## Relationships & Cardinalities

### Foreign Key Relationships

```
messages (1) ──────< (N) conversation_messages (N) >────── (1) conversations
                          (many-to-many join table)

messages (1) ──────────── (0..1) message_classifications
                          (one-to-one, optional)
```

| Parent Table     | Child Table               | Cardinality | FK Column          | On Delete |
|------------------|---------------------------|-------------|--------------------|-----------|
| `conversations`  | `conversation_messages`   | 1 : N       | `conversation_id`  | CASCADE   |
| `messages`       | `conversation_messages`   | 1 : N       | `message_id`       | CASCADE   |
| `messages`       | `message_classifications` | 1 : 0..1    | `message_id`       | CASCADE   |

### Logical Relationships (no FK constraints, linked by `account_id`)

| Table A              | Table B             | Relationship | Join Column    |
|----------------------|---------------------|--------------|----------------|
| `messages`           | `folder_sync_state` | N : 1        | `account_id`, `folder_name` |
| `conversations`      | `messages`          | N : N        | via `conversation_messages` |
| `action_queue`       | `messages`          | N : 0..1     | `account_id`, `folder_name`, `uid` |
| `sync_progress`      | `folder_sync_state` | 1 : 1        | `account_id`, `folder_name` |
| `server_capabilities`| `connection_configs`| 1 : 1        | `account_id` (across databases) |
| `entities`           | `messages`          | N : N        | `account_id`, `email` ↔ `from_address` |

---

## Table Summary

| # | Database | Table                    | Primary Key                      | Row Description                  |
|---|----------|--------------------------|----------------------------------|----------------------------------|
| 1 | Config   | `connection_configs`     | `account_id`                     | Email account connection config  |
| 2 | Config   | `app_settings`           | `key`                            | Application setting key-value    |
| 3 | Sync     | `folder_sync_state`      | `(account_id, folder_name)`      | Sync state per folder            |
| 4 | Sync     | `messages`               | `id` (autoincrement)             | Cached email message             |
| 5 | Sync     | `conversations`          | `id` (autoincrement)             | Participant-grouped thread       |
| 6 | Sync     | `conversation_messages`  | `(conversation_id, message_id)`  | Message ↔ Conversation mapping   |
| 7 | Sync     | `action_queue`           | `id` (autoincrement)             | Queued offline action            |
| 8 | Sync     | `message_classifications`| `message_id`                     | ML classification result         |
| 9 | Sync     | `sync_progress`          | `(account_id, folder_name)`      | Initial sync progress tracker    |
|10 | Sync     | `server_capabilities`    | `account_id`                     | Cached IMAP server capabilities  |
|11 | Sync     | `entities`               | `id` (autoincrement)             | Contact/participant record       |

---

## Index Summary

| Table                    | Index Name                            | Columns                          |
|--------------------------|---------------------------------------|----------------------------------|
| `connection_configs`     | `idx_connection_configs_active`       | `active`                         |
| `messages`               | `idx_messages_account_folder`         | `account_id, folder_name`        |
| `messages`               | `idx_messages_date`                   | `date DESC`                      |
| `messages`               | `idx_messages_message_id`             | `message_id`                     |
| `messages`               | `idx_messages_from`                   | `from_address`                   |
| `conversations`          | `idx_conversations_account`           | `account_id`                     |
| `conversations`          | `idx_conversations_last_date`         | `last_message_date DESC`         |
| `conversations`          | `idx_conversations_participant_key`   | `participant_key`                |
| `conversations`          | `idx_conversations_classification`    | `classification`                 |
| `conversation_messages`  | `idx_conv_msg_conversation`           | `conversation_id`                |
| `conversation_messages`  | `idx_conv_msg_message`               | `message_id`                     |
| `action_queue`           | `idx_action_queue_status`             | `status, created_at`             |
| `action_queue`           | `idx_action_queue_account`            | `account_id`                     |
| `entities`               | `idx_entities_account`                | `account_id`                     |
| `entities`               | `idx_entities_email`                  | `account_id, email`              |
| `entities`               | `idx_entities_connection`             | `account_id, is_connection`      |
| `entities`               | `idx_entities_latest_contact`         | `account_id, latest_contact DESC`|

---

## Database Configuration

Both databases use the following SQLite PRAGMA settings:

| Setting              | Value     | Purpose                                    |
|----------------------|-----------|--------------------------------------------|
| `foreign_keys`       | `ON`      | Enforce foreign key constraints            |
| `journal_mode`       | `WAL`     | Write-Ahead Logging for concurrency        |
| `synchronous`        | `NORMAL`  | Balance durability and performance         |
| `cache_size`         | `-64000`  | 64 MB cache (sync DB only)                |
