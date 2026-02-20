# Eddie Chat - Development Instructions

These instructions guide Claude Code when making changes to this codebase.

## Project Overview

Eddie Chat is a cross-platform email client built with:
- **Frontend**: React 19 + TypeScript + Tailwind CSS
- **Backend**: Rust + Tauri 2
- **Database**: SQLite (local cache)
- **Protocols**: IMAP/SMTP via `email-lib`

---

## Feature Documentation Maintenance

**CRITICAL: Always read and update [FEATURES.md](./FEATURES.md) when making changes.**

### When to Consult FEATURES.md

**Before making changes:**
- Read FEATURES.md to understand existing functionality
- Identify which features your changes will affect
- Understand the user-facing behavior you're modifying

**Examples:**
- Adding a new Tauri command? Check if it extends an existing feature
- Modifying sync logic? Review the "Email Synchronization" section
- Changing UI components? Review the relevant feature sections

### When to Update FEATURES.md

**You MUST update FEATURES.md when:**
- ✅ Adding new user-facing functionality
- ✅ Modifying existing feature behavior
- ✅ Adding new commands, events, or capabilities
- ✅ Changing what the application can do
- ✅ Adding new UI features or interactions
- ✅ Modifying data storage or privacy behavior

**You DON'T need to update FEATURES.md for:**
- ❌ Refactoring code without behavior changes
- ❌ Bug fixes that restore intended behavior (unless fixing adds new capability)
- ❌ Internal implementation changes that don't affect functionality
- ❌ Performance optimizations without new features
- ❌ Code organization or style changes

### How to Update FEATURES.md

**Focus on WHAT, not HOW:**
- Document what users can do, not how it's implemented
- Use user-centric language (e.g., "Automatically saves to Sent folder", not "Calls save_message command")
- Describe capabilities and behavior, not code structure

**Update Location:**
- Find the relevant section (e.g., "Message Composition" for compose features)
- Add new features under the appropriate heading
- Create new sections only for major new feature areas
- Keep the hierarchy logical: High-level → Feature Area → Specific Capabilities

**Update Format:**
```markdown
**Feature Name**
- List what the feature does
- Include key capabilities
- Note any important limitations or future enhancements
```

**Example - Adding a new message action:**
```markdown
### Message Actions (in FEATURES.md)

**Archive Messages** (newly added)
- Archive individual messages
- Archive entire conversations
- Moves to Archive folder (provider-specific)
- Keyboard shortcut support
- Undo archive action
```

### Verification Checklist

After making changes, verify:
- [ ] FEATURES.md accurately reflects the new/changed functionality
- [ ] Documentation describes WHAT users can do, not HOW it works
- [ ] Related features are updated if affected
- [ ] New capabilities are in the appropriate section
- [ ] User-facing language is clear and concise

---

## Frontend Architecture

The frontend uses **file-based routing** with TanStack Router and a shared context layer. Follow these rules strictly:

### Directory Structure

```
src/
├── main.tsx               # Entry point (providers + router)
├── router.tsx             # TanStack Router config (hash history)
├── routes/                # File-based routing (TanStack Router)
│   ├── __root.tsx         # Root layout
│   ├── login.tsx          # Login screen
│   ├── _app.tsx           # Auth guard (beforeLoad)
│   ├── _app/_tabs.tsx     # Tab layout (header, tabs, account drawer)
│   ├── _app/_tabs/        # Tab routes (points, circles, lines)
│   ├── _app/settings.tsx  # Settings screen
│   ├── _app/conversation.$id.tsx  # Conversation detail
│   ├── _app/cluster.$id.tsx       # Cluster detail
│   └── _app/skills.*.tsx  # Skills routes (hub, studio, community)
├── skills/                # Standalone skill components
├── shared/                # Reusable utilities, components & context
│   ├── components/        # Generic UI components (Icons, etc.)
│   ├── context/           # Global state (AuthContext, DataContext, SearchContext)
│   └── lib/               # Utility functions (helpers)
└── tauri/                 # Tauri communication layer
    ├── commands.ts        # Type-safe command wrappers
    ├── events.ts          # Event listeners
    └── types.ts           # Frontend type definitions
```

### Rule 1: Route-Based Organization

Routes live in `src/routes/` following TanStack Router file-based conventions:
- Prefix `_` for layout routes (`_app.tsx`, `_tabs.tsx`)
- `$param` for dynamic segments (`conversation.$id.tsx`)
- Dot notation for nested paths (`skills.hub.tsx` → `/skills/hub`)
- `routeTree.gen.ts` is auto-generated (gitignored)

**Route files should be thin** — delegate to standalone components for complex UI:

```typescript
// src/routes/_app/skills.hub.tsx — thin route wrapper
function SkillsHubRoute() {
  const navigate = useNavigate();
  return <SkillsHub onNewSkill={() => navigate({ to: '/skills/studio' })} />;
}
```

### Rule 2: Centralized Tauri Communication

**NEVER call `invoke()` directly in components or routes.**

All Tauri communication must go through the `tauri/` layer:

```typescript
// WRONG - Direct invoke in component
import { invoke } from '@tauri-apps/api/core';
const data = await invoke('get_accounts');

// CORRECT - Use tauri layer
import { connectAccount } from '../tauri';
const data = await connectAccount(params);
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
// src/shared/context/index.ts
export { AuthProvider, useAuth } from './AuthContext';
export { DataProvider, useData } from './DataContext';
export { SearchProvider, useTabSearch } from './SearchContext';
```

```typescript
// src/skills/index.ts
export { SkillsHub } from './SkillsHub';
export { SkillStudio } from './SkillStudio';
export { CommunitySkills } from './CommunitySkills';
```

### Rule 4: Import Patterns

Use these import patterns consistently:

```typescript
// Tauri layer (commands, events, types)
import { connectAccount, fetchConversations, onSyncStatus } from '../tauri';
import type { Conversation, Cluster, SyncStatus } from '../tauri';

// Shared context
import { useAuth } from '../shared/context';
import { useData } from '../shared/context';

// Shared utilities and components
import { ComposeIcon } from '../shared/components';
import { relTime, avatarBg, displayName } from '../shared/lib';

// Skills (from route wrappers)
import { SkillsHub } from '../skills';
```

### Rule 5: Shared vs Skills vs Routes

**Put in `shared/`:**
- Generic UI components (Icons, etc.)
- Context providers (AuthContext, DataContext, SearchContext)
- Utility functions used across routes

**Put in `skills/`:**
- Standalone skill components (SkillsHub, SkillStudio, etc.)
- Skill types and mock data
- Skill-specific styling

**Put in `routes/`:**
- Route definitions with thin wrappers
- Navigation logic (passing `navigate()` callbacks to components)

### Rule 6: Type Safety

- All Tauri command return types must be defined in `src/tauri/types.ts`
- Types should mirror the Rust backend types
- Use explicit return types on all exported functions

```typescript
// src/tauri/commands.ts
export async function fetchConversations(accountId: string): Promise<Conversation[]> {
  return invoke<Conversation[]>('fetch_conversations', { accountId });
}
```

### Rule 7: State Management

Global state is managed through React Context in `src/shared/context/`:

- **AuthContext** — login credentials, account ID, authentication flow
- **DataContext** — conversations, clusters, sync status; auto-refreshes on Tauri events
- **SearchContext** — tab search string (shared across tab routes)

Routes and components access state via hooks: `useAuth()`, `useData()`, `useTabSearch()`.

### Adding New Routes

1. Create the route file in `src/routes/` following TanStack Router conventions
2. For complex UI, create a standalone component in the appropriate directory
3. Route file wraps the component, passing navigation callbacks
4. Add any Tauri commands to `src/tauri/commands.ts`
5. Add any new types to `src/tauri/types.ts`

### Frontend Mistakes to Avoid

1. **Don't** create new top-level directories in `src/`
2. **Don't** call `invoke()` outside of `src/tauri/commands.ts`
3. **Don't** put route-specific code in `shared/`
4. **Don't** forget barrel exports when adding new files
5. **Don't** use `any` types — define proper interfaces
6. **Don't** put heavy logic in route files — delegate to components

---

## Rust/Tauri Code Guidelines

### 1. Project Structure & Module Organization

Follow the established separation of concerns:

```
src-tauri/src/
├── commands/          # Tauri command handlers (thin wrappers)
│   ├── account.rs     # Account management (connect_account)
│   ├── conversations.rs # Conversation/cluster queries
│   ├── sync.rs        # Sync control (sync_now)
│   └── classify.rs    # Message classification
├── services/          # Business logic (Tauri-agnostic)
│   └── sync/          # Sync engine
│       ├── worker.rs  # Main tick loop (15s interval)
│       ├── helpers/   # Processing utilities
│       │   ├── email_normalization.rs
│       │   ├── message_builder.rs
│       │   ├── message_classification.rs
│       │   ├── message_distillation.rs
│       │   └── status_emit.rs
│       └── tasks/     # Onboarding & sync tasks
│           ├── trust_network.rs
│           ├── historical_fetch.rs
│           ├── connection_history.rs
│           └── incremental_sync.rs
├── adapters/          # External service adapters
│   ├── imap/          # IMAP protocol implementation
│   │   ├── connection.rs
│   │   ├── envelopes.rs
│   │   ├── folders.rs
│   │   ├── sent_scan.rs
│   │   └── historical.rs
│   └── sqlite/        # SQLite persistence
│       └── sync/      # Sync database
│           ├── db.rs          # Connection pool init
│           ├── db_schema.rs   # Schema definition
│           ├── messages.rs    # Message queries
│           ├── accounts.rs    # Account queries
│           ├── entities.rs    # Email entity queries
│           ├── conversations.rs # Conversation materialization
│           ├── folder_sync.rs # Folder sync tracking
│           └── onboarding_tasks.rs
├── error.rs           # EddieError enum
├── lib.rs             # Tauri app setup (state, worker spawn)
└── main.rs            # Binary entry point
```

**Rules:**
- Keep `commands/` as thin wrappers that delegate to `services/` and `adapters/`
- Business logic should live in `services/` and be Tauri-agnostic
- Protocol and persistence code belongs in `adapters/`
- Serializable types are defined in the adapter or command modules that use them

### 2. Command Design

Commands should be thin wrappers that:
1. Extract and validate arguments
2. Delegate to services or state managers
3. Map errors to `EddieError`
4. Return serializable responses

```rust
// ✅ Good: Thin wrapper delegating to adapters
#[tauri::command]
pub async fn fetch_conversations(
    pool: State<'_, Pool>,
    account_id: String,
) -> Result<Vec<Conversation>, EddieError> {
    let convos = adapters::sqlite::sync::conversations::get_conversations(&pool, &account_id)?;
    Ok(convos)
}

// ❌ Bad: Business logic in command
#[tauri::command]
pub async fn fetch_conversations(...) -> Result<(), String> {
    // 50+ lines of IMAP fetching, parsing, database writes, etc.
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

State is managed via Tauri's `app.manage()` in `lib.rs`:

```rust
// lib.rs — managed state
app.manage(pool);      // SQLite connection pool (for DB access)
app.manage(wake_tx);   // Channel to wake the sync worker

// Commands access via State extractor
#[tauri::command]
pub async fn sync_now(
    wake_tx: State<'_, mpsc::Sender<()>>,
) -> Result<(), EddieError> {
    let _ = wake_tx.send(()).await;
    Ok(())
}
```

**The sync worker** runs as an independent async task (spawned in `lib.rs`) that ticks every 15 seconds or when woken via the channel.

### 5. Async Operations

**Use `async` for all I/O operations:**

```rust
// ✅ Good: async command for I/O
#[tauri::command]
pub async fn connect_account(
    pool: State<'_, Pool>,
    wake_tx: State<'_, mpsc::Sender<()>>,
    params: ConnectAccountParams,
) -> Result<String, EddieError> {
    // Store account, then wake sync worker
    let account_id = adapters::sqlite::sync::accounts::insert(&pool, &params)?;
    let _ = wake_tx.send(()).await;
    Ok(account_id)
}
```

**Emit events for long-running operations** (from the sync worker via `services/sync/helpers/status_emit.rs`).

### 6. Database Operations

**The sync database is a cache, not source of truth.** All database access goes through `adapters/sqlite/sync/`:

```rust
// Pattern: Store messages from IMAP into local cache
db::messages::upsert_message(&pool, &message)?;

// Query materialized conversations
db::conversations::get_conversations(&pool, &account_id)?;
```

**Key database modules:**
- `db.rs` — connection pool initialization (r2d2 + rusqlite)
- `db_schema.rs` — schema definition and migrations
- `messages.rs` — message CRUD and classification updates
- `conversations.rs` — conversation materialization and cluster queries
- `folder_sync.rs` — IMAP UID/modseq tracking per folder
- `onboarding_tasks.rs` — initial sync task queue

### 7. Event Emission

**Emit events to update the frontend:**

```rust
// Emit sync events
self.emit_event(SyncEvent::ConversationsUpdated {
    conversation_ids: affected_ids,
});

// Events emitted via services/sync/helpers/status_emit.rs
// Frontend listens via src/tauri/events.ts
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
- [ ] Business logic is in `services/`, not in commands
- [ ] Protocol code is in `adapters/imap/`, DB code in `adapters/sqlite/`
- [ ] Returns `Result<T, EddieError>`, not `Result<T, String>`
- [ ] No `.unwrap()` or `.expect()` in command handlers
- [ ] Async operations use `async/await`
- [ ] Long operations emit progress events
- [ ] Database operations go through `adapters/sqlite/sync/`

---

## Common Patterns

### Adding a New Command

1. **Define in `commands/<domain>.rs`:**
   ```rust
   #[tauri::command]
   pub async fn new_command(
       pool: State<'_, Pool>,
       param: String,
   ) -> Result<ResponseType, EddieError> {
       // Thin wrapper — delegate to adapters/services
   }
   ```

2. **Register in `lib.rs`:**
   ```rust
   .invoke_handler(tauri::generate_handler![
       // ...existing commands
       commands::new_command,
   ])
   ```

3. **Add frontend wrapper in `src/tauri/commands.ts`:**
   ```typescript
   export async function newCommand(param: string): Promise<ResponseType> {
     return invoke<ResponseType>('new_command', { param });
   }
   ```

4. **Add types in `src/tauri/types.ts` if needed**

### Adding a New Sync Task

1. **Create task in `services/sync/tasks/`:**
   ```rust
   pub async fn run(pool: &Pool, account_id: &str) -> Result<(), EddieError> {
       // Task logic using adapters
   }
   ```

2. **Wire into worker in `services/sync/worker.rs`**

3. **Add event emission via `services/sync/helpers/status_emit.rs`**

4. **Add listener in `src/tauri/events.ts` if the frontend needs to react**

### Adding a New IMAP Adapter Function

1. **Add function in `adapters/imap/<module>.rs`**
2. **Call from `services/sync/tasks/` or `commands/`**
3. **Store results via `adapters/sqlite/sync/`**

---

## File Naming Conventions

### Frontend
- Components: PascalCase (`SkillsHub.tsx`, `CommunitySkills.tsx`)
- Routes: TanStack Router conventions (`_app.tsx`, `_tabs.tsx`, `conversation.$id.tsx`, `skills.hub.tsx`)
- Hooks: camelCase with `use` prefix (`useConversations.ts`)
- Utilities: camelCase (`helpers.ts`, `commands.ts`)
- Types: PascalCase for types/interfaces, camelCase for type files

### Backend
- Modules: snake_case (`sync_manager.rs`)
- Types: PascalCase (`SyncManager`, `EddieError`)
- Functions: snake_case (`get_cached_conversations`)
