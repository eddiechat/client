// Types that mirror the Rust backend types

export interface Envelope {
  id: string;
  message_id?: string;
  in_reply_to?: string;
  from: string;
  to: string[];
  subject: string;
  date: string;
  flags: string[];
  has_attachment: boolean;
}

export interface Message {
  id: string;
  envelope: Envelope;
  headers: [string, string][];
  text_body?: string;
  html_body?: string;
  attachments: Attachment[];
}

export interface Attachment {
  filename?: string;
  mime_type: string;
  size: number;
}

export interface Account {
  name: string;
  is_default: boolean;
  backend: string;
}

export interface AccountDetails {
  name: string;
  email: string;
  display_name?: string;
  imap_host: string;
  imap_port: number;
  imap_tls: boolean;
  imap_tls_cert?: string;
  smtp_host: string;
  smtp_port: number;
  smtp_tls: boolean;
  smtp_tls_cert?: string;
  username: string;
}

export interface ComposeMessageData {
  from?: string;
  to: string[];
  cc?: string[];
  subject: string;
  body: string;
  in_reply_to?: string;
}

// Account configuration for saving new accounts
export interface SaveAccountRequest {
  name: string;
  email: string;
  display_name?: string;
  imap_host: string;
  imap_port: number;
  imap_tls: boolean;
  imap_tls_cert?: string;
  smtp_host: string;
  smtp_port: number;
  smtp_tls: boolean;
  smtp_tls_cert?: string;
  username: string;
  password: string;
}

// Conversation type for Signal-like messaging UI
export interface Conversation {
  id: string;
  participants: string[];
  participant_names: string[];
  last_message_date: string;
  last_message_preview: string;
  last_message_from: string;
  unread_count: number;
  message_ids: string[];
  is_outgoing: boolean;
  user_name: string;
  user_in_conversation: boolean;
}

// Email autodiscovery result
export interface DiscoveryResult {
  /** Provider name (if detected) */
  provider?: string;
  /** Provider ID for known providers */
  provider_id?: string;
  /** IMAP host */
  imap_host: string;
  /** IMAP port */
  imap_port: number;
  /** Whether IMAP uses TLS */
  imap_tls: boolean;
  /** SMTP host */
  smtp_host: string;
  /** SMTP port */
  smtp_port: number;
  /** Whether SMTP uses TLS */
  smtp_tls: boolean;
  /** Authentication method: "password", "oauth2", "app_password" */
  auth_method: string;
  /** OAuth provider if OAuth2: "google", "microsoft", "yahoo", "fastmail" */
  oauth_provider?: string;
  /** Whether app-specific password is required */
  requires_app_password: boolean;
  /** Username format hint */
  username_hint: string;
  /** Discovery source for debugging */
  source: string;
}

// OAuth token status
export interface OAuthStatus {
  has_tokens: boolean;
  needs_refresh: boolean;
  is_expired: boolean;
}
