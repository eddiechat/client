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
  unread_count: number;
  total_count: number;
  is_muted: boolean;
  is_pinned: boolean;
  is_important: boolean;
  updated_at: number;
};

export type Cluster = {
  id: string;
  name: string;
  from_name: string | null;
  message_count: number;
  unread_count: number;
  keywords: string;
  last_activity: number;
  account_id: string;
  is_join: boolean;
  domains: string; // JSON array of sender email strings
  is_skill: boolean;
  skill_id: string | null;
  icon: string | null;
  icon_bg: string | null;
};

export type Thread = {
  thread_id: string;
  subject: string | null;
  message_count: number;
  unread_count: number;
  last_activity: number;
  from_name: string | null;
  from_address: string;
  preview: string | null;
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

export type Skill = {
  id: string;
  account_id: string;
  name: string;
  icon: string;
  icon_bg: string;
  enabled: boolean;
  prompt: string;
  modifiers: string;
  settings: string;
  created_at: number;
  updated_at: number;
  has_model: boolean;
};

export type SkillModifiers = {
  excludeNewsletters: boolean;
  onlyKnownSenders: boolean;
  hasAttachments: boolean;
  recentSixMonths: boolean;
  excludeAutoReplies: boolean;
};

export type SkillSettings = Record<string, unknown>;

export type OllamaModels = {
  models: string[];
  selected_model: string | null;
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
