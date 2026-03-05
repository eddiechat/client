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
