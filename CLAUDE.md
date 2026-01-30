# Claude Code Guidelines for eddie.chat

This document provides instructions for Claude when working on the eddie.chat codebase.

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

## Creating New Features

When adding a new feature:

1. Create the feature directory: `src/features/{feature-name}/`
2. Add subdirectories as needed: `components/`, `hooks/`, `context/`
3. Create barrel exports at each level
4. Add any Tauri commands to `src/tauri/commands.ts`
5. Add any new types to `src/tauri/types.ts`
6. Export the feature from `src/features/index.ts`

## File Naming

- Components: PascalCase (`ConversationView.tsx`)
- Hooks: camelCase with `use` prefix (`useConversations.ts`)
- Utilities: camelCase (`utils.ts`)
- Types: PascalCase for types/interfaces, camelCase for type files

## Backend (Rust/Tauri)

- Commands go in `src-tauri/src/commands/`
- Register commands in `src-tauri/src/lib.rs`
- Business logic in `src-tauri/src/services/`
- State management in `src-tauri/src/state/`
- Types in `src-tauri/src/types/`

## Common Mistakes to Avoid

1. **Don't** create new top-level directories in `src/`
2. **Don't** call `invoke()` outside of `src/tauri/commands.ts`
3. **Don't** put feature-specific code in `shared/`
4. **Don't** forget barrel exports when adding new files
5. **Don't** use `any` types - define proper interfaces
6. **Don't** mix concerns - keep UI separate from data fetching
