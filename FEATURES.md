# Eddie Chat — Features

## Email Autodiscovery

**Automatic Server Detection**
- Automatically discovers IMAP and SMTP server settings from an email address
- Multi-step login wizard: enter email → auto-detect → enter password → connect
- Falls back to manual configuration if autodiscovery fails

**Supported Discovery Methods**
- Known provider lookup (instant) for Gmail, Outlook, Yahoo, iCloud, Fastmail, ProtonMail, and more
- Mozilla Autoconfig (ISPDB) and domain-hosted autoconfig
- Microsoft Autodiscover v2 (Office 365 and on-premises Exchange)
- DNS SRV records (RFC 6186 / RFC 8314)
- MX record analysis for provider detection
- Heuristic server probing as last resort

**Provider-Specific Guidance**
- Detects when app-specific passwords are required (Gmail, iCloud, Yahoo, etc.)
- Provides direct links to generate app passwords for supported providers
- Shows detected provider name during login

**Manual Configuration Fallback**
- IMAP host, port, and TLS settings
- SMTP host, port, and TLS settings
- Switch between auto-detected and manual settings at any time

## Email Classification

**Two-Step Classification Pipeline**
- Deterministic rules engine handles ~60–70% of messages instantly (zero-cost)
- ONNX neural model classifies ambiguous messages (~5–15ms per message)
- Every message is classified as Chat or Not Chat

**Deterministic Rules**
- Gmail category labels (Promotions, Updates, Forums, Social)
- IMAP folder patterns (newsletters, spam, promotions)
- Known email service provider (ESP) sending domains
- Automated sender prefixes (noreply, newsletter, alerts, etc.)
- Unsubscribe text detection in message body
- Mailing list subject tags
- Mass recipient count (>5 To+CC)
- Thread context (In-Reply-To + References headers → Chat)

**Neural Model (ONNX)**
- INT8 quantized DistilBERT-based model downloaded on first use
- Onboarding screen shows download progress with percentage
- Processes subject + body text with head+tail truncation (512 tokens max)
- Uses 12 metadata features (reply status, recipient count, sender type, etc.)
- Loaded once after download for fast per-message inference

**Reclassification**
- Reclassify all messages for an account on demand

## Email Views

**Chats (Points tab)**
- Shows conversations with senders in your trust network
- Groups messages by conversation (participant set)
- Long-press to move a conversation to Requests

**Requests (Requests tab)**
- Shows conversations from senders not yet in your trust network
- Allows discovering messages from new people reaching out

**Groups (Circles tab)**
- Placeholder for future group/circle functionality

**Skill Studio (Create / Edit / Delete)**
- Create new skills with name, icon, classification prompt, and modifiers
- Edit existing skills (name, prompt, modifiers, settings)
- Quick modifiers: exclude newsletters, only known senders, has attachments, recent 6 months, exclude auto-replies
- Settings: auto-archive matched emails, notify on new matches
- Delete skills from the settings tab
- Preview tab placeholder for future classification testing

## Message Composition

**Compose New Messages**
- Compose new messages via the FAB button on the main screen
- Tokenized recipient input with type-ahead entity search
- Search matches contacts by email and display name
- From-address selector cycles through account aliases
- Navigates to conversation view for typing and sending the first message

**Reply to Messages**
- Reply to any received message with a single tap
- Shows quoted preview above the compose input (sender name + truncated body)
- Automatically sets In-Reply-To and References headers for proper threading
- Subject automatically prefixed with "Re:" when replying
- Reply quote blocks shown inline in message bubbles

**Send via SMTP**
- Sends emails through the configured SMTP server
- Supports STARTTLS (port 587) and implicit TLS (port 465)
- Automatically saves sent messages to the Sent folder via IMAP APPEND
- Optimistic message insertion — sent messages appear instantly in the conversation

## Mark as Read

**Automatic Read Tracking**
- Messages are automatically marked as read after 1 second of visibility
- Uses IntersectionObserver to detect when message bubbles are on screen
- Optimistic local flag update for instant UI feedback
- Queues IMAP STORE command to set \Seen flag on the server
- Server-wins conflict resolution via periodic flag resync

## Action Queue

**Offline-First Mutations**
- All write operations (mark as read, send email) go through a persistent action queue
- Actions are stored in SQLite and survive app restarts
- Replay worker processes pending actions before each sync cycle
- Failed actions are retried up to 5 times with error tracking
- Completed actions are cleaned up automatically

## Read-Only Mode

**Mailbox Protection**
- Read-only toggle in Settings > Privacy (defaults ON)
- When enabled, IMAP connections use EXAMINE (read-only) instead of SELECT
- Prevents any modifications to the remote mailbox
- Write actions queue locally but will fail gracefully on replay

## Account Management

**Account Settings**
- Expandable account card in Settings shows current configuration
- Edit display name, password, IMAP/SMTP server settings
- Configure TLS settings for both IMAP and SMTP connections
- Manage email aliases (comma-separated)
- Changes saved immediately to the local database

## Entity Display Names

**Contact Name Resolution**
- Automatically populates entity display names from incoming message headers
- Names extracted from the From header of received emails
- Used in compose autocomplete and contact suggestions
- Entity search matches by both email address and display name
