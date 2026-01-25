import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountDetails,
  SaveAccountRequest,
  Conversation,
  Message,
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

// Message commands
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
