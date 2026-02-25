# Skill Classification Engine

This document specifies the background classification engine that runs user-defined **skills** against all messages in the local database. A skill is an LLM-powered classifier: the user writes a natural-language prompt describing which emails should match, and the engine evaluates every message against that prompt using a local Ollama model.

The engine is designed to run inside the existing sync worker tick loop (see [IMAP_SYNC.md](./IMAP_SYNC.md)), processing small batches each tick so it never blocks mail delivery or onboarding.

---

## Overview

```
┌──────────────────────────────────────────────────────────────────┐
│  Worker Tick Loop (15s)                                          │
│                                                                  │
│  1. run_incremental_sync_all()     ← fetch new mail              │
│  2. run_flag_resync_all()          ← sync flags/labels           │
│  3. run_skill_classify_all()       ← NEW: classify one batch     │
│  4. onboarding tasks (if any)                                    │
│                                                                  │
│  ┌───────────────────────────────────────────────┐               │
│  │  run_skill_classify_all()                     │               │
│  │                                               │               │
│  │  for each onboarded account:                  │               │
│  │    for each enabled skill:                    │               │
│  │      ensure folder_classify cursors           │               │
│  │      check revision_hash → reset if stale      │               │
│  │                                               │               │
│  │    Phase 1: forward batch (new messages)      │               │
│  │    Phase 2: backward batch (historical)       │               │
│  │                                               │               │
│  │    → classify batch of 10 via Ollama          │               │
│  │    → persist matches to skill_matches         │               │
│  │    → advance cursor                           │               │
│  └───────────────────────────────────────────────┘               │
│                                                                  │
│           ┌──────────────┐    ┌─────────────────-─┐              │
│           │ Ollama API   │    │  SQLite (sync.db) │              │
│           │ /v1/chat/    │    │  folder_classify  │              │
│           │ completions  │    │  skill_matches    │              │
│           └──────────────┘    └──────────────────-┘              │
└──────────────────────────────────────────────────────────────────┘
```

### Key properties

- **Many-to-many:** One message can match many skills; one skill can match many messages.
- **Only matches persisted:** Messages that don't match a skill leave no trace — no "tested but didn't match" records.
- **Revision-tracked:** Every skill has a `revision_hash` derived from its classification-affecting inputs (prompt, model, temperature). Matches are discarded and reclassification begins only when the hash actually changes — saving a skill without meaningful changes is a no-op.
- **Batch-oriented:** 10 messages per tick. Small batches keep the tick responsive (Ollama calls take ~1–2s each).
- **Priority:** New messages (forward cursor) are always classified before historical messages (backward cursor).

### Key source files

| Area | Files |
|------|-------|
| DB adapter | `adapters/sqlite/sync/skill_classify.rs` |
| Task logic | `services/sync/tasks/skill_classify.rs` |
| Worker integration | `services/sync/worker.rs` (tick function) |
| Skills CRUD | `adapters/sqlite/sync/skills.rs` |
| Ollama adapter | `adapters/ollama/mod.rs` |
| Schema | `adapters/sqlite/sync/db_schema.rs` |

All paths are relative to `src-tauri/src/`.

---

## Data Model

### `skills` table (modified)

A `revision_hash` column is added to the `skills` table. It stores a SHA-256 hash (truncated to 16 hex characters) of the fields that affect classification output: **prompt**, **model**, and **temperature**. The hash is recomputed on every save — if the result is the same as the stored value, no reclassification occurs.

The existing `skills` table is replaced in the schema (DROP + CREATE). No migration needed — skills are lightweight user config that can be recreated.

```sql
CREATE TABLE IF NOT EXISTS skills (
    id              TEXT PRIMARY KEY,
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    name            TEXT NOT NULL,
    icon            TEXT NOT NULL DEFAULT '⚡',
    icon_bg         TEXT NOT NULL DEFAULT '#5b4fc7',
    enabled         INTEGER NOT NULL DEFAULT 1,
    prompt          TEXT NOT NULL DEFAULT '',
    modifiers       TEXT NOT NULL DEFAULT '{}',   -- JSON (SkillModifiers)
    settings        TEXT NOT NULL DEFAULT '{}',   -- JSON (ollamaModel, temperature)
    revision_hash   TEXT NOT NULL DEFAULT '',      -- SHA-256 of prompt+model+temperature
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);
```

### Revision hash computation

The hash is computed from the three fields that determine classification behavior:

```
input  = "{prompt}\0{ollamaModel}\0{temperature}"
hash   = SHA-256(input)[..16]   -- first 16 hex characters
```

- **prompt** — the classification instruction
- **ollamaModel** — extracted from the `settings` JSON field `ollamaModel` (empty string if absent)
- **temperature** — extracted from the `settings` JSON field `temperature` (stringified, `"0"` if absent)

The null byte (`\0`) separator prevents collisions between fields (e.g., prompt "ab" + model "cd" vs prompt "a" + model "bcd").

The hash is computed in Rust during `create_skill()` and `update_skill()`. Cosmetic changes (name, icon, icon_bg) do not affect the hash and therefore do not trigger reclassification.

### `skill_matches` table (new)

Junction table storing only matches. No record is created for messages that don't match.

```sql
CREATE TABLE IF NOT EXISTS skill_matches (
    skill_id    TEXT NOT NULL,
    message_id  TEXT NOT NULL,
    matched_at  INTEGER NOT NULL,   -- unix epoch ms
    PRIMARY KEY (skill_id, message_id)
);

CREATE INDEX IF NOT EXISTS idx_skill_matches_message
    ON skill_matches(message_id);
```

The composite primary key enforces one match record per skill–message pair. `INSERT OR IGNORE` prevents duplicates if a message is re-encountered.

When a skill is deleted, all its matches are cascade-deleted. When a skill's `revision_hash` changes, all its matches are explicitly deleted before reclassification begins.

### `folder_classify` table (new)

Per-skill, per-folder UID cursors, analogous to the `folder_sync` table used by the IMAP sync engine.

```sql
CREATE TABLE IF NOT EXISTS folder_classify (
    skill_id                TEXT NOT NULL,
    account_id              TEXT NOT NULL,
    folder                  TEXT NOT NULL,
    skill_rev               TEXT NOT NULL DEFAULT '',      -- revision_hash at time of classification
    highest_classified_uid  INTEGER NOT NULL DEFAULT 0,   -- forward cursor
    lowest_classified_uid   INTEGER NOT NULL DEFAULT 0,   -- backward cursor
    last_classify           INTEGER,                      -- unix epoch ms
    PRIMARY KEY (skill_id, account_id, folder)
);
```

| Column | Purpose |
|--------|---------|
| `skill_rev` | The `revision_hash` value this cursor state was classified under. When `skill.revision_hash != skill_rev`, the cursors are stale and must be reset. |
| `highest_classified_uid` | Forward cursor: messages with `imap_uid > highest_classified_uid` are "new" and have not been classified by this skill. |
| `lowest_classified_uid` | Backward cursor: messages with `imap_uid < lowest_classified_uid` are "historical" and have not been classified by this skill. |
| `last_classify` | Timestamp of the most recent classification pass on this folder for this skill. |

---

## Cursor Mechanics

### Initialization

When a skill is first created (or after a revision reset), `folder_classify` rows are created with both cursors at **0**:

```
highest_classified_uid = 0
lowest_classified_uid  = 0
```

The forward query `WHERE imap_uid > 0 ORDER BY imap_uid ASC` covers all existing messages, starting from the oldest. This is the primary scan path for initial population.

### Forward scan (new messages — high priority)

```sql
SELECT id, imap_uid, subject, body_text
FROM messages
WHERE account_id = ?
  AND imap_folder = ?
  AND imap_uid > ?             -- highest_classified_uid
  AND processed_at IS NOT NULL
  AND body_text IS NOT NULL
  {modifier_clauses}
ORDER BY imap_uid ASC
LIMIT 10
```

After classifying the batch, advance the cursor:

```sql
UPDATE folder_classify
SET highest_classified_uid = ?,   -- MAX(imap_uid) from the batch
    last_classify = ?
WHERE skill_id = ? AND account_id = ? AND folder = ?
  AND highest_classified_uid < ?
```

### Backward scan (historical — low priority)

Once the forward scan reaches the end (no more messages above the cursor), the backward cursor is initialized to `highest_classified_uid`. The backward scan then fills in any messages below that point:

```sql
SELECT id, imap_uid, subject, body_text
FROM messages
WHERE account_id = ?
  AND imap_folder = ?
  AND imap_uid < ?             -- lowest_classified_uid
  AND processed_at IS NOT NULL
  AND body_text IS NOT NULL
  {modifier_clauses}
ORDER BY imap_uid DESC
LIMIT 10
```

After classifying the batch, advance the cursor:

```sql
UPDATE folder_classify
SET lowest_classified_uid = ?,   -- MIN(imap_uid) from the batch
    last_classify = ?
WHERE skill_id = ? AND account_id = ? AND folder = ?
  AND (lowest_classified_uid = 0 OR lowest_classified_uid > ?)
```

### Priority

Within a single tick, the engine:

1. Checks **all** enabled skills for forward work (new messages above `highest_classified_uid` in any folder).
2. If any forward work exists, processes **one batch of 10** from the first skill with work, then returns.
3. Only if no forward work exists across all skills does it fall through to backward (historical) processing.
4. Processes **one batch of 10** from the first skill with backward work, then returns.

This ensures new messages are always classified before historical backfill resumes.

### Completion

When no more forward or backward work exists for any enabled skill in any folder, the classification tick returns `false` (no work done), and the worker sleeps until the next tick.

---

## Revision Tracking & Reset

### Hash computation on save

When `create_skill()` or `update_skill()` is called, the `revision_hash` is recomputed from the current prompt, model, and temperature:

```rust
fn compute_revision_hash(prompt: &str, settings_json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(settings_json)
        .unwrap_or(serde_json::Value::Object(Default::default()));
    let model = parsed.get("ollamaModel")
        .and_then(|v| v.as_str()).unwrap_or("");
    let temperature = parsed.get("temperature")
        .and_then(|v| v.as_f64()).unwrap_or(0.0);

    let input = format!("{}\0{}\0{}", prompt, model, temperature);
    let digest = sha2::Sha256::digest(input.as_bytes());
    hex::encode(&digest[..8])  // 16 hex chars
}
```

The SQL stores the result:

```sql
UPDATE skills SET ..., revision_hash = ? WHERE id = ?
```

If the computed hash equals the existing `revision_hash`, the classification state is unaffected — no reclassification occurs. This means saving a skill with only cosmetic changes (name, icon) is a no-op from the classification engine's perspective.

### Reset detection

On each tick, the engine checks every `folder_classify` row for the skill. If `folder_classify.skill_rev != skill.revision_hash`:

```
reset_skill_cursors(skill_id, new_hash):
    1. DELETE FROM skill_matches WHERE skill_id = ?
    2. UPDATE folder_classify
       SET highest_classified_uid = 0,
           lowest_classified_uid = 0,
           skill_rev = ?,
           last_classify = NULL
       WHERE skill_id = ?
```

This clears all stale matches and resets cursors to 0, causing full reclassification from scratch starting on the next tick.

### Toggle behavior

- **Disable:** The skill is filtered out at the top of the tick (`WHERE enabled = 1`). Existing matches remain in `skill_matches` for display, but no new classification runs. Cursors are untouched.
- **Enable:** Classification resumes from the current cursor positions. No revision change, no reset.

### Delete behavior

When a skill is deleted:

```sql
DELETE FROM skill_matches WHERE skill_id = ?
DELETE FROM folder_classify WHERE skill_id = ?
DELETE FROM skills WHERE id = ?
```

---

## Worker Integration

### Tick loop placement

```rust
pub async fn tick(app, pool) -> Result<bool, EddieError> {
    // Step 1: Always sync mail + flags (unchanged)
    let _ = tasks::run_incremental_sync_all(app, pool).await;
    let _ = tasks::run_flag_resync_all(app, pool).await;

    // Step 2: Skill classification (NEW — one batch per tick)
    let skill_did_work = tasks::run_skill_classify_all(app, pool)
        .await
        .unwrap_or(false);

    // Step 3: Onboarding (unchanged)
    let account_id = match accounts::find_account_for_onboarding(pool)? {
        Some(id) => id,
        None => return Ok(skill_did_work),
    };
    // ... existing onboarding logic ...
    Ok(true)
}
```

Skill classification runs **after** incremental sync (so new messages are already inserted and classified by the built-in classifier via `process_changes()`), and **before** onboarding tasks.

### Return value behavior

- If skill classification processed a batch → `tick()` returns `true` → worker loops immediately (no 15s sleep). This allows rapid processing of backlogs while the Ollama call duration provides natural throttling.
- If no classification work remains → `tick()` returns `false` (unless onboarding is active) → worker sleeps.
- Onboarding always returns `true` and takes precedence.

### Ordering guarantee

Because `run_incremental_sync_all()` calls `process_changes()` which sets `processed_at` on new messages, skill classification's `WHERE processed_at IS NOT NULL` filter ensures we never classify a message before its built-in classification (chat/newsletter/automated/transactional) is set. This is critical because modifiers like `excludeNewsletters` depend on the built-in classification.

---

## Ollama Integration

### Prompt construction

The engine uses the same prompt format as the frontend preview in SkillStudio:

```
System prompt:
  "You are an email classifier. Given a classification prompt and an email,
   decide if the email matches. Respond with exactly one word: true or false.
   Do not explain."

User prompt:
  "Classification prompt: {skill.prompt}

   Email subject: {message.subject}
   Email body: {message.body_text[:2000]}"
```

Body text is truncated to 2000 characters to keep prompt size manageable.

### Config resolution (per skill)

| Setting | Source | Fallback |
|---------|--------|----------|
| **URL** | `settings` table, key `ollama_url` | `http://localhost:11434` |
| **Model** | Skill's `settings` JSON, field `ollamaModel` | `settings` table, key `ollama_model` |
| **Temperature** | Skill's `settings` JSON, field `temperature` | `0.0` |

If no model is resolvable (neither per-skill nor global), the skill is **silently skipped** — no error, no log spam. The user must configure a model before the skill will run.

### Response parsing

```
response.trim().to_lowercase().contains("true") → MATCH → insert into skill_matches
anything else                                    → NO MATCH → no record
```

This is intentionally lenient (contains rather than equals) to handle minor LLM output variations like "True.", "true\n", etc.

---

## Modifier Filters

Modifiers are stored as a JSON string in `skills.modifiers` and are applied as SQL WHERE clauses **before** the Ollama call, reducing unnecessary inference:

| Modifier | JSON key | SQL clause |
|----------|----------|-----------|
| Exclude newsletters | `excludeNewsletters` | `AND classification != 'newsletter'` |
| Exclude auto-replies | `excludeAutoReplies` | `AND classification != 'automated'` |
| Has attachments | `hasAttachments` | `AND has_attachments = 1` |
| Recent 6 months | `recentSixMonths` | `AND date >= {now - 180 days in epoch ms}` |
| Only known senders | `onlyKnownSenders` | `AND from_address IN (SELECT email FROM entities WHERE account_id = ? AND trust_level IN ('connection', 'contact'))` |

All queries also include:
```sql
AND processed_at IS NOT NULL   -- wait for built-in classification
AND body_text IS NOT NULL       -- need body for LLM evaluation
```

---

## Batch Processing

### Batch size

**10 messages per tick.** This is deliberately small:

- Each Ollama call takes ~1–2 seconds for small models (ministral-3:3b).
- A batch of 10 → ~10–20 seconds of processing time per tick.
- The worker still ticks every 15 seconds after the batch completes (or immediately if `did_work` is true).
- For a mailbox with 50,000 messages and 1 skill, full historical classification takes ~75,000 seconds (~21 hours) of background processing. This is acceptable for a non-blocking background task.

### Per-tick processing

Only **one batch** is processed per tick, across all skills and folders. This means:

- With 3 enabled skills, each skill gets roughly 1 batch every 3 ticks (round-robin effect from iterating skills).
- New messages for any skill are prioritized over historical backfill for all skills.
- If Ollama is fast (< 15s for 10 messages), the worker immediately loops for another batch.

### Batch failure

If an Ollama call fails mid-batch:

1. Any matches found so far in the batch are persisted.
2. The cursor advances to the last **successfully classified** message's UID.
3. The error is logged.
4. The function returns an error, which the caller catches and logs. Processing moves to the next skill (or stops for this tick).
5. On the next tick, the failed message is retried from the cursor position.

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| **Ollama is down** (connection refused, timeout) | Batch stops. Cursor stays at its current position. Matches found so far are persisted. Retries next tick. |
| **Ollama returns unexpected output** (not "true"/"false") | Treated as "no match." Message is skipped, cursor advances past it. No retry for that message. |
| **No model configured** | `resolve_ollama_config()` returns `None`. Skill is silently skipped. |
| **Skill disabled between ticks** | Filtered out by the `enabled` check at tick start. No processing occurs. |
| **Skill deleted between ticks** | Explicit cleanup in `delete_skill()` removes `skill_matches` and `folder_classify` rows. Worker won't find the skill in the enabled list. |
| **Empty prompt** | Skip classification for this skill. An empty prompt would produce meaningless results. |
| **Message deleted from DB** | `INSERT OR IGNORE` on `skill_matches` handles the case where `message_id` no longer exists. |
| **Database errors** | Propagated as `EddieError::Database`, logged by the worker. Skill is skipped for this tick. |

---

## Database State Summary

### Tables involved

| Table | Role |
|-------|------|
| `skills` | Skill definitions with `revision_hash` column |
| `skill_matches` | Many-to-many junction: `(skill_id, message_id)` — only matches |
| `folder_classify` | Per-skill, per-folder UID cursors with revision hash tracking |
| `messages` | Source of messages to classify (read-only from this engine's perspective) |
| `entities` | Trust network, used by `onlyKnownSenders` modifier filter |
| `settings` | Global Ollama URL and model settings |

### Resumability

The engine survives app restarts at any point:

- **Cursors** are persisted in `folder_classify`. On restart, classification resumes from the last committed cursor position.
- **Matches** are committed incrementally. Any matches found before a crash are retained.
- **Revision tracking** is durable. If a skill was updated but the reset didn't complete, the next tick will detect the hash mismatch and complete the reset.

---

## File Organization

### New files

| File | Contents |
|------|----------|
| `adapters/sqlite/sync/skill_classify.rs` | `ClassifyCursor` struct (with `skill_rev: String` for hash comparison), `Modifiers` struct + `from_json()`, cursor CRUD (`ensure_cursor`, `get_cursor`, `reset_skill_cursors`, `update_highest_classified_uid`, `update_lowest_classified_uid`), batch queries (`get_forward_batch`, `get_backward_batch`), match CRUD (`insert_matches_batch`, `delete_skill_data`, `count_matches`), `get_message_folders()` |
| `services/sync/tasks/skill_classify.rs` | `run_skill_classify_all()` (entry point called from worker), `run_skill_classify_one()` (per-skill orchestration), `classify_batch()` (Ollama calls + match persistence), `resolve_ollama_config()`, `SYSTEM_PROMPT` constant, `BATCH_SIZE` constant (10), `BODY_SNIPPET_LEN` constant (2000) |

### Modified files

| File | Changes |
|------|---------|
| `adapters/sqlite/sync/db_schema.rs` | Replace `skills` table definition to include `revision_hash`. Add `folder_classify` table, `skill_matches` table + index to schema. |
| `adapters/sqlite/sync/skills.rs` | Add `revision_hash: String` to `Skill` struct. Update all SELECT queries to include `revision_hash`. Add `compute_revision_hash()` helper. Change `create_skill()` and `update_skill()` to compute and store the hash. Change `delete_skill()` to also delete from `skill_matches` and `folder_classify`. |
| `adapters/sqlite/sync/mod.rs` | Add `pub mod skill_classify;` |
| `services/sync/tasks/mod.rs` | Add `mod skill_classify;` and `pub use skill_classify::run_skill_classify_all;` |
| `services/sync/worker.rs` | Add `tasks::run_skill_classify_all(app, pool).await` call between flag_resync and onboarding. Wire `skill_did_work` into the return value. |

All paths relative to `src-tauri/src/`.
