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

export interface Folder {
  name: string;
  desc?: string;
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

export interface ListEnvelopesRequest {
  account?: string;
  folder?: string;
  page?: number;
  page_size?: number;
  query?: string;
}

export interface ListEnvelopesResponse {
  envelopes: Envelope[];
  page: number;
  page_size: number;
  total?: number;
}

export interface ReadMessageRequest {
  account?: string;
  folder?: string;
  id: string;
  preview: boolean;
}

export interface FlagRequest {
  account?: string;
  folder?: string;
  ids: string[];
  flags: string[];
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

// Common flag names
export const FLAGS = {
  SEEN: 'seen',
  ANSWERED: 'answered',
  FLAGGED: 'flagged',
  DELETED: 'deleted',
  DRAFT: 'draft',
} as const;

export type FlagName = typeof FLAGS[keyof typeof FLAGS];
