/**
 * Types that mirror the Rust backend types.
 * This file serves as the contract between frontend and backend.
 */

// ========== Email Display Types ==========
// These types are used by frontend components for rendering.
// They may differ from backend types - hooks do the conversion.

export interface Envelope {
  id: string;
  message_id?: string;
  in_reply_to?: string;
  from: string;
  to: string[];
  cc: string[];
  subject: string;
  date: string;
  flags: string[];
  has_attachment: boolean;
}

export interface ChatMessage {
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

// ========== Account Types ==========

export interface EmailAccount {
  name: string;
  is_default: boolean;
  backend: string;
}

export interface EmailAccountDetails {
  name: string;
  email: string;
  display_name?: string;
  aliases?: string;
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

export interface SaveEmailAccountRequest {
  name: string;
  email: string;
  display_name?: string;
  aliases?: string;
  imap_host: string;
  imap_port: number;
  imap_tls: boolean;
  imap_tls_cert?: string;
  smtp_host: string;
  smtp_port: number;
  smtp_tls: boolean;
  smtp_tls_cert?: string;
  username: string;
  password?: string;
}

export interface SaveDiscoveredEmailAccountRequest {
  name: string;
  email: string;
  displayName?: string;
  imapHost: string;
  imapPort: number;
  imapTls: boolean;
  smtpHost: string;
  smtpPort: number;
  smtpTls: boolean;
  authMethod: string;
  password?: string;
}

// ========== Conversation Types ==========

/**
 * Conversation as returned by the backend (adapters::sqlite::conversations::Conversation).
 * Used directly in hooks and components.
 */
export interface Conversation {
  id: string;
  account_id: string;
  participant_key: string;
  /** JSON string: Record<email, display_name> */
  participant_names: string | null;
  classification: string;
  /** Epoch milliseconds */
  last_message_date: number;
  last_message_preview: string | null;
  unread_count: number;
  total_count: number;
  is_muted: boolean;
  is_pinned: boolean;
  is_important: boolean;
  /** Epoch milliseconds */
  updated_at: number;
  // Frontend-computed display helpers (set by hooks)
  /** Parsed participant emails (excluding self) */
  participants: string[];
  /** Parsed participant display names (excluding self) */
  participant_display_names: string[];
}

// ========== Sync Engine Types ==========

export interface SyncStatus {
  state: string;
  account_id: string;
  current_folder: string | null;
  progress_current: number | null;
  progress_total: number | null;
  progress_message: string | null;
  last_sync: string | null;
  error: string | null;
  is_online: boolean;
  pending_actions: number;
}

/**
 * Message as returned by the backend (adapters::sqlite::messages::Message).
 */
export interface Message {
  id: string;
  /** Epoch milliseconds */
  date: number;
  from_address: string;
  from_name: string | null;
  /** JSON string array */
  to_addresses: string;
  /** JSON string array */
  cc_addresses: string;
  subject: string | null;
  body_text: string | null;
  body_html: string | null;
  has_attachments: boolean;
  /** JSON string array of IMAP flags */
  imap_flags: string;
  distilled_text: string | null;
}

// ========== Message Compose Types ==========

export interface ComposeAttachment {
  path: string;
  name: string;
  mime_type: string;
  size: number;
}

export interface ComposeMessageData {
  from?: string;
  to: string[];
  cc?: string[];
  subject: string;
  body: string;
  in_reply_to?: string;
  attachments?: ComposeAttachment[];
}

export interface SendMessageResult {
  message_id: string;
  sent_folder: string;
}

// ========== Attachment Types ==========

export interface AttachmentInfo {
  index: number;
  filename: string;
  mime_type: string;
  size: number;
}

// ========== Email Discovery Types ==========

export interface DiscoveryResult {
  provider?: string;
  provider_id?: string;
  imap_host: string;
  imap_port: number;
  imap_tls: boolean;
  smtp_host: string;
  smtp_port: number;
  smtp_tls: boolean;
  auth_method: string;
  requires_app_password: boolean;
  username_hint: string;
  source: string;
}

// ========== Sync Event Types ==========

export interface SyncEventPayload {
  StatusChanged?: SyncStatus;
  NewMessages?: { folder: string; count: number };
  MessagesDeleted?: { folder: string; uids: number[] };
  FlagsChanged?: { folder: string; uids: number[] };
  ConversationsUpdated?: { conversation_ids: string[] };
  Error?: { message: string };
  SyncComplete?: Record<string, never>;
}
