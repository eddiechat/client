/**
 * Type-safe wrappers for all Tauri invoke commands.
 * All backend communication should go through this module.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountDetails,
  SaveAccountRequest,
  SaveDiscoveredAccountRequest,
  SendMessageResult,
  SyncStatus,
  CachedConversation,
  CachedMessage,
  DiscoveryResult,
  AttachmentInfo,
  ComposeAttachment,
  SyncActionType,
} from "./types";

// ========== Account Commands ==========

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

export async function saveDiscoveredAccount(
  request: SaveDiscoveredAccountRequest
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

export async function saveMessage(
  message: string,
  folder?: string,
  account?: string
): Promise<string> {
  return invoke("save_message", { account, folder, message });
}

export async function getConversationMessages(
  messageIds: string[],
  account?: string
): Promise<CachedMessage[]> {
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
  includeHidden?: boolean,
  account?: string
): Promise<CachedConversation[]> {
  return invoke("get_cached_conversations", { account, includeHidden });
}

export async function getCachedConversationMessages(
  conversationId: number,
  account?: string
): Promise<CachedMessage[]> {
  return invoke("get_cached_conversation_messages", { account, conversationId });
}

export async function fetchMessageBody(
  messageId: number,
  account?: string
): Promise<CachedMessage> {
  return invoke("fetch_message_body", { account, messageId });
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

export async function testEmailConnection(
  email: string,
  imapHost: string,
  imapPort: number,
  imapTls: boolean,
  smtpHost: string,
  smtpPort: number,
  smtpTls: boolean,
  authMethod: string,
  password?: string
): Promise<boolean> {
  return invoke("test_email_connection", {
    email,
    imapHost,
    imapPort,
    imapTls,
    smtpHost,
    smtpPort,
    smtpTls,
    authMethod,
    password,
  });
}

// ========== Credential Commands ==========

export async function storePassword(email: string, password: string): Promise<void> {
  return invoke("store_password", { email, password });
}

export async function storeAppPassword(email: string, password: string): Promise<void> {
  return invoke("store_app_password", { email, password });
}

export async function deleteCredentials(email: string): Promise<void> {
  return invoke("delete_credentials", { email });
}

export async function hasCredentials(
  email: string,
  credentialType: "password" | "app_password"
): Promise<boolean> {
  return invoke("has_credentials", { email, credentialType });
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

export async function downloadAttachments(
  folder: string,
  id: string,
  downloadDir?: string,
  account?: string
): Promise<string[]> {
  return invoke("download_attachments", { account, folder, id, downloadDir });
}
