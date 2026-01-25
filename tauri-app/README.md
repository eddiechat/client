# Himalaya Tauri

A desktop email client built with Tauri, React, and TypeScript. This is the GUI version of the [Himalaya CLI](https://github.com/pimalaya/himalaya) email client.

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [pnpm](https://pnpm.io/) package manager
- [Rust](https://www.rust-lang.org/tools/install) (nightly toolchain required)

### Installing Rust Nightly

The project requires Rust nightly due to imap-codec dependency. Install it with:

```bash
rustup install nightly
```

The project includes a `rust-toolchain.toml` that automatically selects nightly when building.

## Installation

```bash
# Install frontend dependencies
pnpm install
```

## Development

```bash
# Run in development mode with hot reload
pnpm tauri dev
```

## Building

```bash
# Build the application
pnpm tauri build

# Build without bundling (faster, outputs binary only)
pnpm tauri build --no-bundle
```

The built application will be located at `src-tauri/target/release/himalaya-tauri`.

## Project Structure

### Backend (Rust) - `src-tauri/src/`

| Path | Description |
|------|-------------|
| `lib.rs` | Main entry point, registers all Tauri commands |
| `commands/` | Tauri command handlers |
| `commands/accounts.rs` | Account management (list, get default, check exists) |
| `commands/folders.rs` | Folder operations (list, create, delete, expunge) |
| `commands/envelopes.rs` | Email envelope listing and threading |
| `commands/messages.rs` | Message operations (read, delete, copy, move, send, save) |
| `commands/flags.rs` | Flag management (add, remove, set, mark read/unread) |
| `commands/config.rs` | Configuration initialization and management |
| `types/` | Serializable types for frontend communication |
| `types/error.rs` | Error types for the application |
| `config/` | Configuration management using pimalaya-tui |

### Frontend (React/TypeScript) - `src/`

| Path | Description |
|------|-------------|
| `lib/api.ts` | Tauri invoke wrappers for all backend commands |
| `hooks/useEmail.ts` | React hooks for state management |
| `types/index.ts` | TypeScript interfaces mirroring Rust types |
| `components/FolderList.tsx` | Sidebar folder navigation |
| `components/EnvelopeList.tsx` | Email list view |
| `components/MessageView.tsx` | Email reader panel |
| `components/AccountSelector.tsx` | Account dropdown selector |
| `components/ComposeModal.tsx` | Email composition modal |
| `App.tsx` | Main application layout |
| `App.css` | Application styles (dark theme) |

## Configuration

The application uses the same configuration format as the Himalaya CLI. Place your configuration file at:

- Linux/macOS: `~/.config/himalaya/config.toml`
- Windows: `%APPDATA%\himalaya\config.toml`

See the [Himalaya configuration documentation](https://pimalaya.org/himalaya/cli/latest/configuration/) for details.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Tech Stack

- **Frontend:** React 19, TypeScript, Vite
- **Backend:** Rust, Tauri 2.x
- **Email:** pimalaya ecosystem (email-lib, pimalaya-tui, mml-lib, secret-lib)
- **Supported backends:** IMAP, Maildir, Notmuch, SMTP, Sendmail
