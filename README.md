<p align="center">
  <img src="public/eddie-swirl-green.svg" alt="eddie logo" width="120" height="120">
</p>

# eddie

**eddie** is a modern, lightweight email client that reimagines email as a conversation-first experience. Inspired by the clean aesthetics of Signal, eddie groups your emails into threaded conversations by participant, making email feel as natural as messaging.

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
│                       Client Application                    │
├─────────────────────────┬───────────────────────────────────┤
│     React Frontend      │           Rust Backend            │
│     (TypeScript)        │            (Tauri v2)             │
├─────────────────────────┼───────────────────────────────────┤
│  • File-based Routing   │  • Sync Engine (worker loop)      │
│  • Context Providers    │  • Command Handlers               │
│  • Tauri IPC Client     │  • IMAP/SMTP Adapters             │
│  • Skills UI            │  • SQLite Cache + Classification  │
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
| **Styling** | Tailwind CSS 4 | Utility-first CSS |
| **Routing** | TanStack Router | File-based routing with hash history |
| **Build Tool** | Vite 7 | Fast development and bundling |
| **Client Runtime** | Tauri v2 | Cross-platform native shell |
| **Backend** | Rust (Edition 2021) | Core email operations |
| **Async Runtime** | Tokio | Non-blocking I/O |
| **Email Protocol** | async-imap + mailparse | IMAP protocol and message parsing |
| **TLS** | tokio-rustls | Secure connections |
| **Database** | SQLite (rusqlite + r2d2) | Local email cache and settings |
| **AI** | Ollama (optional) | Local LLM for skill classification |

### Data Flow

```
User Action → React Route → Tauri invoke() → Rust Command Handler
                                                      │
                                                      ▼
                                              Services / Adapters
                                                      │
                                          ┌───────────┴───────────┐
                                          ▼                       ▼
                                    SQLite Cache          IMAP/SMTP Server
                                          │
                                          ▼
                                  Tauri Event Emission
                                          │
User Interface ← React Context Update ←───┘
```

### Core Components

#### Frontend (`src/`)

The frontend uses **file-based routing** with TanStack Router and React Context for state:

| Directory | Purpose |
|-----------|---------|
| `routes/` | File-based route definitions (TanStack Router conventions) |
| `shared/` | Reusable components, context providers, and utility functions |
| `skills/` | Skill UI components (SkillsHub, SkillStudio) |
| `tauri/` | Centralized Tauri communication layer (commands, events, types) |

##### Routes (`routes/`)

Route files follow TanStack Router conventions:
- `__root.tsx` — Root layout
- `_app.tsx` — Auth guard (`beforeLoad`)
- `_app/_tabs.tsx` — Tab layout (header, tabs, account drawer)
- `_app/_tabs/points.tsx`, `circles.tsx`, `lines.tsx` — Tab routes
- `_app/conversation.$id.tsx` — Conversation detail
- `_app/cluster.$id.tsx` — Cluster detail
- `_app/settings.tsx` — Settings screen
- `_app/skills.hub.tsx`, `skills.studio.tsx` — Skills routes
- `login.tsx`, `onboarding.tsx` — Unauthenticated routes

##### Tauri Layer (`tauri/`)

All Tauri communication is centralized for type safety and maintainability:

| File | Purpose |
|------|---------|
| `commands.ts` | Type-safe wrappers for all `invoke()` calls to Rust backend |
| `events.ts` | Event listener subscriptions (sync status, conversations updated) |
| `types.ts` | TypeScript types mirroring Rust backend types |
| `index.ts` | Barrel exports for clean imports |

##### Shared (`shared/`)

| Directory | Purpose |
|-----------|---------|
| `components/` | Generic UI components (Avatar, Icons, ErrorFallback, etc.) |
| `context/` | Global state providers (AuthContext, DataContext, SearchContext, ThemeContext) |
| `lib/` | Utility functions (helpers, gravatar) |

#### Backend (`src-tauri/src/`)

| Module | Purpose |
|--------|---------|
| `adapters/` | External service bridges — IMAP protocol, SQLite persistence, Ollama AI |
| `commands/` | Thin Tauri command wrappers exposed to frontend |
| `services/` | Business logic — sync engine (worker, helpers, tasks), Ollama, logger |
| `autodiscovery/` | Email provider auto-configuration (autoconfig, DNS, probing) |
| `error.rs` | `EddieError` enum for all error returns |

---

## Project Structure

```
eddie.chat/
├── src/                              # React/TypeScript frontend
│   ├── main.tsx                      # Entry point (providers + router)
│   ├── router.tsx                    # TanStack Router config (hash history)
│   │
│   ├── routes/                       # File-based routing (TanStack Router)
│   │   ├── __root.tsx                # Root layout
│   │   ├── login.tsx                 # Login screen
│   │   ├── onboarding.tsx            # Onboarding screen
│   │   ├── _app.tsx                  # Auth guard (beforeLoad)
│   │   ├── _app/_tabs.tsx            # Tab layout
│   │   ├── _app/_tabs/              # Tab routes
│   │   │   ├── points.tsx            # Points tab (connections)
│   │   │   ├── circles.tsx           # Circles tab (clusters)
│   │   │   └── lines.tsx             # Lines tab (automated)
│   │   ├── _app/conversation.$id.tsx # Conversation detail
│   │   ├── _app/cluster.$id.tsx      # Cluster detail
│   │   ├── _app/settings.tsx         # Settings screen
│   │   └── _app/skills.*.tsx         # Skills routes
│   │
│   ├── shared/                       # Shared utilities & components
│   │   ├── components/               # Generic UI components
│   │   │   ├── Avatar.tsx
│   │   │   ├── Icons.tsx
│   │   │   ├── ErrorFallback.tsx
│   │   │   ├── MessageDetail.tsx
│   │   │   ├── OnboardingScreen.tsx
│   │   │   └── index.ts
│   │   ├── context/                  # Global state providers
│   │   │   ├── AuthContext.tsx
│   │   │   ├── DataContext.tsx
│   │   │   ├── SearchContext.tsx
│   │   │   ├── ThemeContext.tsx
│   │   │   └── index.ts
│   │   ├── lib/                      # Utility functions
│   │   │   ├── helpers.ts
│   │   │   ├── gravatar.ts
│   │   │   └── index.ts
│   │   └── index.ts
│   │
│   ├── skills/                       # Skill UI components
│   │   ├── SkillsHub.tsx
│   │   ├── SkillStudio.tsx
│   │   ├── types.ts
│   │   └── index.ts
│   │
│   └── tauri/                        # Tauri integration layer
│       ├── commands.ts               # Type-safe invoke wrappers
│       ├── events.ts                 # Event listener subscriptions
│       ├── types.ts                  # Backend contract types
│       └── index.ts                  # Barrel exports
│
├── src-tauri/                        # Rust backend
│   ├── src/
│   │   ├── main.rs                   # Binary entry point
│   │   ├── lib.rs                    # Tauri setup, state init, worker spawn
│   │   ├── error.rs                  # EddieError enum
│   │   ├── commands/                 # Tauri command handlers
│   │   │   ├── account.rs            # Account connect/lookup
│   │   │   ├── conversations.rs      # Conversation & cluster queries
│   │   │   ├── sync.rs               # Sync control & onboarding status
│   │   │   ├── classify.rs           # Message reclassification
│   │   │   ├── discovery.rs          # Email autodiscovery
│   │   │   ├── skills.rs             # Skill CRUD
│   │   │   ├── settings.rs           # App settings & Ollama models
│   │   │   ├── ollama.rs             # Ollama LLM completion
│   │   │   ├── app.rs                # App metadata (version)
│   │   │   └── mod.rs
│   │   ├── services/                 # Business logic
│   │   │   ├── sync/                 # Sync engine
│   │   │   │   ├── worker.rs         # Tick loop (15s interval)
│   │   │   │   ├── helpers/          # Processing utilities
│   │   │   │   └── tasks/            # Onboarding & recurring tasks
│   │   │   ├── ollama.rs             # Ollama model discovery
│   │   │   ├── logger.rs             # Structured logging
│   │   │   └── mod.rs
│   │   ├── adapters/                 # External service bridges
│   │   │   ├── imap/                 # IMAP protocol (async-imap)
│   │   │   │   ├── connection.rs     # TCP + TLS + LOGIN
│   │   │   │   ├── envelopes.rs      # Message envelope fetching
│   │   │   │   ├── folders.rs        # Folder discovery & classification
│   │   │   │   ├── historical.rs     # Historical message fetch
│   │   │   │   └── sent_scan.rs      # Sent folder scanning
│   │   │   ├── sqlite/               # SQLite persistence
│   │   │   │   └── sync/             # Sync database
│   │   │   │       ├── db.rs         # Connection pool init
│   │   │   │       ├── db_schema.rs  # Schema & migrations
│   │   │   │       ├── messages.rs   # Message CRUD
│   │   │   │       ├── conversations.rs # Conversation materialization
│   │   │   │       ├── entities.rs   # Trust network
│   │   │   │       ├── folder_sync.rs # IMAP sync cursors
│   │   │   │       ├── skills.rs     # Skill persistence
│   │   │   │       ├── skill_classify.rs # Skill classification
│   │   │   │       ├── settings.rs   # App settings
│   │   │   │       └── onboarding_tasks.rs
│   │   │   └── ollama/               # Ollama AI adapter
│   │   └── autodiscovery/            # Email provider detection
│   │       ├── autoconfig.rs         # Mozilla autoconfig
│   │       ├── dns.rs                # DNS SRV/MX lookup
│   │       ├── providers.rs          # Known provider database
│   │       └── probe.rs              # Server probing
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
3. Launch the application with hot reload

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

Stable releases are created using the `/release` skill in [Claude Code](https://docs.anthropic.com/en/docs/claude-code). The skill handles the full release workflow:

1. Prompts you to choose a version bump (patch, minor, or major)
2. Generates a categorized changelog entry from commits since the last tag
3. Lets you review and edit the changelog before committing
4. Updates version numbers in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`
5. Commits, pushes, and creates a git tag

Once the tag is pushed, GitHub Actions automatically builds and publishes releases for all platforms.

You can also create a release manually by pushing a version tag:

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

Note that the builds aren't signed, for macOS run the following command after installation.

`xattr -cr /Applications/eddie.chat.app`

---

## Configuration

Accounts are configured through the application's setup wizard, which includes email autodiscovery for automatic IMAP/SMTP server detection. Account credentials are stored locally in an encrypted SQLite database.

App settings (theme, Ollama model preferences, etc.) are stored in a key-value `settings` table in the same local database.

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
- **Encrypted Storage**: Account credentials encrypted with device-specific keys
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
