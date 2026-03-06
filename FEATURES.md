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
