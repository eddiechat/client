import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountDetails,
  Envelope,
  Folder,
  ListEnvelopesRequest,
  ListEnvelopesResponse,
  Message,
  ReadMessageRequest,
  FlagRequest,
  SaveAccountRequest,
} from "../types";

// Config commands
export async function initConfig(): Promise<void> {
  return invoke("init_config");
}

export async function initConfigFromPaths(paths: string[]): Promise<void> {
  return invoke("init_config_from_paths", { paths });
}

export async function isConfigInitialized(): Promise<boolean> {
  return invoke("is_config_initialized");
}

export async function getConfigPaths(): Promise<string[]> {
  return invoke("get_config_paths");
}

export async function saveAccount(request: SaveAccountRequest): Promise<void> {
  return invoke("save_account", { request });
}

// Account commands
export async function listAccounts(): Promise<Account[]> {
  return invoke("list_accounts");
}

export async function getDefaultAccount(): Promise<string | null> {
  return invoke("get_default_account");
}

export async function accountExists(name: string): Promise<boolean> {
  return invoke("account_exists", { name });
}

export async function removeAccount(name: string): Promise<void> {
  return invoke("remove_account", { name });
}

export async function getAccountDetails(name: string): Promise<AccountDetails> {
  return invoke("get_account_details", { name });
}

// Folder commands
export async function listFolders(account?: string): Promise<Folder[]> {
  return invoke("list_folders", { account });
}

export async function createFolder(name: string, account?: string): Promise<void> {
  return invoke("create_folder", { account, name });
}

export async function deleteFolder(name: string, account?: string): Promise<void> {
  return invoke("delete_folder", { account, name });
}

export async function expungeFolder(name: string, account?: string): Promise<void> {
  return invoke("expunge_folder", { account, name });
}

// Envelope commands
export async function listEnvelopes(
  request: ListEnvelopesRequest
): Promise<ListEnvelopesResponse> {
  return invoke("list_envelopes", { request });
}

export async function threadEnvelopes(
  account?: string,
  folder?: string,
  envelopeId?: string,
  query?: string
): Promise<Envelope[]> {
  return invoke("thread_envelopes", {
    account,
    folder,
    envelope_id: envelopeId,
    query,
  });
}

// Message commands
export async function readMessage(request: ReadMessageRequest): Promise<Message> {
  return invoke("read_message", { request });
}

export async function deleteMessages(
  ids: string[],
  account?: string,
  folder?: string
): Promise<void> {
  return invoke("delete_messages", { account, folder, ids });
}

export async function copyMessages(
  ids: string[],
  targetFolder: string,
  account?: string,
  sourceFolder?: string
): Promise<void> {
  return invoke("copy_messages", {
    account,
    source_folder: sourceFolder,
    target_folder: targetFolder,
    ids,
  });
}

export async function moveMessages(
  ids: string[],
  targetFolder: string,
  account?: string,
  sourceFolder?: string
): Promise<void> {
  return invoke("move_messages", {
    account,
    source_folder: sourceFolder,
    target_folder: targetFolder,
    ids,
  });
}

export async function sendMessage(message: string, account?: string): Promise<void> {
  return invoke("send_message", { account, message });
}

export async function saveMessage(
  message: string,
  folder?: string,
  account?: string
): Promise<string> {
  return invoke("save_message", { account, folder, message });
}

export async function downloadAttachments(
  id: string,
  account?: string,
  folder?: string,
  downloadDir?: string
): Promise<string[]> {
  return invoke("download_attachments", {
    account,
    folder,
    id,
    download_dir: downloadDir,
  });
}

// Flag commands
export async function addFlags(request: FlagRequest): Promise<void> {
  return invoke("add_flags", { request });
}

export async function removeFlags(request: FlagRequest): Promise<void> {
  return invoke("remove_flags", { request });
}

export async function setFlags(request: FlagRequest): Promise<void> {
  return invoke("set_flags", { request });
}

export async function markAsRead(
  ids: string[],
  account?: string,
  folder?: string
): Promise<void> {
  return invoke("mark_as_read", { account, folder, ids });
}

export async function markAsUnread(
  ids: string[],
  account?: string,
  folder?: string
): Promise<void> {
  return invoke("mark_as_unread", { account, folder, ids });
}

export async function toggleFlagged(
  id: string,
  isFlagged: boolean,
  account?: string,
  folder?: string
): Promise<void> {
  return invoke("toggle_flagged", {
    account,
    folder,
    id,
    is_flagged: isFlagged,
  });
}
