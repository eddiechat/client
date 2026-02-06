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
  CachedConversation,
  CachedChatMessage,
  DiscoveryResult,
  AttachmentInfo,
  ComposeAttachment,
  SyncActionType,
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

export async function getConversationMessages(
  messageIds: string[],
  account?: string
): Promise<CachedChatMessage[]> {
  if (messageIds.length === 0) return [];
  return invoke("get_conversation_messages", { account, messageIds });
}

// ========== Sync Engine Commands ==========

export async function initSyncEngine(account?: string): Promise<SyncStatus> {
  return invoke("init_sync_engine", { account });
}

export async function getSyncStatus(account?: string): Promise<SyncStatus> {
  return invoke("get_sync_status", { account });
}

export async function syncFolder(folder?: string, account?: string): Promise<void> {
  return invoke("sync_folder", { account, folder });
}

export async function initialSync(account?: string): Promise<void> {
  return invoke("initial_sync", { account });
}

export async function getCachedConversations(
  tab?: 'connections' | 'all' | 'others',
  account?: string
): Promise<CachedConversation[]> {
  return invoke("get_cached_conversations", { account, tab });
}

export async function getCachedConversationMessages(
  conversationId: number,
  account?: string
): Promise<CachedChatMessage[]> {
  return invoke("get_cached_conversation_messages", { account, conversationId });
}

export async function fetchMessageBody(
  messageId: number,
  account?: string
): Promise<CachedChatMessage> {
  return invoke("fetch_message_body", { account, messageId });
}

export async function rebuildConversations(
  userEmail: string,
  account?: string
): Promise<number> {
  return invoke("rebuild_conversations", { account, userEmail });
}

export async function dropAndResync(account?: string): Promise<void> {
  return invoke("drop_and_resync", { account });
}

export async function queueSyncAction(
  actionType: SyncActionType,
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

export async function setSyncOnline(online: boolean, account?: string): Promise<void> {
  return invoke("set_sync_online", { account, online });
}

export async function hasPendingSyncActions(account?: string): Promise<boolean> {
  return invoke("has_pending_sync_actions", { account });
}

export async function shutdownSyncEngine(account?: string): Promise<void> {
  return invoke("shutdown_sync_engine", { account });
}

export async function markConversationRead(
  conversationId: number,
  account?: string
): Promise<void> {
  return invoke("mark_conversation_read", { account, conversationId });
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
  id: number;
  email: string;
  name: string | null;
  is_connection: boolean;
  latest_contact: string;
  contact_count: number;
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
