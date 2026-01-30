# Action Queue Pattern

**Offline-first design** - Actions are queued immediately in SQLite and replayed when online:

1. **Queue actions via `queueSyncAction()`** from frontend ([api.ts](src/lib/api.ts))
2. Actions stored in SQLite with status `pending` → `processing` → `completed`/`failed`
3. On sync, `replay_pending()` executes all queued actions before fetching updates
4. Max 5 retries per action, with automatic optimization (merges similar flag operations)

```typescript
// Preferred: Queue the action, don't execute directly
await queueSyncAction("add_flags", folder, uids, ["\\Seen"], undefined, account);
```

## IMAP Concurrency

**Fresh connections per operation** - No connection pooling at the IMAP layer:

- Each IMAP operation creates a new `BackendBuilder` instance ([backend/mod.rs](src-tauri/src/backend/mod.rs))
- Operations are sequential, not parallel
- The `email-lib` crate handles underlying connection management

## Database Connection Pool

SQLite uses **r2d2 pooling with max 10 connections** ([db.rs:128-137](src-tauri/src/sync/db.rs#L128-L137)):

```rust
let pool = Pool::builder().max_size(10).build(manager)?;
```

## Core Design Principles

From [sync/mod.rs:1-11](src-tauri/src/sync/mod.rs#L1-L11):

1. **UI renders exclusively from SQLite** - never directly from IMAP responses
2. **Actions execute on IMAP/SMTP first** - then sync confirms the change
3. **Server wins all conflicts** - IMAP is the source of truth, SQLite is cache

## Sync Flow

```
User Action → queueSyncAction() → SQLite (pending)
                                      ↓
                              [When online]
                                      ↓
                              replay_pending() → Execute on IMAP
                                      ↓
                              sync_folder_from_imap() → Update SQLite
                                      ↓
                              Emit sync-event → Frontend updates
```

## Key Files

| Purpose | File |
|---------|------|
| Action Queue | [src-tauri/src/sync/action_queue.rs](src-tauri/src/sync/action_queue.rs) |
| Sync Engine | [src-tauri/src/sync/engine.rs](src-tauri/src/sync/engine.rs) |
| Database | [src-tauri/src/sync/db.rs](src-tauri/src/sync/db.rs) |
| Frontend Hook | [src/hooks/useSync.ts](src/hooks/useSync.ts) |

---

## Prompt Template

```
Implement [action name] following these steps:

1. Add ActionType variant in action_queue.rs with required payload
2. Add execute_action() match arm to call the backend method
3. Add Tauri command in commands/sync.rs that queues the action
4. Add frontend API function in api.ts calling the Tauri command
5. Call the API from the UI component, then refreshStatus()

Key rules:
- Queue actions, don't execute directly
- UI updates come from sync events, not action results
- Server is source of truth, SQLite is cache
```

### Example Usage

> "Implement a 'move message to folder' action following these steps:
> 1. Add ActionType variant in action_queue.rs with required payload
> 2. Add execute_action() match arm to call the backend method
> 3. Add Tauri command in commands/sync.rs that queues the action
> 4. Add frontend API function in api.ts calling the Tauri command
> 5. Call the API from the UI component, then refreshStatus()"

### Even Shorter Version

```
Implement [X] using the action queue pattern:
- ActionType variant → execute_action() handler → Tauri command → api.ts → UI
- Queue the action, don't execute directly
- UI updates via sync events only
```