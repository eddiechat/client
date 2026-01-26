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

---

## Features

- **Conversation Threading** - Emails grouped by participants, not just subject lines
- **Multi-Account Support** - Manage multiple email accounts with easy switching
- **Modern Dark UI** - Signal-inspired design with green accents
- **Full IMAP/SMTP Support** - Works with any standard email provider
- **Draft Auto-Save** - Never lose your in-progress messages
- **Gravatar Integration** - Automatic avatars for your contacts
- **Folder Management** - Create, delete, and organize folders
- **Message Operations** - Read/unread, flag, delete, move, and copy
- **Search** - Quickly find conversations by name or content
- **Cross-Platform** - Runs on macOS, Windows, and Linux

---

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

| Directory | Purpose |
|-----------|---------|
| `components/` | React UI components (ChatList, ConversationView, ComposeModal, etc.) |
| `hooks/` | Custom React hooks for data fetching (`useEmail`, `useConversations`) |
| `lib/` | API wrapper for Tauri IPC and utility functions |
| `types/` | TypeScript interfaces for Email, Message, Account, etc. |

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
├── src/                          # React/TypeScript frontend
│   ├── App.tsx                   # Main application component
│   ├── App.css                   # Global styles (dark theme)
│   ├── main.tsx                  # React entry point
│   ├── components/               # UI components
│   │   ├── AccountConfigModal.tsx
│   │   ├── Avatar.tsx
│   │   ├── ChatList.tsx
│   │   ├── ComposeModal.tsx
│   │   └── ConversationView.tsx
│   ├── hooks/                    # React hooks
│   │   ├── useEmail.ts
│   │   └── useConversations.ts
│   ├── lib/                      # Utilities
│   │   ├── api.ts               # Tauri IPC wrapper
│   │   └── utils.ts             # Helper functions
│   └── types/                    # TypeScript definitions
│       └── index.ts
│
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── main.rs              # Application entry point
│   │   ├── lib.rs               # Tauri initialization
│   │   ├── backend/             # Email operations
│   │   │   └── mod.rs
│   │   ├── commands/            # IPC command handlers
│   │   │   ├── accounts.rs
│   │   │   ├── conversations.rs
│   │   │   ├── messages.rs
│   │   │   ├── envelopes.rs
│   │   │   ├── flags.rs
│   │   │   ├── folders.rs
│   │   │   └── config.rs
│   │   ├── config/              # Configuration management
│   │   │   └── mod.rs
│   │   └── types/               # Data structures
│   │       ├── mod.rs
│   │       ├── conversation.rs
│   │       └── error.rs
│   ├── Cargo.toml               # Rust dependencies
│   ├── tauri.conf.json          # Tauri configuration
│   └── icons/                   # Application icons
│
├── package.json                  # Frontend dependencies
├── vite.config.ts               # Vite configuration
├── tsconfig.json                # TypeScript configuration
└── index.html                   # HTML entry point
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

**Frontend:**
- Components in `src/components/`
- Data fetching via custom hooks in `src/hooks/`
- Tauri IPC calls wrapped in `src/lib/api.ts`
- Shared types in `src/types/`

**Backend:**
- New IPC commands go in `src-tauri/src/commands/`
- Register commands in `src-tauri/src/lib.rs`
- Email operations extend `src-tauri/src/backend/mod.rs`

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
