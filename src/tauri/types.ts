export type SyncStatus = {
  phase: string;
  message: string;
};

export type ConversationsUpdated = {
  account_id: string;
  count: number;
};

export type Conversation = {
  id: string;
  account_id: string;
  participant_key: string;
  participant_names: string | null;
  classification: string;
  last_message_date: number;
  last_message_preview: string | null;
  last_message_is_sent: boolean;
  last_message_from_name: string | null;
  unread_count: number;
  total_count: number;
  is_muted: boolean;
  is_pinned: boolean;
  is_important: boolean;
  updated_at: number;
  initial_sender_email: string | null;
};

export type Message = {
  id: string;
  date: number;
  from_address: string;
  from_name: string | null;
  to_addresses: string;
  cc_addresses: string;
  subject: string | null;
  body_text: string | null;
  body_html: string | null;
  has_attachments: boolean;
  imap_flags: string;
  distilled_text: string | null;
  is_sent: boolean;
  in_reply_to: string | null;
};

export type ConnectAccountParams = {
  email: string;
  password: string;
  imapHost: string;
  imapPort: number;
  imapTls?: boolean;
  smtpHost: string;
  smtpPort: number;
  aliases?: string;
};

export type TaskStatus = {
  name: string;
  status: string;
};

export type TrustContact = {
  name: string;
  email: string;
  message_count: number;
};

export type OnboardingStatus = {
  tasks: TaskStatus[];
  message_count: number;
  trust_contacts: TrustContact[];
  trust_contact_count: number;
  is_complete: boolean;
};

export type ExistingAccount = {
  id: string;
  email: string;
};

export type DiscoveryResult = {
  provider: string | null;
  provider_id: string | null;
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
};
