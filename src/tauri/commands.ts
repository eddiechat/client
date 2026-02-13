/**
 * Type-safe wrappers for all Tauri invoke commands.
 * All backend communication should go through this module.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  EmailAccount,
  EmailAccountDetails,
  SaveEmailAccountRequest,
  SaveDiscoveredEmailAccountRequest,
  SendMessageResult,
  SyncStatus,
  Message,
  Conversation,
  DiscoveryResult,
  AttachmentInfo,
  ComposeAttachment,
} from "./types";

// ========== App Commands ==========

export async function getAppVersion(): Promise<string> {
  return invoke("get_app_version");
}

// ========== Account Commands ==========

export async function saveAccount(request: SaveEmailAccountRequest): Promise<void> {
  return invoke("save_account", { request });
}

export async function listAccounts(): Promise<EmailAccount[]> {
  return invoke("list_accounts");
}

export async function getDefaultAccount(): Promise<string | null> {
  return invoke("get_default_account");
}

export async function removeAccount(name: string): Promise<void> {
  return invoke("remove_account", { name });
}

export async function getAccountDetails(name: string): Promise<EmailAccountDetails> {
  return invoke("get_account_details", { name });
}

export async function saveDiscoveredAccount(
  request: SaveDiscoveredEmailAccountRequest
): Promise<void> {
  return invoke("save_discovered_account", {
    name: request.name,
    email: request.email,
    displayName: request.displayName,
    imapHost: request.imapHost,
    imapPort: request.imapPort,
    imapTls: request.imapTls,
    smtpHost: request.smtpHost,
    smtpPort: request.smtpPort,
    smtpTls: request.smtpTls,
    authMethod: request.authMethod,
    password: request.password,
  });
}

// ========== Message Commands ==========

export async function sendMessage(
  message: string,
  account?: string
): Promise<SendMessageResult | null> {
  return invoke("send_message", { account, message });
}

export async function sendMessageWithAttachments(
  from: string,
  to: string[],
  subject: string,
  body: string,
  attachments: ComposeAttachment[],
  cc?: string[],
  account?: string
): Promise<SendMessageResult | null> {
  return invoke("send_message_with_attachments", {
    account,
    from,
    to,
    cc,
    subject,
    body,
    attachments,
  });
}

// ========== Sync Engine Commands ==========

export async function initSyncEngine(account?: string): Promise<SyncStatus> {
  return invoke("init_sync_engine", { account });
}

export async function getSyncStatus(account?: string): Promise<SyncStatus> {
  return invoke("get_sync_status", { account });
}

export async function syncNow(): Promise<string> {
  return invoke("sync_now");
}

/** Get cached conversations, optionally filtered by tab */
export async function getCachedConversations(
  tab?: 'connections' | 'all' | 'others',
  account?: string
): Promise<Conversation[]> {
  return invoke("get_cached_conversations", { account, tab });
}

/** Get messages for a conversation by conversation ID (string hash) */
export async function getCachedConversationMessages(
  conversationId: string,
  account?: string
): Promise<Message[]> {
  return invoke("get_cached_conversation_messages", { account, conversationId });
}

/** Fetch a single message by ID, returns null if not found */
export async function fetchMessageBody(
  messageId: string,
  account?: string
): Promise<Message | null> {
  return invoke("fetch_message_body", { account, messageId });
}

export async function rebuildConversations(
  account?: string
): Promise<number> {
  return invoke("rebuild_conversations", { account });
}

export async function reclassify(account?: string): Promise<string> {
  return invoke("reclassify", { account });
}

export async function dropAndResync(account?: string): Promise<void> {
  return invoke("drop_and_resync", { account });
}

export async function markConversationRead(
  conversationId: string,
  account?: string
): Promise<void> {
  return invoke("mark_conversation_read", { account, conversationId });
}

export async function shutdownSyncEngine(account?: string): Promise<void> {
  return invoke("shutdown_sync_engine", { account });
}

// ========== Email Discovery Commands ==========

export async function discoverEmailConfig(email: string): Promise<DiscoveryResult> {
  return invoke("discover_email_config", { email });
}

// ========== Attachment Commands ==========

export async function getMessageAttachments(
  folder: string,
  id: string,
  account?: string
): Promise<AttachmentInfo[]> {
  return invoke("get_message_attachments", { account, folder, id });
}

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

// ========== Entity (Participant) Commands ==========

export interface EntitySuggestion {
  id: string;
  email: string;
  name: string | null;
  trust_level: string;
  last_seen: number | null;
}

/** Search entities for autocomplete suggestions
 * Returns up to `limit` entities matching the query, prioritizing connections and recent contacts
 */
export async function searchEntities(
  query: string,
  limit?: number,
  account?: string
): Promise<EntitySuggestion[]> {
  return invoke("search_entities", { account, query, limit });
}

// ========== Read-Only Mode Commands ==========

/**
 * Get the read-only mode setting.
 * When enabled, all operations that modify data on the server are blocked.
 */
export async function getReadOnlyMode(): Promise<boolean> {
  return invoke("get_read_only_mode");
}

/**
 * Set the read-only mode setting.
 * When enabled, all operations that modify data on the server are blocked.
 */
export async function setReadOnlyMode(enabled: boolean): Promise<void> {
  return invoke("set_read_only_mode", { enabled });
}
