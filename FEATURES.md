# Eddie Chat - Complete Feature Documentation

> **Document Purpose**: This document describes WHAT Eddie Chat can do, not HOW it does it.
> For implementation details, see [CLAUDE.md](./CLAUDE.md).

---

## Table of Contents

1. [Overview](#overview)
2. [Email Account Management](#1-email-account-management)
3. [Email Synchronization](#2-email-synchronization)
4. [Conversation Management](#3-conversation-management)
5. [Message Viewing](#4-message-viewing)
6. [Message Composition](#5-message-composition)
7. [Message Actions](#6-message-actions)
8. [Offline Support](#7-offline-support)
9. [Search & Discovery](#8-search--discovery)
10. [Folders & Organization](#9-folders--organization)
11. [Message Classification](#10-message-classification)
12. [Settings & Configuration](#11-settings--configuration)
13. [User Interface](#12-user-interface)
14. [Data & Privacy](#13-data--privacy)
15. [Platform Support](#14-platform-support)

---

## Overview

**Eddie Chat** is an open source messaging client similar to Signal, Messenger or WhatsApp, but without the fragmentation, without the lock-in.

By building on open, decentralized email protocols, Eddie include rather than divide. The backwards-compatible approach means everyone can join the conversation, even those who don't use Eddie.

Eddie aim for feature parity with modern messaging platforms, while keeping privacy at the core. Your data lives only on your device and your chosen email server. Eddie upgrades email into the rich, collaborative, real-time experience of modern messaging, through a lightweight peer-to-peer layer and on-device processing.


### Core Capabilities

- **Conversation-Based Email**: Groups messages by participants, not subject lines
- **Offline-First Design**: Full functionality without internet connection
- **Intelligent Classification**: Automatically identifies chat, newsletters, automated messages, and transactional emails
- **Multi-Account Support**: Manage multiple email accounts in one interface
- **Local-First Privacy**: All data stored locally with encryption
- **Cross-Platform**: Works on iOS, Android, macOS, Windows, and Linux

### Technology Stack

- **App Wrapper**: Tauri 2
- **Frontend**: React 19 with TypeScript
- **Backend**: Rust
- **Database**: SQLite (local cache)
- **Protocols**: IMAP (receiving) + SMTP (sending)

---

## 1. Email Account Management

### Account Setup

**Automated Configuration Discovery**
- Automatically configures email accounts from email address alone, using a clunky autoconfig semi-standard
- Supports major providers: Gmail, Outlook, iCloud, Yahoo, ProtonMail, Fastmail, and more
- Uses multiple discovery methods:
  - Built-in provider database
  - Mozilla Autoconfig (ISPDB)
  - Microsoft Autodiscover v2
  - DNS SRV records
  - MX record analysis
  - Intelligent server probing

**Setup Wizard**
- Guided account creation process
- Email address and password input
- Automatic display name suggestion
- Background autodiscovery with progress indication
- Manual configuration fallback option
- Secure password storage

### Account Management

**Multiple Accounts**
- Add unlimited email accounts
- Switch between accounts instantly
- Set default/active account
- View all accounts in one place

**Account Configuration**
- Edit server settings (IMAP/SMTP host, port, security)
- Configure custom TLS certificates
- Update credentials and display name
- Manage email aliases (for "Sent by me" detection)
- Delete accounts with data cleanup

### Security

**Password Protection**
- Military-grade AES-256-GCM encryption
- Device-specific encryption (passwords tied to hardware)
- Secure key derivation using Argon2
- Encrypted storage in local database
- Support for app-specific passwords

---

## 2. Email Synchronization

### Sync Capabilities

**Intelligent Synchronization**
- Initial sync: Fetches recent messages (default: last 365 days)
- Incremental sync: Fetches only new or changed messages
- Full sync: Re-synchronizes all folders on demand
- Folder-specific sync: Sync individual folders
- Background monitoring: Automatically checks for changes

**Sync Modes**
- **Background polling**: Checks for changes every 60 seconds
- **IDLE support ready**: Infrastructure for push notifications (future)
- **Quick-check optimization**: Skips full sync when no changes detected
- **Offline resilience**: Graceful handling of connection loss

### Local Cache

**Message Storage**
- SQLite database of all synchronized messages
- Complete message metadata (headers, flags, dates)
- Message bodies (text and HTML)
- Attachment metadata and content
- Folder sync state tracking

**Cache Management**
- Automatic cache updates as messages change
- Rebuild conversations from cache
- Drop and resync: Delete cache and start fresh
- Configurable cache age and retention

### Sync Status

**Real-Time Monitoring**
- Current sync state (Idle, Connecting, Syncing, Error)
- Progress information (phase, message counts, percentage)
- Online/offline status
- Pending action count (queued operations)
- Last sync timestamp
- Error messages with details

**Change Notifications**
- New message alerts
- Message deletion notifications
- Flag change updates
- Connection status changes
- Sync completion events

---

## 3. Conversation Management

### Conversation Grouping

**Participant-Based Threading**
- Groups messages by the same set of people
- Considers From, To, and Cc addresses
- Creates stable conversation threads
- Handles multiple participants intelligently

**Email Normalization**
- Gmail: Ignores dots and plus-addressing (user+tag@gmail.com)
- Other providers: Ignores plus-addressing
- Case-insensitive matching
- Consistent participant identification

### Conversation Views

**Conversation Organization**
- **Connections Tab**: Conversations with people you've emailed
- **Others Tab**: Conversations with people you haven't emailed
- **All Tab**: All conversations together

**Classification Filtering**
- Filter by Chat (human conversations)
- Filter by Newsletter (mailing lists)
- Filter by Automated (CI/CD, notifications)
- Filter by Transactional (receipts, shipping)

### Conversation Information

**Displayed Metadata**
- All participants (names and emails)
- Last message preview snippet
- Last message timestamp
- Unread message count
- Total message count
- Message direction (incoming vs outgoing)
- Connection status (is this a connection?)

### Conversation Actions

**Batch Operations**
- Mark entire conversation as read
- View conversation history
- Search within conversations
- Navigate between conversations

---

## 4. Message Viewing

### Message Display

**Content Rendering**
- Plain text and HTML messages
- Safe HTML rendering (sanitized)
- Proper text formatting and line breaks
- Clickable links
- Inline image display (future)

**Message Information**
- Sender name and email
- All recipients (To, Cc, Bcc)
- Timestamps (relative and absolute)
- Message direction indicators
- Read/unread status
- Starred/flagged status
- Attachment indicators

### Message Organization

**Thread View**
- Chronological message ordering
- Date separators between days
- Grouped consecutive messages from same sender
- Expandable/collapsible long messages
- Visual distinction between sent and received

**Full Message View**
- Complete message with all headers
- Copy message content to clipboard
- View raw message data
- See complete recipient lists

### Attachments

**Attachment Viewing**
- List all attachments with message
- File names and sizes
- File type icons
- MIME type information
- Expandable attachment list

**Attachment Actions**
- Download individual attachments
- Download all attachments at once
- Save to Downloads folder or custom location
- Progress indicators during download

---

## 5. Message Composition

### Composing Messages

**Composition Interface**
- Inline compose at bottom of conversation
- Multi-line text input
- Subject line (first line becomes subject)
- Recipient field with autocomplete

**Recipient Management**
- **Autocomplete suggestions**: Type to see contact suggestions
- **Contact search**: Searches names and email addresses
- **Connection priority**: Shows connections first
- **Multiple recipients**: Add multiple people easily
- **Email validation**: Ensures valid email format

### Rich Content

**Emoji Support**
- **Emoji picker**: Browse and select from full emoji library
- **Emoji categories**: Organized by type (smileys, objects, etc.)
- **Emoji search**: Search emojis by name
- **Inline emoji**: Type `:emoji_name:` for suggestions
- **Keyboard shortcuts**: Quick emoji insertion

**Text Formatting**
- Line breaks supported
- Plain text composition
- HTML composition (future)

### Attachments

**Adding Attachments**
- File picker for selecting files
- Multiple file selection
- Drag-and-drop support (future)
- File size display
- Remove attachments before sending

**Attachment Management**
- Preview attached files before sending
- See file names and sizes
- Remove individual attachments
- Support for multiple attachments per message

### Sending

**Message Delivery**
- Send via SMTP
- Automatic save to Sent folder
- Message ID generation
- Delivery confirmation
- Error handling with user feedback

---

## 6. Message Actions

### Reading Actions

**Mark as Read/Unread**
- Mark individual messages
- Mark entire conversations
- Automatic mark-as-read when opening
- Visual unread indicators

**Starring/Flagging**
- Star/flag important messages
- Toggle flag status
- Visual flag indicators
- Search by flagged status (future)

### Message Management

**Delete Messages**
- Delete individual messages
- Bulk delete multiple messages
- Permanent deletion (expunge)
- Trash folder support

**Move and Copy**
- Move messages to different folders
- Copy messages to multiple folders
- Bulk move/copy operations
- Drag-and-drop to folders (future)

### Bulk Operations

**Multi-Message Actions**
- Select multiple messages
- Apply actions to selection
- Bulk flag changes
- Bulk moves and deletions

### Flag System

**IMAP Flags Supported**
- `\Seen`: Read/unread status
- `\Flagged`: Starred/important
- `\Deleted`: Marked for deletion
- `\Answered`: Has been replied to
- `\Draft`: Draft message
- Custom flags (provider-dependent)

---

## 7. Offline Support

### Offline Functionality

**Works Without Internet**
- View all synchronized messages
- Search conversations and messages
- Read message content
- Browse attachments
- Navigate folders

**Offline Actions**
- Compose new messages
- Reply to messages
- Flag/unflag messages
- Mark as read/unread
- Delete messages
- Move/copy messages

### Action Queue

**Queued Operations**
- All actions queued automatically when offline
- Actions saved to local database
- Survives app restarts
- Automatic replay when back online

**Queue Management**
- View pending action count
- Retry failed actions automatically
- Error tracking for troubleshooting
- Manual retry options

**Supported Queued Actions**
- Send messages
- Flag changes (add, remove, set)
- Delete messages
- Move/copy messages
- Save drafts

### Sync Intelligence

**Coming Back Online**
- Automatic reconnection detection
- Queue replay in background
- Conflict resolution
- Error recovery
- Status notifications

---

## 8. Search & Discovery

### Conversation Search

**Search Conversations**
- Real-time search as you type
- Search participant names
- Search message preview text
- Case-insensitive matching
- Instant results from cache

**Search Features**
- Clear search with one click
- Empty state when no results
- Search within filtered views (Connections, Others)
- Combined with classification filters

### Contact Discovery

**Entity/Contact Search**
- Search for people to message
- Autocomplete suggestions
- Fuzzy name matching
- Email address search
- Recent contact priority

**Search Ranking**
- Connections ranked higher
- Most recent contacts first
- Most frequent contacts prioritized
- Configurable result limits

### Future Search Features

- Full-text message body search
- Advanced search filters (date, folder, flags)
- Saved searches
- Search history

---

## 9. Folders & Organization

### Folder Structure

**IMAP Folders**
- List all folders from server
- Support for nested folders
- Folder hierarchy with delimiters
- Special folder recognition

**Standard Folders**
- **INBOX**: Primary inbox
- **Sent**: Sent messages
- **Drafts**: Draft messages
- **Trash**: Deleted messages
- **Archive**: Archived messages
- **Spam/Junk**: Spam folder
- **Custom folders**: User-created folders

### Folder Management

**Folder Operations**
- Create new folders
- Delete folders
- Rename folders (future)
- Move folders (future)

**Folder Actions**
- Move messages to folders
- Copy messages to folders
- Sync specific folders
- Expunge folder (permanent deletion)

### Sync Configuration

**Per-Folder Sync**
- Choose which folders to sync
- Default: INBOX + Sent
- Sync state tracked per folder
- Sync on-demand or automatically

---

## 10. Message Classification

### Automatic Classification

**Message Categories**
- **Chat**: Human-to-human conversations (shown by default)
- **Newsletter**: Newsletters and mailing lists (hidden by default)
- **Automated**: CI/CD, GitHub notifications, alerts (hidden)
- **Transactional**: Receipts, shipping, password resets (hidden)
- **Unknown**: Unclassified messages (shown by default)

### Classification Rules

**Newsletter Detection**
- Known newsletter platforms (Mailchimp, SendGrid, Substack)
- List-Unsubscribe header presence
- Bulk/list mail headers
- Sender patterns

**Automated Message Detection**
- GitHub, GitLab, CircleCI notifications
- No-reply email addresses
- Monitoring and alerting services
- CI/CD systems

**Transactional Detection**
- Subject patterns (receipt, invoice, order, shipping)
- E-commerce sender domains
- Payment and receipt keywords
- Shipping and tracking notifications

### AI-Powered Classification (Ollama)

**Local LLM Classification**
- Optional classification using a local Ollama instance
- Uses configurable model (default: Mistral 3B)
- Structured prompt sends From, Subject, and body preview to the model
- Falls back to rule-based classification if Ollama is unavailable
- Each classification is tagged with a model+prompt hash for tracking

**Automatic Re-classification**
- When Ollama is enabled or the model changes, messages are re-classified
- Only messages that don't match the current model+prompt hash are re-run
- Conversation classifications are rebuilt after re-classification

### Classification Features

**User Controls**
- Filter conversations by classification
- Show/hide non-chat messages
- Configure AI classification via Settings dialog

**Classification Confidence**
- Confidence scores (0.0-1.0)
- Multiple signals per classification
- Conversation-level classification
- Automatic reclassification on new messages

---

## 11. Settings & Configuration

### Settings Dialog

**Accessible from the sidebar gear icon**, the Settings dialog provides:

**Read-Only Mode**
- Protected mode to prevent accidental changes
- Blocks all write operations:
  - Sending messages
  - Flagging/unflagging
  - Deleting messages
  - Moving/copying messages
  - Marking as read/unread
- Visual indicator when enabled
- Toggle on/off easily

**AI Classification (Ollama)**
- Enable/disable LLM-based message classification
- Configure Ollama server URL (default: http://localhost:11434)
- Configure model name (default: mistral:latest)
- Test connection button to verify Ollama is reachable
- Saving with Ollama enabled triggers re-classification of existing messages

### Account Settings

**Connection Configuration**
- IMAP server settings (host, port, security)
- SMTP server settings (host, port, security)
- Custom TLS certificates
- Authentication methods
- Connection timeouts

**Account Information**
- Display name
- Email address
- Email aliases (comma-separated)
- Default account selection

### Sync Settings

**Synchronization Options**
- Initial sync period (days to fetch)
- Folders to synchronize
- Background monitoring enabled/disabled
- Poll interval (seconds)
- Maximum cache age

**Performance Tuning**
- Fetch page size
- Concurrent connection limits
- Retry delays
- Timeout durations

---

## 12. User Interface

### Visual Design

**Modern Interface**
- Clean, minimal design
- Dark theme optimized
- Consistent color system
- Semantic color usage (success, error, warning)
- Accessible contrast ratios

**Avatar System**
- Letter-based avatars for contacts
- Deterministic colors (same person = same color)
- 12-color palette for variety
- Gravatar integration
- Click avatar to view Gravatar profile

### Layout & Navigation

**Responsive Layout**
- Mobile-first design
- Desktop split-view (list + detail)
- Mobile single-view with navigation
- Sidebar auto-hide on mobile
- Back button navigation

**Navigation Features**
- Tab switching (Connections, Others, All)
- Folder navigation (future)
- Keyboard shortcuts
- Search from anywhere

### Interactive Elements

**User Feedback**
- Loading indicators during operations
- Progress bars for sync
- Success/error messages
- Empty states with helpful messages
- Hover effects on clickable items

**Modals & Overlays**
- Account setup wizard
- Account configuration
- Emoji picker
- Gravatar profiles
- Full message view

### Responsive Features

**Mobile Optimizations**
- Touch-friendly tap targets
- Swipe gestures (future)
- Bottom sheet modals
- Safe area support (notches, status bars)
- Optimized font sizes

**Desktop Enhancements**
- Keyboard shortcuts
- Right-click context menus (future)
- Resizable panels (future)
- Multi-window support (future)

---

## 13. Data & Privacy

### Local-First Architecture

**Data Storage**
- All data stored locally on device
- No cloud storage or sync to external servers
- SQLite databases for cache and config
- Platform-specific app data directories

**Privacy Benefits**
- Complete data ownership
- No third-party access
- No analytics or tracking
- No data sharing
- Full offline access

### Security

**Password Encryption**
- AES-256-GCM encryption
- Device-specific encryption keys
- Keys derived from hardware ID + OS username
- Argon2 key derivation function
- Random nonces per encryption

**Data Protection**
- Encrypted credentials at rest
- Secure password entry
- No password logging
- Secure credential deletion
- Protection against unauthorized access

### Data Management

**User Control**
- Delete accounts and all data
- Clear cache and resync
- Export data (future)
- Import from other clients (future)

**Storage Locations**
- Config database: Platform app data directory
- Sync database: Platform app data directory
- Debug mode: Project directory (development only)
- Attachments: System Downloads folder

---

## 14. Platform Support

### Desktop Platforms

**macOS**
- Native macOS application
- Menu bar integration
- System notifications
- Keychain integration (future)
- Auto-updates (future)

**Windows**
- Native Windows application
- System tray support
- Windows notifications
- Credential Manager integration (future)
- Auto-updates (future)

**Linux**
- Native Linux application
- XDG standards compliance
- Desktop notifications
- Secret Service integration (future)
- Distribution packages (future)

### Cross-Platform Features

**Consistent Experience**
- Same features across all platforms
- Shared codebase (React + Rust)
- Platform-specific optimizations
- Native look and feel

**Platform Integration**
- System notifications
- Native file pickers
- Default email client integration (future)
- Deep linking (future)

### Future Platform Support

**Mobile Platforms**
- iOS (infrastructure present)
- Android (infrastructure present)
- Mobile-optimized UI
- Background sync
- Push notifications

---

## Feature Summary

Eddie Chat provides a complete email experience with:

✅ **Multi-account management** with automatic configuration
✅ **Intelligent conversation grouping** by participants
✅ **Offline-first design** with action queue
✅ **Automatic message classification** (chat, newsletter, automated, transactional)
✅ **Full-text search** of conversations and contacts
✅ **Rich message composition** with emoji and attachments
✅ **Complete message management** (read, flag, delete, move, copy)
✅ **Secure local storage** with encrypted credentials
✅ **Cross-platform support** (macOS, Windows, Linux)
✅ **Modern, responsive UI** optimized for desktop and mobile
✅ **Real-time sync** with background monitoring

---

## Version Information

This documentation reflects Eddie Chat as of **February 2026**.

For implementation details and architecture, see [CLAUDE.md](./CLAUDE.md).
For changelog and release history, see [CHANGELOG.md](./CHANGELOG.md).

---

**Last Updated**: 2026-02-06
