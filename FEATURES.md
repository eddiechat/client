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

## Skills

**My Skills (Skills Hub)**
- View all created skills with name, icon, and enabled/disabled status
- Toggle skills on/off directly from the list
- Navigate to Skill Studio to create new or edit existing skills
- Skills are persisted locally in SQLite database per account

**Skill Studio (Create / Edit / Delete)**
- Create new skills with name, icon, classification prompt, and modifiers
- Edit existing skills (name, prompt, modifiers, settings)
- Quick modifiers: exclude newsletters, only known senders, has attachments, recent 6 months, exclude auto-replies
- Settings: auto-archive matched emails, notify on new matches
- Delete skills from the settings tab
- Preview tab placeholder for future classification testing
