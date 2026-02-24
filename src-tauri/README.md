# Eddie Chat - Tauri Backend

This directory contains the Rust backend for the Eddie Chat email client, built with [Tauri v2](https://tauri.app/).

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Module Structure](#module-structure)
- [Sync Engine](#sync-engine)
- [Error Handling](#error-handling)
- [Development](#development)

---

## Architecture Overview

The backend follows a **separation of concerns** pattern with four top-level modules:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Frontend (React/TypeScript)                       │
├─────────────────────────────────────────────────────────────────────────┤
│                              Tauri IPC Bridge                            │
├─────────────────────────────────────────────────────────────────────────┤
│  commands/          │  services/          │  autodiscovery/              │
│  (thin wrappers)    │  (business logic)   │  (provider detection)        │
├─────────────────────────────────────────────────────────────────────────┤
│  adapters/                                                               │
│  (imap protocol, sqlite persistence, ollama AI)                          │
├─────────────────────────────────────────────────────────────────────────┤
│                           SQLite Database                                │
└─────────────────────────────────────────────────────────────────────────┘
```

**Key Design Principles:**
- The UI renders from the SQLite cache, not directly from IMAP
- Commands are thin wrappers that delegate to services and adapters
- All errors use `EddieError`, never `Result<T, String>`
- The sync worker runs as an independent async task on a 15-second tick loop

---

## Module Structure

```
src/
├── lib.rs                 # Tauri app setup, state init, worker spawn
├── main.rs                # Binary entry point (delegates to lib.rs)
├── error.rs               # EddieError enum
│
├── commands/              # Tauri command handlers (thin wrappers)
│   ├── mod.rs             # Module exports
│   ├── account.rs         # Account connect/lookup
│   ├── conversations.rs   # Conversation & cluster queries
│   ├── sync.rs            # Sync control & onboarding status
│   ├── classify.rs        # Message reclassification
│   ├── discovery.rs       # Email autodiscovery
│   ├── skills.rs          # Skill CRUD
│   ├── settings.rs        # App settings & Ollama model listing
│   ├── ollama.rs          # Ollama LLM completion
│   └── app.rs             # App metadata (version)
│
├── services/              # Business logic (Tauri-agnostic)
│   ├── mod.rs
│   ├── sync/              # Sync engine
│   │   ├── worker.rs      # Main tick loop (15s interval)
│   │   ├── helpers/       # Processing utilities
│   │   │   ├── email_normalization.rs
│   │   │   ├── entity_extraction.rs
│   │   │   ├── message_builder.rs
│   │   │   ├── message_classification.rs
│   │   │   ├── message_distillation.rs
│   │   │   └── status_emit.rs
│   │   └── tasks/         # Onboarding & recurring tasks
│   │       ├── trust_network.rs
│   │       ├── historical_fetch.rs
│   │       ├── connection_history.rs
│   │       ├── incremental_sync.rs
│   │       ├── flag_resync.rs
│   │       └── skill_classify.rs
│   ├── ollama.rs          # Ollama model discovery & state
│   └── logger.rs          # Structured logging to DB
│
├── adapters/              # External service bridges
│   ├── imap/              # IMAP protocol (async-imap)
│   │   ├── connection.rs  # TCP + TLS (tokio-rustls) + LOGIN
│   │   ├── envelopes.rs   # Message envelope fetching
│   │   ├── folders.rs     # Folder discovery & classification
│   │   ├── historical.rs  # Historical message fetch
│   │   └── sent_scan.rs   # Sent folder scanning for trust network
│   ├── sqlite/            # SQLite persistence (rusqlite + r2d2)
│   │   └── sync/          # Sync database
│   │       ├── db.rs              # Connection pool initialization
│   │       ├── db_schema.rs       # Schema definition & migrations
│   │       ├── messages.rs        # Message CRUD
│   │       ├── conversations.rs   # Conversation materialization
│   │       ├── entities.rs        # Trust network (entities table)
│   │       ├── accounts.rs        # Account queries
│   │       ├── folder_sync.rs     # Per-folder IMAP sync cursors
│   │       ├── onboarding_tasks.rs# Onboarding task queue
│   │       ├── skills.rs          # Skill persistence
│   │       ├── skill_classify.rs  # Skill classification results
│   │       ├── settings.rs        # App settings (key-value)
│   │       └── line_groups.rs     # Line grouping
│   └── ollama/            # Ollama AI adapter
│       └── mod.rs         # HTTP calls to local Ollama
│
└── autodiscovery/         # Email provider auto-configuration
    ├── mod.rs             # Discovery pipeline
    ├── providers.rs       # Known provider database
    ├── autoconfig.rs      # Mozilla autoconfig XML
    ├── dns.rs             # DNS SRV/MX lookup
    └── probe.rs           # Server probing
```

---

## Sync Engine

The sync engine (`services/sync/`) maintains a local SQLite cache synchronized with IMAP servers. For detailed documentation, see [IMAP_SYNC.md](../IMAP_SYNC.md).

### Core Concepts

1. **Server is Source of Truth**: The local database is a cache. Server state wins all conflicts.
2. **UI Reads from Cache**: The frontend renders from SQLite, not IMAP, for instant responsiveness.
3. **Event-Driven Updates**: Changes are pushed to the UI via Tauri events.
4. **Tick-Based Worker**: A 15-second tick loop processes sync tasks; commands can wake it immediately via an mpsc channel.

### Sync Phases

**Onboarding** — When a new account is added, three sequential tasks build initial state:
1. `trust_network` — Scan Sent folder for contacts
2. `historical_fetch` — Fetch 12 months of messages
3. `connection_history` — Expand threads with known connections

**Recurring** — On every tick (even during onboarding), incremental sync and flag resync run for all onboarded accounts.

### UI Events

| Event | Purpose |
|-------|---------|
| `sync:status` | Progress messages during sync phases |
| `sync:conversations-updated` | Triggers frontend data refresh |
| `onboarding:complete` | Signals onboarding finished for an account |

### Frontend Integration

Events are consumed via the centralized `src/tauri/events.ts` layer:

```typescript
import { onSyncStatus, onConversationsUpdated } from '../tauri';

onSyncStatus((status) => {
  // Update progress indicator
});

onConversationsUpdated((data) => {
  // Refresh conversation list
});
```

---

## Error Handling

All commands return `Result<T, EddieError>`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum EddieError {
    Database(String),
    Backend(String),
    Config(String),
    InvalidInput(String),
    AccountNotFound(String),
    NoActiveAccount,
}
```

Errors are serialized as plain strings for the frontend.

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
| `tauri` | Desktop framework (v2) |
| `async-imap` | IMAP protocol |
| `mailparse` | Email message parsing |
| `imap-proto` | IMAP protocol types |
| `tokio-rustls` | TLS connections |
| `rusqlite` + `r2d2` | SQLite with connection pooling |
| `tokio` | Async runtime |
| `serde` | Serialization |
| `tracing` | Structured logging |
| `reqwest` | HTTP client (Ollama, autodiscovery) |
| `html2text` | HTML to plain text conversion |
| `sentry` | Error tracking |

---

## Quick Reference

### Adding a New Command

1. Add function in appropriate `commands/*.rs` file
2. Return `Result<T, EddieError>`
3. Delegate to services/adapters for business logic
4. Register in `lib.rs` `invoke_handler`
5. Add frontend wrapper in `src/tauri/commands.ts`
6. Add types in `src/tauri/types.ts` if needed

### Adding a New Sync Task

1. Create task in `services/sync/tasks/`
2. Wire into worker in `services/sync/worker.rs`
3. Add event emission via `services/sync/helpers/status_emit.rs`
4. Add listener in `src/tauri/events.ts` if the frontend needs to react
