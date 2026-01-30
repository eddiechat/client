/**
 * Types that mirror the Rust backend types.
 * This file serves as the contract between frontend and backend.
 */

// ========== Email Core Types ==========

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
  /** Internal: cached conversation ID for database operations */
  _cached_id?: number;
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

export interface CachedConversation {
  id: number;
  participant_key: string;
  participants: { email: string; name: string | null }[];
  last_message_date: string | null;
  last_message_preview: string | null;
  last_message_from: string | null;
  message_count: number;
  unread_count: number;
  is_outgoing: boolean;
}

export interface CachedChatMessage {
  id: number;
  folder: string;
  uid: number;
  message_id: string | null;
  from_address: string;
  from_name: string | null;
  to_addresses: string[];
  cc_addresses: string[];
  subject: string | null;
  date: string | null;
  flags: string[];
  has_attachment: boolean;
  text_body: string | null;
  html_body: string | null;
  body_cached: boolean;
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

export type SyncActionType = "add_flags" | "remove_flags" | "delete" | "move" | "copy";

export interface SyncEventPayload {
  StatusChanged?: SyncStatus;
  NewMessages?: { folder: string; count: number };
  MessagesDeleted?: { folder: string; uids: number[] };
  FlagsChanged?: { folder: string; uids: number[] };
  ConversationsUpdated?: { conversation_ids: number[] };
  Error?: { message: string };
  SyncComplete?: null;
}
