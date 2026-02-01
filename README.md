<p align="center">
  <img src="public/eddie-swirl-green.svg" alt="eddie logo" width="120" height="120">
</p>

# eddie

**eddie** is a modern, lightweight desktop email client that reimagines email as a conversation-first experience. Inspired by the clean aesthetics of Signal, eddie groups your emails into threaded conversations by participant, making email feel as natural as messaging.

Built on standard email protocols, eddie brings the simplicity of modern chat to your inbox, without locking you into another platform.

- **Privacy**: Your data stays on your machine. No cloud sync, no tracking, no middleman.
- **Transparency**: Fully open source with a client-centric architecture.
- **Simplicity**:  Signal-inspired interface that cuts through inbox noise with smart, personalized filters, and zero onboarding.
- **Openness**: Works with anyone who has an email address. No new accounts, no walled gardens.

But it doesn't stop there. We want to augment communication with a client-centric and fully transparent AI infrastructure.

We believe that an open and shared repository of agent skills, and the ability for anyone to easily use, improve, and reshare skills, will help humanity communicate better, spark creativity, learn faster, and automate repetitive processes.

[Read the Manifesto →](https://eddie.chat)

## Architecture

### High-Level Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Desktop Application                    │
├─────────────────────────┬───────────────────────────────────┤
│     React Frontend      │           Rust Backend            │
│     (TypeScript)        │            (Tauri v2)             │
├─────────────────────────┼───────────────────────────────────┤
│  • Components           │  • EmailBackend Service           │
│  • Hooks                │  • Command Handlers               │
│  • State Management     │  • Configuration System           │
│  • Tauri IPC Client     │  • Type Serialization             │
└─────────────────────────┴───────────────────────────────────┘
                               │
                               ▼
                    ┌─────────────────────┐
                    │   Email Servers     │
                    │   (IMAP / SMTP)     │
                    └─────────────────────┘
```

### Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Frontend** | React 19 + TypeScript | UI components and state management |
| **Build Tool** | Vite 7 | Fast development and bundling |
| **Desktop Runtime** | Tauri v2 | Cross-platform native shell |
| **Backend** | Rust (Edition 2021) | Core email operations |
| **Async Runtime** | Tokio | Non-blocking I/O |
| **Email Protocol** | email-lib (pimalaya) | IMAP/SMTP implementation |
| **Config** | TOML | Human-readable configuration |

### Vendored Dependencies

Eddie uses a vendored version of the [pimalaya/core](https://github.com/pimalaya/core) email library to enable custom patches while maintaining easy upstream tracking:

**Location**: `src-tauri/vendor/pimalaya-core/`

**Method**: Git subtree (vendored commit: `c36dd7c5`)

**Rationale**: The upstream email-lib didn't extract CC (carbon copy) recipients from IMAP envelope responses. We vendor the library to patch this functionality while maintaining the ability to pull and merge upstream updates.

**Custom Patches**:
- **CC Field Support**: Adds CC field extraction to the `Envelope` struct for both IMAP envelope parsing and full message parsing
- Detailed patch documentation in [`src-tauri/vendor/patches/`](src-tauri/vendor/patches/)

**Updating from Upstream**:
```bash
git subtree pull --prefix=src-tauri/vendor/pimalaya-core \
  https://github.com/pimalaya/core.git master --squash
```

See [`src-tauri/vendor/README.md`](src-tauri/vendor/README.md) for complete vendoring documentation and maintenance workflow.

### Data Flow

```
User Action → React Component → Tauri invoke() → Rust Command Handler
                                                         │
                                                         ▼
                                                  EmailBackend
                                                         │
                                                         ▼
                                              IMAP/SMTP Server
                                                         │
                                                         ▼
                                              Response (JSON)
                                                         │
User Interface ← React State Update ← Tauri Response ←───┘
```

### Core Components

#### Frontend (`src/`)

The frontend follows a **feature-based architecture** with clear separation of concerns:

| Directory | Purpose |
|-----------|---------|
| `features/` | Feature modules organized by domain (accounts, conversations) |
| `shared/` | Reusable components, hooks, and utilities across features |
| `tauri/` | Centralized Tauri communication layer (commands, events, types) |
| `lib/` | Static data and external integrations (emoji data) |

##### Feature Modules (`features/`)

Each feature module is self-contained with its own components, hooks, and utilities:

| Module | Contents |
|--------|----------|
| `accounts/` | Account management (SidebarHeader, AccountSetupWizard, AccountConfigModal, useAccounts hook, AccountContext) |
| `conversations/` | Email conversations (ChatMessages, ConversationView, useConversations hook, useConversationMessages hook) |

##### Tauri Layer (`tauri/`)

All Tauri communication is centralized for type safety and maintainability:

| File | Purpose |
|------|---------|
| `commands.ts` | Type-safe wrappers for all `invoke()` calls to Rust backend |
| `events.ts` | Event listener subscriptions (sync events, status changes) |
| `types.ts` | TypeScript types mirroring Rust backend types |
| `index.ts` | Barrel exports for clean imports |

##### Shared Utilities (`shared/`)

| Directory | Purpose |
|-----------|---------|
| `components/` | Generic UI components (Avatar, LoadingSpinner, EmptyState) |
| `lib/` | Utility functions (avatar colors, email parsing, date formatting) |

#### Backend (`src-tauri/src/`)

| Module | Purpose |
|--------|---------|
| `backend/` | EmailBackend service - IMAP/SMTP operations |
| `commands/` | Tauri command handlers exposed to frontend |
| `config/` | TOML configuration management and account settings |
| `types/` | Rust structs for serialization across IPC boundary |

---

## Project Structure

```
eddie.chat/
├── src/                              # React/TypeScript frontend
│   ├── App.tsx                       # Main application component
│   ├── App.css                       # Global styles (dark theme)
│   ├── main.tsx                      # React entry point
│   │
│   ├── features/                     # Feature modules (domain-based)
│   │   ├── accounts/                 # Account management feature
│   │   │   ├── components/           # Account-related UI
│   │   │   │   ├── SidebarHeader.tsx
│   │   │   │   ├── AccountSetupWizard.tsx
│   │   │   │   ├── AccountConfigModal.tsx
│   │   │   │   └── index.ts
│   │   │   ├── hooks/
│   │   │   │   ├── useAccounts.ts    # Account state management
│   │   │   │   └── index.ts
│   │   │   ├── context/
│   │   │   │   ├── AccountContext.tsx # Global account state
│   │   │   │   └── index.ts
│   │   │   └── index.ts              # Barrel exports
│   │   │
│   │   ├── conversations/            # Conversations feature
│   │   │   ├── components/           # Conversation UI
│   │   │   │   ├── ChatMessages.tsx  # Conversation list
│   │   │   │   ├── ChatMessage.tsx   # Single conversation item
│   │   │   │   ├── ConversationView.tsx # Main chat view
│   │   │   │   ├── AttachmentList.tsx
│   │   │   │   ├── EmojiPicker.tsx
│   │   │   │   ├── GravatarModal.tsx
│   │   │   │   └── index.ts
│   │   │   ├── hooks/
│   │   │   │   ├── useConversations.ts      # Conversation list
│   │   │   │   ├── useConversationMessages.ts # Messages in conversation
│   │   │   │   └── index.ts
│   │   │   ├── utils.ts              # Conversation helpers
│   │   │   └── index.ts
│   │   │
│   │   └── index.ts                  # Feature barrel exports
│   │
│   ├── shared/                       # Shared utilities & components
│   │   ├── components/               # Generic UI components
│   │   │   ├── Avatar.tsx
│   │   │   ├── LoadingSpinner.tsx
│   │   │   ├── EmptyState.tsx
│   │   │   └── index.ts
│   │   ├── lib/                      # Utility functions
│   │   │   ├── utils.ts              # Avatar, email, date utils
│   │   │   └── index.ts
│   │   └── index.ts
│   │
│   ├── tauri/                        # Tauri integration layer
│   │   ├── commands.ts               # Type-safe invoke wrappers
│   │   ├── events.ts                 # Event listener subscriptions
│   │   ├── types.ts                  # Backend contract types
│   │   └── index.ts                  # Barrel exports
│   │
│   └── lib/
│       └── emojiData.ts              # Emoji database
│
├── src-tauri/                        # Rust backend
│   ├── src/
│   │   ├── main.rs                   # Application entry point
│   │   ├── lib.rs                    # Tauri initialization
│   │   ├── backend/                  # Email operations
│   │   │   └── mod.rs
│   │   ├── commands/                 # IPC command handlers
│   │   │   ├── accounts.rs
│   │   │   ├── conversations.rs
│   │   │   ├── messages.rs
│   │   │   ├── discovery.rs
│   │   │   ├── flags.rs
│   │   │   ├── folders.rs
│   │   │   ├── sync.rs
│   │   │   └── mod.rs
│   │   ├── services/                 # Business logic services
│   │   │   ├── account_service.rs
│   │   │   ├── message_service.rs
│   │   │   └── mod.rs
│   │   ├── state/                    # Application state
│   │   │   ├── sync_manager.rs
│   │   │   ├── oauth_state.rs
│   │   │   └── mod.rs
│   │   └── types/                    # Data structures
│   │       ├── mod.rs
│   │       ├── responses.rs
│   │       └── error.rs
│   ├── vendor/                       # Vendored dependencies
│   │   ├── pimalaya-core/            # email-lib, secret-lib, etc.
│   │   ├── patches/                  # Patch documentation
│   │   │   └── 001-envelope-cc-field.md
│   │   └── README.md                 # Vendoring documentation
│   ├── Cargo.toml                    # Rust dependencies
│   ├── tauri.conf.json               # Tauri configuration
│   └── icons/                        # Application icons
│
├── package.json                      # Frontend dependencies
├── vite.config.ts                    # Vite configuration
├── tsconfig.json                     # TypeScript configuration
└── index.html                        # HTML entry point
```

---

## Installation

### Prerequisites

- **Rust** (latest stable) - [Install Rust](https://rustup.rs/)
- **Node.js** 18+ or **Bun** - [Install Bun](https://bun.sh/) (recommended)
- **Platform-specific dependencies**:

**macOS:**
```bash
xcode-select --install
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel \
    openssl-devel \
    curl \
    wget \
    file \
    libxdo-devel \
    libappindicator-gtk3-devel \
    librsvg2-devel
```

**Windows:**
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- Install [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

### Clone and Install

```bash
# Clone the repository
git clone https://github.com/eddiechat/client.git
cd client

# Install frontend dependencies
bun install
# or with npm
npm install
```

---

## Running the Application

### Development Mode

```bash
# Start the development environment with hot reload
bun run tauri dev
# or
npm run tauri dev
```

This will:
1. Start the Vite dev server on port 1420
2. Compile the Rust backend
3. Launch the desktop application with hot reload

### Production Build

```bash
# Build the application for your platform
bun run tauri build
# or
npm run tauri build
```

Built applications will be in `src-tauri/target/release/bundle/`.

---

## Releases

This project uses automated builds via GitHub Actions.

### Development Builds

Every push to `main` (including merged PRs) automatically creates a **pre-release** build. These builds are not thoroughly tested and are intended for development and testing purposes only.

### Stable Releases

Stable releases are created by pushing a version tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```

### Downloads

| Platform | Architectures | Formats |
|----------|---------------|---------|
| Windows | x64 | `.msi`, `.exe` |
| macOS | Apple Silicon, Intel | `.dmg` |
| Linux | x64 | `.deb`, `.AppImage` |

Download the latest stable release or development builds from the [Releases](../../releases) page.

Note that the builds aren't signed, for iOS run the following command after installation.

`xattr -cr /Applications/eddie.chat.app`

More info: https://claude.ai/share/6a5cdec1-f6ba-4152-8c36-7347eddab9f1

---

## Configuration

eddie.chat stores configuration at:

- **Linux/macOS**: `~/.config/eddie.chat/config.toml` or `~/.eddie.chat.rc`
- **Windows**: `%APPDATA%\eddie.chat\config.toml`

### Example Configuration

```toml
# Default account to use
default_account = "personal"

[accounts.personal]
email = "you@example.com"
display_name = "Your Name"

[accounts.personal.imap]
host = "imap.example.com"
port = 993
encryption = "tls"  # "tls", "starttls", or "none"

[accounts.personal.imap.auth]
type = "password"
# Raw password (not recommended for shared machines)
password = "your-password"
# Or use a command to fetch from keychain
# command = "security find-generic-password -a your@email.com -s eddie -w"

[accounts.personal.smtp]
host = "smtp.example.com"
port = 587
encryption = "starttls"

[accounts.personal.smtp.auth]
type = "password"
password = "your-password"
```

### Adding Accounts via UI

You can also add and configure accounts directly through the application's settings interface without editing the TOML file manually.

---

## Development

### Project Scripts

```bash
# Start Tauri development environment
bun run tauri dev

# Build for production
bun run tauri build

# Run frontend only (no Rust backend)
bun run dev

# Build frontend only
bun run build

# Preview production frontend build
bun run preview
```

### Code Structure Guidelines

**Frontend Architecture:**

The frontend follows a **feature-based architecture** with these principles:

1. **Feature Modules** (`src/features/`)
   - Group code by domain (accounts, conversations) not by type
   - Each feature has its own components, hooks, and utilities
   - Features export via barrel `index.ts` files for clean imports

2. **Tauri Layer** (`src/tauri/`)
   - **Never call `invoke()` directly in components**
   - All backend communication goes through `tauri/commands.ts`
   - Event subscriptions go through `tauri/events.ts`
   - Types matching Rust backend in `tauri/types.ts`

3. **Shared Code** (`src/shared/`)
   - Generic UI components (Avatar, LoadingSpinner, EmptyState)
   - Utility functions used across multiple features
   - Import via `from '@/shared'` or relative paths

4. **Import Pattern:**
   ```typescript
   // Feature imports
   import { useAccounts, AccountSetupWizard } from './features/accounts';
   import { ConversationView, useConversations } from './features/conversations';

   // Tauri layer
   import { saveAccount, onSyncEvent } from './tauri';
   import type { EmailAccount, SyncStatus } from './tauri';

   // Shared utilities
   import { Avatar, getAvatarColor } from './shared';
   ```

**Backend:**
- New IPC commands go in `src-tauri/src/commands/`
- Register commands in `src-tauri/src/lib.rs`
- Business logic in `src-tauri/src/services/`
- State management in `src-tauri/src/state/`

### Debugging

**Frontend:**
- Open DevTools with `Cmd+Option+I` (macOS) or `Ctrl+Shift+I` (Windows/Linux)

**Backend:**
- Rust logs via `tracing` - check terminal output
- Set `RUST_LOG=debug` for verbose logging

---

## Security & Privacy

eddie.chat is designed with privacy as a core principle:

- **Local-First**: All data processing happens on your machine
- **No Cloud Sync**: Your emails are never uploaded to third-party servers
- **Standard Protocols**: Uses IMAP/SMTP directly with your email provider
- **Keychain Integration**: Support for secure password storage via system keychain
- **Open Source**: Full transparency - audit the code yourself

---

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## License

MIT License - see [LICENSE](LICENSE) for details.

Copyright 2022-2024 eddie.chat contributors.

---

## Roadmap

- [ ] OAuth2 authentication support
- [ ] Agent skills marketplace
- [ ] Collaborative inbox features
- [ ] End-to-end encryption
- [ ] Plugin system for custom workflows
- [ ] Mobile companion apps

---

Built with Rust and React, powered by Tauri.
