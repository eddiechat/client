# Eddie Chat - Tauri Backend

This directory contains the Rust backend for the Eddie Chat email client, built with [Tauri](https://tauri.app/).

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Module Structure](#module-structure)
- [Sync Engine](#sync-engine)
  - [Core Concepts](#core-concepts)
  - [Data Flow](#data-flow)
  - [Database Integration](#database-integration)
  - [IMAP Operations](#imap-operations)
  - [UI Updates](#ui-updates)
- [Error Handling](#error-handling)
- [Development](#development)

---

## Architecture Overview

The backend follows a **separation of concerns** pattern:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Frontend (Vue/TypeScript)                      │
├─────────────────────────────────────────────────────────────────────────┤
│                              Tauri IPC Bridge                            │
├─────────────────────────────────────────────────────────────────────────┤
│  commands/          │  state/           │  services/                     │
│  (thin wrappers)    │  (app state)      │  (business logic)              │
├─────────────────────────────────────────────────────────────────────────┤
│  sync/              │  backend/         │  types/                        │
│  (sync engine)      │  (IMAP/SMTP)      │  (shared types)                │
├─────────────────────────────────────────────────────────────────────────┤
│                           SQLite Database                                │
└─────────────────────────────────────────────────────────────────────────┘
```

**Key Design Principle**: The UI renders from the SQLite cache, not directly from IMAP. This enables offline support and fast UI rendering.

---

## Module Structure

```
src/
├── lib.rs                 # Tauri app setup and command registration
├── main.rs                # Entry point (delegates to lib.rs)
│
├── commands/              # Tauri command handlers (thin wrappers)
│   ├── mod.rs             # Module exports
│   ├── accounts.rs        # Account listing/management
│   ├── config.rs          # Configuration management
│   ├── discovery.rs       # Email autodiscovery & OAuth2
│   ├── envelopes.rs       # Email envelope listing (deprecated)
│   ├── flags.rs           # Message flag operations
│   ├── folders.rs         # Folder/mailbox operations
│   ├── messages.rs        # Message read/send/delete
│   ├── conversations.rs   # Conversation listing (deprecated)
│   └── sync.rs            # Sync engine commands (recommended)
│
├── state/                 # Application state management
│   ├── mod.rs
│   ├── sync_manager.rs    # Manages sync engines per account
│   └── oauth_state.rs     # OAuth2 flow management
│
├── services/              # Business logic (Tauri-agnostic)
│   ├── mod.rs
│   ├── helpers.rs         # Shared utilities
│   ├── account_service.rs # Account creation/deletion
│   └── message_service.rs # MIME message building
│
├── types/                 # Data structures and types
│   ├── mod.rs
│   ├── error.rs           # EddieError enum
│   ├── responses.rs       # DTOs for frontend
│   └── conversation.rs    # Conversation types
│
├── sync/                  # Sync engine (core)
│   ├── mod.rs
│   ├── engine.rs          # Main sync engine
│   ├── db.rs              # SQLite database layer
│   ├── action_queue.rs    # Offline action queue
│   ├── conversation.rs    # Conversation grouping
│   ├── classifier.rs      # Message classification
│   ├── capability.rs      # Server capability detection
│   └── idle.rs            # IDLE/polling monitoring
│
├── backend/               # Email protocol implementation
│   └── mod.rs             # IMAP/SMTP via email-lib
│
├── config/                # Configuration management
│   └── mod.rs             # TOML config parsing
│
├── credentials/           # Secure credential storage
│   └── mod.rs             # System keyring integration
│
├── oauth/                 # OAuth2 implementation
│   └── mod.rs             # Google, Microsoft, etc.
│
└── autodiscovery/         # Email provider auto-configuration
    ├── mod.rs             # Discovery pipeline
    ├── providers.rs       # Known provider database
    ├── autoconfig.rs      # Mozilla autoconfig
    ├── dns.rs             # DNS SRV/MX lookup
    └── probe.rs           # Server probing
```

---

## Sync Engine

The sync engine (`sync/`) is the core component that maintains a local SQLite cache synchronized with IMAP servers.

### Core Concepts

1. **Server is Source of Truth**: The local database is a cache. Server state wins all conflicts.

2. **Offline-First with Action Queue**: User actions are queued locally and replayed on reconnect.

3. **UI Reads from Cache**: The frontend renders from SQLite, not IMAP, for instant responsiveness.

4. **Event-Driven Updates**: Changes are pushed to the UI via Tauri events.

### Data Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              USER ACTION                                  │
│                    (mark as read, delete, send, etc.)                    │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                           TAURI COMMAND                                   │
│            (commands/sync.rs, commands/messages.rs, etc.)                │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
┌────────────────────────────┐    ┌────────────────────────────────────────┐
│     UPDATE LOCAL CACHE     │    │         QUEUE ACTION (if offline)       │
│        (SQLite)            │    │         (action_queue.rs)               │
└────────────────────────────┘    └────────────────────────────────────────┘
                    │                               │
                    ▼                               │
┌────────────────────────────┐                     │
│      EMIT UI EVENT         │                     │
│   (sync-event channel)     │                     │
└────────────────────────────┘                     │
                    │                               │
                    ▼                               ▼
┌────────────────────────────┐    ┌────────────────────────────────────────┐
│      FRONTEND UPDATES      │    │        EXECUTE ON IMAP SERVER          │
│    (Vue reactive state)    │    │     (on reconnect or immediately)      │
└────────────────────────────┘    └────────────────────────────────────────┘
```

### Database Integration

The sync engine uses SQLite with connection pooling (`r2d2`) for all local storage.

#### Database Schema

| Table | Purpose |
|-------|---------|
| `messages` | Cached email envelopes and metadata |
| `message_bodies` | Full message content (fetched on-demand) |
| `conversations` | Conversation grouping by participants |
| `conversation_messages` | Message-to-conversation mapping |
| `actions` | Queued offline actions |
| `folder_sync_state` | Sync progress per folder (UIDVALIDITY, etc.) |
| `message_classifications` | Auto-categorization results |
| `connection_configs` | Account configurations |

#### Key Database Operations

```rust
// Upserting a message (sync/db.rs)
db.upsert_message(&cached_message)?;

// Getting conversations (sync/db.rs)
db.get_conversations(&account_id, include_hidden)?;

// Updating flags (sync/db.rs)
db.add_message_flags(&account_id, folder, uid, &["\\Seen"])?;

// Queueing an action (sync/action_queue.rs)
action_queue.queue(&account_id, ActionType::AddFlags { ... })?;
```

### IMAP Operations

#### Events that Trigger IMAP Operations

| Event | IMAP Operation | Description |
|-------|----------------|-------------|
| `init_sync_engine` | Full sync | Fetches all messages and rebuilds cache |
| `sync_folder` | Folder sync | Incremental sync of a single folder |
| `fetch_message_body` | FETCH BODY | Downloads message content on-demand |
| `mark_as_read/unread` | STORE +FLAGS | Queued and executed on server |
| `delete_messages` | STORE +FLAGS, EXPUNGE | Marks deleted, then expunges |
| `move_messages` | COPY, STORE +FLAGS, EXPUNGE | Move via IMAP |
| `send_message` | SMTP SEND | Sends via SMTP, saves to Sent |
| Monitor notification | Quick sync | Re-syncs when changes detected |
| Action queue replay | Various | Executes queued offline actions |

#### Action Queue (Offline Support)

The action queue (`sync/action_queue.rs`) persists user actions to SQLite and replays them on reconnect:

```rust
pub enum ActionType {
    AddFlags { folder, uids, flags },    // Uses +FLAGS (additive)
    RemoveFlags { folder, uids, flags }, // Uses -FLAGS (subtractive)
    Delete { folder, uids },              // Mark \Deleted, EXPUNGE
    Move { source_folder, target_folder, uids },
    Copy { source_folder, target_folder, uids },
    Send { raw_message, save_to_sent },
    Save { folder, raw_message },
}
```

**Conflict Resolution**: Uses additive flag operations (`+FLAGS`/`-FLAGS`) instead of overwriting, which merges cleanly with server-side changes.

#### Monitoring (IDLE/Polling)

The monitor (`sync/idle.rs`) detects mailbox changes:

```rust
pub enum ChangeNotification {
    NewMessages { folder },      // New mail arrived
    MessagesExpunged { folder }, // Messages deleted externally
    FlagsChanged { folder },     // Flags changed externally
    FolderChanged { folder },    // General change detected
    PollTrigger,                 // Periodic poll timer
    ConnectionLost { error },    // Need to reconnect
    Shutdown,                    // Monitor stopping
}
```

- **Desktop**: Uses IMAP IDLE for push notifications (when supported)
- **Mobile/Fallback**: Polls at configurable intervals (default: 60s)

### UI Updates

#### Tauri Event Channel

The sync engine emits events to the frontend via Tauri's event system:

```rust
// Event emission (sync/engine.rs)
fn emit_event(&self, event: SyncEvent) {
    if let Some(handle) = &self.app_handle {
        handle.emit("sync-event", &event)?;
    }
}
```

#### Events that Update the UI

| Event | Payload | UI Action |
|-------|---------|-----------|
| `StatusChanged` | `SyncStatus` | Updates sync indicator, progress bar |
| `NewMessages` | `{ folder, count }` | Shows notification, refreshes list |
| `MessagesDeleted` | `{ folder, uids }` | Removes messages from UI |
| `FlagsChanged` | `{ folder, uids }` | Updates read/flagged indicators |
| `ConversationsUpdated` | `{ conversation_ids }` | Refreshes conversation list |
| `Error` | `{ message }` | Shows error toast |
| `SyncComplete` | - | Hides progress, enables actions |

#### Frontend Integration (TypeScript)

```typescript
// Listening for sync events
import { listen } from '@tauri-apps/api/event';

listen<SyncEvent>('sync-event', (event) => {
  switch (event.payload.type) {
    case 'StatusChanged':
      updateSyncStatus(event.payload.data);
      break;
    case 'ConversationsUpdated':
      refreshConversations(event.payload.data.conversation_ids);
      break;
    case 'NewMessages':
      showNotification(event.payload.data.count);
      break;
    // ...
  }
});
```

#### Response Types (types/responses.rs)

DTOs for frontend consumption:

```rust
pub struct SyncStatusResponse {
    pub state: String,           // "idle", "syncing", "error"
    pub account_id: String,
    pub current_folder: Option<String>,
    pub progress_current: Option<u32>,
    pub progress_total: Option<u32>,
    pub progress_message: Option<String>,
    pub last_sync: Option<String>,
    pub error: Option<String>,
    pub is_online: bool,
    pub pending_actions: u32,
    pub monitor_mode: Option<String>,
}

pub struct ConversationResponse {
    pub id: i64,
    pub participants: Vec<ParticipantInfo>,
    pub last_message_date: Option<String>,
    pub last_message_preview: Option<String>,
    pub message_count: u32,
    pub unread_count: u32,
    pub is_outgoing: bool,
}

pub struct CachedChatMessageResponse {
    pub id: i64,
    pub uid: u32,
    pub from_address: String,
    pub subject: Option<String>,
    pub date: Option<String>,
    pub flags: Vec<String>,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub body_cached: bool,
}
```

---

## Error Handling

All commands return `Result<T, EddieError>` with serializable errors:

```rust
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum EddieError {
    Config(String),
    AccountNotFound(String),
    NoActiveAccount,
    FolderNotFound(String),
    MessageNotFound(String),
    Backend(String),
    Auth(String),
    Network(String),
    Database(String),
    Credential(String),
    OAuth(String),
    InvalidInput(String),
    Other(String),
}
```

Frontend receives errors as JSON:
```json
{ "type": "AccountNotFound", "message": "user@example.com" }
```

---

## Development

### Building

```bash
# Development build
cargo tauri dev

# Production build
cargo tauri build
```

### Running Tests

```bash
cd src-tauri
cargo test
```

### Debugging

Enable debug logging:
```bash
RUST_LOG=eddie_chat_lib=debug cargo tauri dev
```

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `tauri` | Desktop framework |
| `email-lib` | IMAP/SMTP protocol (Pimalaya) |
| `rusqlite` + `r2d2` | SQLite with connection pooling |
| `tokio` | Async runtime |
| `serde` | Serialization |
| `tracing` | Structured logging |
| `keyring` | Secure credential storage |
| `oauth2` | OAuth2 protocol |

---

## Quick Reference

### Adding a New Command

1. Add function in appropriate `commands/*.rs` file
2. Return `Result<T, EddieError>`
3. Use services for business logic
4. Register in `lib.rs` `invoke_handler`

### Adding a New Sync Event

1. Add variant to `SyncEvent` enum in `sync/engine.rs`
2. Emit via `self.emit_event(SyncEvent::NewVariant { ... })`
3. Handle in frontend event listener

### Offline Action Flow

1. User action → Command handler
2. Update local cache (SQLite)
3. Queue action (`action_queue.queue()`)
4. Emit UI event
5. On reconnect: `action_queue.replay_pending()`
