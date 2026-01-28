import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountDetails,
  SaveAccountRequest,
  Conversation,
  Message,
  ComposeAttachment,
} from "../types";

export async function saveAccount(request: SaveAccountRequest): Promise<void> {
  return invoke("save_account", { request });
}

export async function listAccounts(): Promise<Account[]> {
  return invoke("list_accounts");
}

export async function getDefaultAccount(): Promise<string | null> {
  return invoke("get_default_account");
}

export async function removeAccount(name: string): Promise<void> {
  return invoke("remove_account", { name });
}

export async function getAccountDetails(name: string): Promise<AccountDetails> {
  return invoke("get_account_details", { name });
}

// Message commands
export interface SendMessageResult {
  message_id: string;
  sent_folder: string;
}

// Returns the message ID and sent folder name, or null if no Sent folder was found
export async function sendMessage(message: string, account?: string): Promise<SendMessageResult | null> {
  return invoke("send_message", { account, message });
}

// Send a message with optional attachments
export async function sendMessageWithAttachments(
  from: string,
  to: string[],
  subject: string,
  body: string,
  attachments: ComposeAttachment[],
  cc?: string[],
  account?: string,
  inReplyTo?: string
): Promise<SendMessageResult | null> {
  return invoke("send_message_with_attachments", {
    account,
    from,
    to,
    cc,
    subject,
    body,
    attachments,
    in_reply_to: inReplyTo, // Tauri expects snake_case to match Rust parameter
  });
}

export async function saveMessage(
  message: string,
  folder?: string,
  account?: string
): Promise<string> {
  return invoke("save_message", { account, folder, message });
}

// Conversation commands (for Signal-like messaging UI)
export async function listConversations(account?: string): Promise<Conversation[]> {
  return invoke("list_conversations", { account });
}

export async function getConversationMessages(
  messageIds: string[],
  account?: string
): Promise<Message[]> {
  if (messageIds.length === 0) return [];
  return invoke("get_conversation_messages", { account, messageIds });
}

// ========== Sync Engine Commands ==========

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

export interface CachedMessage {
  id: number;
  folder: string;
  uid: number;
  message_id: string | null;
  in_reply_to: string | null;
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

/** Initialize the sync engine for an account */
export async function initSyncEngine(account?: string): Promise<SyncStatus> {
  return invoke("init_sync_engine", { account });
}

/** Get the current sync status */
export async function getSyncStatus(account?: string): Promise<SyncStatus> {
  return invoke("get_sync_status", { account });
}

/** Trigger a sync for a folder */
export async function syncFolder(folder?: string, account?: string): Promise<void> {
  return invoke("sync_folder", { account, folder });
}

/** Perform initial sync for a new account */
export async function initialSync(account?: string): Promise<void> {
  return invoke("initial_sync", { account });
}

/** Get cached conversations from SQLite */
export async function getCachedConversations(
  includeHidden?: boolean,
  account?: string
): Promise<CachedConversation[]> {
  return invoke("get_cached_conversations", { account, includeHidden });
}

/** Get cached messages for a conversation */
export async function getCachedConversationMessages(
  conversationId: number,
  account?: string
): Promise<CachedMessage[]> {
  return invoke("get_cached_conversation_messages", { account, conversationId });
}

/** Fetch message body on demand (if not already cached) */
export async function fetchMessageBody(
  messageId: number,
  account?: string
): Promise<CachedMessage> {
  return invoke("fetch_message_body", { account, messageId });
}

/** Queue a sync action for offline support */
export async function queueSyncAction(
  actionType: "add_flags" | "remove_flags" | "delete" | "move" | "copy",
  folder: string,
  uids: number[],
  flags?: string[],
  targetFolder?: string,
  account?: string
): Promise<number> {
  return invoke("queue_sync_action", {
    account,
    actionType,
    folder,
    uids,
    flags,
    targetFolder,
  });
}

/** Set online status for the sync engine */
export async function setSyncOnline(online: boolean, account?: string): Promise<void> {
  return invoke("set_sync_online", { account, online });
}

/** Check if there are pending sync actions */
export async function hasPendingSyncActions(account?: string): Promise<boolean> {
  return invoke("has_pending_sync_actions", { account });
}

/** Shutdown the sync engine */
export async function shutdownSyncEngine(account?: string): Promise<void> {
  return invoke("shutdown_sync_engine", { account });
}

/** Mark all unread messages in a conversation as read */
export async function markConversationRead(
  conversationId: number,
  account?: string
): Promise<void> {
  return invoke("mark_conversation_read", { account, conversationId });
}

// ========== Attachment Commands ==========

export interface AttachmentInfo {
  index: number;
  filename: string;
  mime_type: string;
  size: number;
}

/** Get attachment information for a message */
export async function getMessageAttachments(
  folder: string,
  id: string,
  account?: string
): Promise<AttachmentInfo[]> {
  return invoke("get_message_attachments", { account, folder, id });
}

/** Download a specific attachment from a message */
export async function downloadAttachment(
  folder: string,
  id: string,
  attachmentIndex: number,
  downloadDir?: string,
  account?: string
): Promise<string> {
  return invoke("download_attachment", {
    account,
    folder,
    id,
    attachmentIndex,
    downloadDir,
  });
}

/** Download all attachments from a message */
export async function downloadAttachments(
  folder: string,
  id: string,
  downloadDir?: string,
  account?: string
): Promise<string[]> {
  return invoke("download_attachments", { account, folder, id, downloadDir });
}
