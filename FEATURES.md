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

## On-Device LLM Inference

**Multi-Backend Model Discovery**
- Discover available LLM models from all backends in a single call
- Supports OS-native models and Ollama-hosted models
- Each model reports availability status and readiness
- Metadata includes model family, parameter size, and quantization level

**Supported Backends**
- **Apple Foundation Model**: On-device inference via Apple Intelligence (macOS 26+, iOS 26+)
- **Gemini Nano**: On-device inference via Android ML Kit (supported Pixel/Galaxy devices)
- **Phi Silica**: On-device inference via Windows Copilot+ NPU
- **Ollama**: Local or remote server supporting any Ollama-compatible model

**Text Generation**
- Generate text from any available model
- Configurable temperature and max token limits
- Automatic routing to the correct backend based on model ID
- Response includes model and provider information

**Ollama Configuration**
- Hot-swap Ollama server connection without app restart
- Configure server URL, API key, and timeout
- Supports reverse proxies (LiteLLM, OpenWebUI, nginx auth)
- Disable Ollama entirely by setting URL to null
- In-flight requests complete against prior configuration

**Privacy**
- OS-native models run entirely on-device
- No data sent to external servers for native inference
- Ollama can run locally on the same machine
- User controls which servers receive their data
