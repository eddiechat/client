# Codebase Audit — Issues & Discrepancies

Full audit of frontend and backend code, ordered by severity.

---

## High

### 7. Auth state not persisted across page reload
**File:** `src/shared/context/AuthContext.tsx:39-42`

`loggedIn` defaults to `false` and is only set after `handleLogin()` completes. Refreshing the page sends the user back to the login screen even though an account exists in the database.

**Fix:** On mount, check if an account exists in the DB and set `loggedIn` accordingly.

---

## Medium

### 8. Compose button and message input are non-functional
**Files:**
- `src/routes/_app/_tabs.tsx:54-58` — Compose button has no `onClick` handler
- `src/routes/_app/conversation.$id.tsx:163-168` — Message input and send button have no handlers

Both are visible in the UI but do nothing when clicked.

---

### 9. Participant key normalization could drift between insert and rebuild
**File:** `src-tauri/src/services/sync/helpers/message_builder.rs:19-45`

Messages are inserted with `normalize_email()` applied to participant keys. Conversation rebuild reads `from_address` (already normalized at insert) and recomputes. If normalization logic ever changes, stored keys and rebuilt keys could diverge, fragmenting conversations into duplicates.

---

### 13. Silent error suppression
- `src-tauri/src/adapters/sqlite/sync/conversations.rs:~250` — `serde_json::to_string().ok()` silently drops serialization errors for participant names

---

### 14. No logout functionality
**File:** `src/routes/_app/_tabs.tsx:143-149`

Account drawer has a Settings button but no sign-out or switch-account option. Users have no way to disconnect.

---

## Low

### 15. UnionFind uses recursion
**File:** `src-tauri/src/adapters/sqlite/sync/conversations.rs:484-495`

The `find()` method uses recursion with path compression. Could stack overflow on extremely deep threads (unlikely in practice but violates best practices).

**Fix:** Convert to iterative implementation with a while loop.

---

### 16. References fetch logic duplicated across tasks
The 3-round-trip fetch pattern (envelopes, references, bodies) is duplicated in:
- `services/sync/tasks/incremental_sync.rs`
- `services/sync/tasks/connection_history.rs`
- `adapters/imap/historical.rs`

Maintenance risk — a fix in one location can be missed in others.

---

### 17. Event listener cleanup returns Promise
**File:** `src/shared/context/DataContext.tsx:27`

```typescript
return () => { u.then((f) => f()); };
```

React cleanup functions should be synchronous. Returning a function that creates a dangling Promise could cause race conditions during unmount.

---

### 18. Conversation/cluster list filtering not memoized
**Files:** `src/routes/_app/_tabs/points.tsx`, `circles.tsx`

Filtering and sorting happen on every render without `useMemo`. With thousands of conversations and active search input, this recalculates on every keystroke.

---

## Summary

| # | Severity | Description |
|---|----------|-------------|
| 7 | High | Auth state not persisted across reload |
| 8 | Medium | Compose button non-functional |
| 9 | Medium | Participant key normalization drift risk |
| 13 | Medium | Silent error suppression |
| 14 | Medium | No logout functionality |
| 15 | Low | UnionFind uses recursion |
| 16 | Low | References fetch logic duplicated |
| 17 | Low | Event listener cleanup returns Promise |
| 18 | Low | List filtering not memoized |
