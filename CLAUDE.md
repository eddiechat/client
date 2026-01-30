# Eddie Chat - Development Instructions

These instructions guide Claude Code when making changes to this codebase.

## Project Overview

Eddie Chat is a cross-platform email client built with:
- **Frontend**: React 19 + TypeScript + Tailwind CSS
- **Backend**: Rust + Tauri 2
- **Database**: SQLite (local cache)
- **Protocols**: IMAP/SMTP via `email-lib`

---

## Frontend Architecture

The frontend uses a **feature-based architecture**. Follow these rules strictly:

### Directory Structure

```
src/
├── features/           # Feature modules (domain-based)
│   ├── accounts/       # Account management
│   └── conversations/  # Email conversations
├── shared/             # Reusable utilities & components
├── tauri/              # Tauri communication layer
└── lib/                # Static data (emoji, etc.)
```

### Rule 1: Feature-Based Organization

**DO:** Group code by domain/feature, not by type.

```
src/features/accounts/
├── components/         # UI components for this feature
├── hooks/              # React hooks for this feature
├── context/            # Context providers (if needed)
├── utils.ts            # Feature-specific utilities
└── index.ts            # Barrel exports
```

**DON'T:** Create top-level `components/`, `hooks/`, or `types/` directories.

### Rule 2: Centralized Tauri Communication

**NEVER call `invoke()` directly in components or hooks.**

All Tauri communication must go through the `tauri/` layer:

```typescript
// WRONG - Direct invoke in component
import { invoke } from '@tauri-apps/api/core';
const data = await invoke('get_accounts');

// CORRECT - Use tauri layer
import { listAccounts } from '../tauri';
const data = await listAccounts();
```

**Adding new Tauri commands:**

1. Add the type-safe wrapper in `src/tauri/commands.ts`
2. Add any new types in `src/tauri/types.ts`
3. Export from `src/tauri/index.ts`

**Adding new Tauri events:**

1. Add the listener in `src/tauri/events.ts`
2. Export from `src/tauri/index.ts`

### Rule 3: Barrel Exports

Every directory with multiple files needs an `index.ts` barrel export:

```typescript
// src/features/accounts/components/index.ts
export { SidebarHeader } from './SidebarHeader';
export { AccountSetupWizard } from './AccountSetupWizard';
export { AccountConfigModal } from './AccountConfigModal';
```

```typescript
// src/features/accounts/index.ts
export * from './components';
export * from './hooks';
export * from './context';
```

### Rule 4: Import Patterns

Use these import patterns consistently:

```typescript
// Feature imports
import { useAccounts, AccountSetupWizard } from './features/accounts';
import { ConversationView, useConversations } from './features/conversations';

// Tauri layer (commands, events, types)
import { saveAccount, listAccounts, onSyncEvent } from './tauri';
import type { Account, SyncStatus, Conversation } from './tauri';

// Shared utilities and components
import { Avatar, LoadingSpinner, EmptyState } from './shared/components';
import { extractEmail, getAvatarColor, formatMessageTime } from './shared/lib';
```

### Rule 5: Shared vs Feature Code

**Put in `shared/`:**
- Generic UI components (Avatar, LoadingSpinner, EmptyState)
- Utility functions used by multiple features
- Common types used across features

**Put in `features/{name}/`:**
- Components specific to that feature
- Hooks that manage feature state
- Feature-specific utilities

### Rule 6: Type Safety

- All Tauri command return types must be defined in `src/tauri/types.ts`
- Types should mirror the Rust backend types
- Use explicit return types on all exported functions

```typescript
// src/tauri/commands.ts
export async function listAccounts(): Promise<Account[]> {
  return invoke<Account[]>('list_accounts');
}
```

### Rule 7: Hook Patterns

Hooks should return consistent shapes:

```typescript
interface UseAccountsResult {
  accounts: Account[];
  currentAccount: string | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useAccounts(): UseAccountsResult {
  // implementation
}
```

### Creating New Features

When adding a new feature:

1. Create the feature directory: `src/features/{feature-name}/`
2. Add subdirectories as needed: `components/`, `hooks/`, `context/`
3. Create barrel exports at each level
4. Add any Tauri commands to `src/tauri/commands.ts`
5. Add any new types to `src/tauri/types.ts`
6. Export the feature from `src/features/index.ts`

### Frontend Mistakes to Avoid

1. **Don't** create new top-level directories in `src/`
2. **Don't** call `invoke()` outside of `src/tauri/commands.ts`
3. **Don't** put feature-specific code in `shared/`
4. **Don't** forget barrel exports when adding new files
5. **Don't** use `any` types - define proper interfaces
6. **Don't** mix concerns - keep UI separate from data fetching

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

4. **Add frontend wrapper in `src/tauri/commands.ts`:**
   ```typescript
   export async function newCommand(param: string): Promise<ResponseType> {
     return invoke<ResponseType>('new_command', { param });
   }
   ```

5. **Add types in `src/tauri/types.ts` if needed**

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

3. **Add listener in `src/tauri/events.ts`:**
   ```typescript
   export async function onNewEventType(
     callback: (data: string) => void
   ): Promise<UnlistenFn> {
     return listen<SyncEventPayload>('sync-event', (event) => {
       if ('NewEventType' in event.payload) {
         callback(event.payload.NewEventType.data);
       }
     });
   }
   ```

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

---

## File Naming Conventions

### Frontend
- Components: PascalCase (`ConversationView.tsx`)
- Hooks: camelCase with `use` prefix (`useConversations.ts`)
- Utilities: camelCase (`utils.ts`)
- Types: PascalCase for types/interfaces, camelCase for type files

### Backend
- Modules: snake_case (`sync_manager.rs`)
- Types: PascalCase (`SyncManager`, `EddieError`)
- Functions: snake_case (`get_cached_conversations`)
