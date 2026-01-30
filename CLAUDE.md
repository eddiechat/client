# Eddie Chat - Development Instructions

These instructions guide Claude Code when making changes to this codebase.

## Project Overview

Eddie Chat is a cross-platform email client built with:
- **Frontend**: Vue 3 + TypeScript + Tailwind CSS
- **Backend**: Rust + Tauri 2
- **Database**: SQLite (local cache)
- **Protocols**: IMAP/SMTP via `email-lib`

---

## Rust/Tauri Code Guidelines

### 1. Project Structure & Module Organization

Follow the established separation of concerns:

```
src-tauri/src/
├── commands/     # Tauri command handlers (thin wrappers)
├── services/     # Business logic (Tauri-agnostic)
├── state/        # Application state management
├── types/        # Data structures and types
├── sync/         # Sync engine
├── backend/      # IMAP/SMTP protocol
└── ...
```

**Rules:**
- Keep `commands/` as thin wrappers that delegate to `services/`
- Business logic should live in `services/` and be Tauri-agnostic
- State types belong in `state/`, not scattered in command files
- Response DTOs for the frontend go in `types/responses.rs`

### 2. Command Design

Commands should be thin wrappers that:
1. Extract and validate arguments
2. Delegate to services or state managers
3. Map errors to `EddieError`
4. Return serializable responses

```rust
// ✅ Good: Thin wrapper
#[tauri::command]
pub async fn delete_messages(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    let account_id = resolve_account_id_string(account)?;
    let backend = backend::get_backend(Some(&account_id)).await?;
    backend.delete_messages(folder.as_deref(), &ids).await?;
    update_cache(&sync_manager, &account_id, &ids).await;
    Ok(())
}

// ❌ Bad: Business logic in command
#[tauri::command]
pub async fn delete_messages(...) -> Result<(), String> {
    // 50+ lines of MIME parsing, database queries, etc.
}
```

### 3. Error Handling

**Always use `EddieError`** - never return `Result<T, String>`:

```rust
// ✅ Good
pub async fn my_command() -> Result<Data, EddieError> {
    something().map_err(|e| EddieError::Backend(e.to_string()))?;
    Ok(data)
}

// ❌ Bad
pub async fn my_command() -> Result<Data, String> {
    something().map_err(|e| e.to_string())?;
}
```

**Use appropriate error variants:**
- `EddieError::Backend` - IMAP/SMTP errors
- `EddieError::Database` - SQLite errors
- `EddieError::Config` - Configuration errors
- `EddieError::Auth` - Authentication errors
- `EddieError::InvalidInput` - Bad user input
- `EddieError::AccountNotFound` - Missing account
- `EddieError::NoActiveAccount` - No account selected

### 4. State Management

**Use the `state/` module for managed state:**

```rust
// State lives in state/sync_manager.rs
pub struct SyncManager {
    engines: RwLock<HashMap<String, Arc<RwLock<SyncEngine>>>>,
    // ...
}

// Commands access via State extractor
#[tauri::command]
pub async fn init_sync_engine(
    manager: State<'_, SyncManager>,
    account: Option<String>,
) -> Result<SyncStatusResponse, EddieError> {
    manager.get_or_create(&account_id).await?;
}
```

**Rules:**
- Use `RwLock` for state that's read more than written
- Use `Mutex` for write-heavy state
- Wrap engines in `Arc<RwLock<T>>` for shared async access

### 5. Async Operations

**Use `async` for all I/O operations:**

```rust
// ✅ Good: async command for I/O
#[tauri::command]
pub async fn fetch_message_body(
    manager: State<'_, SyncManager>,
    message_id: i64,
) -> Result<CachedMessageResponse, EddieError> {
    let engine = manager.get_or_create(&account_id).await?;
    engine.read().await.fetch_message_body(message_id).await?
}
```

**Emit events for long-running operations:**

```rust
// Emit progress updates
self.emit_event(SyncEvent::StatusChanged(status.clone()));
```

### 6. Database Operations

**The sync database is a cache, not source of truth:**

```rust
// Pattern: Update cache, then sync to server
db.add_message_flags(&account_id, folder, uid, &flags)?;  // Local first
action_queue.queue(ActionType::AddFlags { ... })?;         // Queue for server
```

**Use the action queue for offline support:**

```rust
// Queue actions that will replay on reconnect
engine.queue_action(ActionType::Delete { folder, uids })?;
```

### 7. Event Emission

**Emit events to update the frontend:**

```rust
// Emit sync events
self.emit_event(SyncEvent::ConversationsUpdated {
    conversation_ids: affected_ids,
});

// Event types (sync/engine.rs)
pub enum SyncEvent {
    StatusChanged(SyncStatus),
    NewMessages { folder: String, count: u32 },
    MessagesDeleted { folder: String, uids: Vec<u32> },
    FlagsChanged { folder: String, uids: Vec<u32> },
    ConversationsUpdated { conversation_ids: Vec<i64> },
    Error { message: String },
    SyncComplete,
}
```

### 8. Type Design

**Use strong types over primitives:**

```rust
// ✅ Good: Dedicated response types
pub struct SyncStatusResponse {
    pub state: String,
    pub account_id: String,
    pub is_online: bool,
    // ...
}

// ❌ Bad: Raw tuples or deeply nested structures
fn get_status() -> (String, String, bool, Option<u32>, ...)
```

**Implement `From` traits for conversions:**

```rust
impl From<SyncStatus> for SyncStatusResponse {
    fn from(s: SyncStatus) -> Self {
        Self {
            state: format!("{:?}", s.state),
            account_id: s.account_id,
            // ...
        }
    }
}
```

### 9. Logging

**Use `tracing` macros:**

```rust
use tracing::{debug, info, warn, error};

info!("Starting sync for account: {}", account_id);
debug!("Fetched {} messages", count);
warn!("Could not fetch folder {}: {}", folder, e);
error!("Sync failed: {}", e);
```

### 10. Code Organization Checklist

When adding or modifying code, verify:

- [ ] Commands are thin wrappers (< 30 lines typically)
- [ ] Business logic is in `services/` or domain modules
- [ ] Returns `Result<T, EddieError>`, not `Result<T, String>`
- [ ] State types are in `state/` module
- [ ] Response DTOs are in `types/responses.rs`
- [ ] No `.unwrap()` or `.expect()` in command handlers
- [ ] Async operations use `async/await`
- [ ] Long operations emit progress events
- [ ] Database operations go through `sync/db.rs`
- [ ] Offline-capable actions use the action queue

---

## Frontend Guidelines

### Vue Components
- Use Composition API with `<script setup lang="ts">`
- Keep components focused and small
- Use Tailwind CSS for styling (mobile-first)

### Tauri Integration
```typescript
// Invoke commands
import { invoke } from '@tauri-apps/api/core';
const result = await invoke<ResponseType>('command_name', { args });

// Listen for events
import { listen } from '@tauri-apps/api/event';
listen<SyncEvent>('sync-event', (event) => {
    // Handle event
});
```

---

## Common Patterns

### Adding a New Command

1. **Define in `commands/<domain>.rs`:**
   ```rust
   #[tauri::command]
   pub async fn new_command(
       manager: State<'_, SyncManager>,
       param: String,
   ) -> Result<ResponseType, EddieError> {
       // Thin wrapper logic
   }
   ```

2. **Register in `lib.rs`:**
   ```rust
   .invoke_handler(tauri::generate_handler![
       // ...existing commands
       commands::new_command,
   ])
   ```

3. **Add response type if needed in `types/responses.rs`**

### Adding a New Sync Event

1. **Add variant in `sync/engine.rs`:**
   ```rust
   pub enum SyncEvent {
       // ...existing
       NewEventType { data: String },
   }
   ```

2. **Emit where appropriate:**
   ```rust
   self.emit_event(SyncEvent::NewEventType { data });
   ```

3. **Handle in frontend event listener**

### Adding Offline Support for an Action

1. **Add action type in `sync/action_queue.rs`:**
   ```rust
   pub enum ActionType {
       // ...existing
       NewAction { params },
   }
   ```

2. **Implement replay logic in `ActionQueue::execute_action()`**

3. **Queue from command:**
   ```rust
   engine.queue_action(ActionType::NewAction { params })?;
   ```
