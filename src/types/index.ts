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
  bcc?: string[];
  subject: string;
  body: string;
  reply_to?: string;
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
