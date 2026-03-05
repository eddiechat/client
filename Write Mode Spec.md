# Specification: Mutating Email Actions in Eddie

## Overview

Eddie implements an **optimistic, queue-backed mutation system** for email operations. The core principle is:

> **The UI always renders from SQLite. IMAP is the single source of truth. The server always wins.**

All write operations follow a three-phase flow:

1. **Optimistic local update** — apply the change to SQLite immediately so the UI reflects it instantly
2. **Queue the action** — persist the intended operation in an `action_queue` table
3. **Async replay** — the sync worker drains the queue, executing each action against IMAP/SMTP, then lets the next incremental sync confirm or correct the local state

---

## Architecture

### Key Files

| Layer | File | Purpose |
|---|---|---|
| DB Schema | `src-tauri/src/adapters/sqlite/sync/db_schema.rs` | `action_queue` table definition |
| DB Adapter | `src-tauri/src/adapters/sqlite/sync/action_queue.rs` | enqueue, get_pending, mark_* lifecycle |
| DB Adapter | `src-tauri/src/adapters/sqlite/sync/messages.rs` | optimistic message insert/delete/flag update |
| SMTP Adapter | `src-tauri/src/adapters/smtp/send.rs` | RFC 5322 message build + SMTP delivery |
| IMAP Adapter | `src-tauri/src/adapters/imap/envelopes.rs` | envelope parsing for confirmation |
| Replay Task | `src-tauri/src/services/engine/tasks/action_replay.rs` | drains action queue per account |
| Worker | `src-tauri/src/services/engine/worker.rs` | tick loop; calls replay first, then sync |
| Commands | `src-tauri/src/commands/messages.rs` | `send_message` Tauri command |
| Commands | `src-tauri/src/commands/actions.rs` | `queue_action` Tauri command |
| Frontend Tauri | `src/tauri/commands.ts` | `queueAction()`, `sendMessage()` wrappers |
| Frontend Tauri | `src/tauri/types.ts` | `SendMessageParams`, `SendResult` types |
| Conversation View | `src/routes/_app/conversation.$id.tsx` | mark-read observer + compose/send UI |

---

## Database Schema

```sql
CREATE TABLE IF NOT EXISTS action_queue (
    id              TEXT PRIMARY KEY,    -- UUID
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    action_type     TEXT NOT NULL,       -- 'mark_read' | 'send' | 'archive' | 'delete' | 'move' | 'flag' | 'mute' | 'pin'
    payload         TEXT NOT NULL,       -- JSON blob
    status          TEXT NOT NULL DEFAULT 'pending',  -- 'pending' | 'in_progress' | 'completed' | 'failed'
    retry_count     INTEGER DEFAULT 0,
    max_retries     INTEGER DEFAULT 5,
    created_at      INTEGER NOT NULL,    -- epoch ms
    completed_at    INTEGER,
    error           TEXT
);

CREATE INDEX IF NOT EXISTS idx_action_queue_status ON action_queue(status, created_at);
```

Action statuses follow a linear lifecycle:

```
pending → in_progress → completed (deleted on next cleanup)
                      ↘ failed (retry_count++) → pending (if retry_count < max_retries)
                                               → abandoned (if retry_count = max_retries)
```

---

## Implemented Actions

### 1. `mark_read`

**Trigger:** User opens a conversation. Messages become ≥50% visible for ≥1 second (IntersectionObserver in `src/routes/_app/conversation.$id.tsx`).

**Optimistic update (frontend):**
- Immediately mutates local React state: adds `\Seen` to each message's `imap_flags` JSON array
- UI re-renders instantly; unread indicators disappear

**Action queue entry payload:**
```json
{
  "folder": "INBOX",
  "uids": [123, 456]
}
```

Actions are grouped by folder — one queue entry per distinct IMAP folder represented in the batch.

**Async replay (`execute_mark_read`):**
1. Check `read_only` setting — if true, fail gracefully with log warning (action retries then exhausts)
2. Open IMAP connection (read-write)
3. `SELECT folder`
4. `UID STORE uid_set +FLAGS (\Seen)`
5. Mark action `completed`; cleanup on next tick

**Confirmation:** The next `flag_resync` pass fetches current flags from IMAP and overwrites local SQLite. If the server already had `\Seen`, the update is idempotent.

---

### 2. `send`

**Trigger:** User submits the compose box in a conversation (or new conversation view).

**Frontend path (`handleSend` in `src/routes/_app/conversation.$id.tsx`):**
1. Compute subject (prepend `Re:` for replies), `In-Reply-To`, and `References` from the replied-to message or the latest message in thread
2. Call `sendMessage()` Tauri wrapper → invokes `send_message` command

**Backend `send_message` command (`src-tauri/src/commands/messages.rs`):**

1. **Compute placement** — derive `participant_key` and `conversation_id` from sender + recipients, normalizing against known self-email aliases
2. **Insert optimistic placeholder message** into `messages` table:
   - `imap_uid = 0`
   - `imap_folder = "OUTBOX"`
   - `message_id = "<uuid>.eddie@local"` (local placeholder ID)
   - `imap_flags = ["Seen"]`
   - `classification = "chat"`
   - Full body text available immediately
3. **Upsert recipients as entities** in the trust network (source = `"compose"`, `trust_level = "connection"`, `sent_count = 1`)
4. **Enqueue `send` action** with full payload (see below)
5. **Rebuild conversations** via `worker::process_changes()` so the message appears in the list
6. **Wake worker** via `wake_tx` channel

**Action queue entry payload:**
```json
{
  "from": "user@example.com",
  "from_name": "User Name",
  "to": ["recipient@example.com"],
  "cc": [],
  "subject": "Re: Subject",
  "body": "Body text",
  "in_reply_to": "<original-message-id@domain>",
  "references": ["<original-message-id@domain>"],
  "message_db_id": "<uuid>",
  "placeholder_message_id": "<uuid>.eddie@local"
}
```

**Async replay (`execute_send`):**
1. Parse payload; fetch SMTP credentials from Config DB
2. Build RFC 5322 message with `lettre`:
   - Explicit `Message-ID: <uuid@eddie.app>`
   - `In-Reply-To` and `References` headers wrapped in angle brackets per RFC 5322
   - Supports PLAIN, LOGIN, XOAUTH2 auth mechanisms
   - Implicit TLS (port 465) or STARTTLS (port 587) or plain
3. Send via SMTP → returns raw RFC 5322 bytes
4. **Delete OUTBOX placeholder** from SQLite (`delete_message_by_message_id`)
5. **APPEND to Sent folder** via IMAP (unless Gmail — Gmail auto-copies sent messages)
   - Finds Sent folder via `find_sent_folder()` heuristic
   - Appends with `\Seen` flag
6. Mark action `completed`

**Confirmation:** On the next incremental IMAP sync, the real sent message arrives (either from APPEND or Gmail's auto-copy) and is stored in SQLite. The conversation shows the real message; the placeholder is already gone.

---

## Worker Integration

The sync worker tick (`src-tauri/src/services/engine/worker.rs`) runs every 15 seconds or when woken via channel. **Action replay always runs first**, before any IMAP sync:

```
tick():
  1. replay_pending_actions()     ← drain action queue
  2. run_incremental_sync_all()   ← fetch new messages from IMAP
  3. run_flag_resync_all()        ← pull current flags from IMAP (server wins)
  4. run_skill_classify_all()     ← classify messages
  5. onboarding tasks (if needed)
```

Because replay runs before flag resync, actions are applied to IMAP before the local state is overwritten with server truth. This means:
- Optimistic `\Seen` flag is applied to IMAP → flag resync reads it back as `\Seen` → no flicker
- Sent message is APPENDed → incremental sync fetches it back → no gap in history

---

## Read-Only Mode

A `read_only` setting (defaults `true` in early builds) gates all IMAP mutations:

- IMAP connections open with `EXAMINE` (read-only) when this flag is on
- `execute_mark_read` returns an error immediately if `read_only = true`
- The action fails gracefully, increments `retry_count`, and will retry on each tick until exhausted
- `send` actions are **not** gated by `read_only` (sending always requires write access)

---

## Retry and Failure Behavior

- Default `max_retries = 5` (set at enqueue time)
- Failed actions are retried on every worker tick (every 15s or on wake)
- After `retry_count >= max_retries`, the action is no longer picked up by `get_pending` (silently abandoned)
- The error message is stored in `action_queue.error` for debugging
- Completed actions are deleted from the table on the same tick they complete

---

## Frontend Tauri API

```typescript
// Queue a generic IMAP mutation action
queueAction(accountId: string, actionType: string, payload: object): Promise<string>
// Returns the action UUID

// Compose and send a message (creates optimistic placeholder + queues send action)
sendMessage(params: SendMessageParams): Promise<SendResult>
// Returns { message_id: string, conversation_id: string }
```

Both wrappers live in `src/tauri/commands.ts` and are exported from `src/tauri/index.ts`.

---

## Conversation Threading (Send)

When sending, the backend derives the correct conversation from the participants:

1. Normalize all addresses (lowercase, strip aliases)
2. Remove self-addresses from the participant set
3. Compute `participant_key` (sorted, comma-joined normalized emails)
4. Hash `participant_key` → `conversation_id` (deterministic, same conversation for any reply)

This ensures the optimistic placeholder message and the real inbound IMAP message land in the same conversation.

---

## Future Action Types (Planned in Schema, Not Yet Implemented)

The `action_type` column schema anticipates but does not yet implement:
- `archive` — move message to Archive folder
- `delete` — move to Trash / expunge
- `move` — move to arbitrary folder
- `flag` — toggle starred/flagged
- `mute` — mute conversation (suppress future notifications)
- `pin` — pin conversation to top
