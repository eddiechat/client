import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountDetails,
  SaveAccountRequest,
  Conversation,
  Message,
  Contact,
  AddressBook,
  SaveContactRequest,
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
// Returns the message ID in the Sent folder, or null if no Sent folder was found
export async function sendMessage(message: string, account?: string): Promise<string | null> {
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

// Contact commands (CardDAV)
export async function listContacts(account?: string): Promise<Contact[]> {
  return invoke("list_contacts", { account });
}

export async function getContact(contactId: string, account?: string): Promise<Contact> {
  return invoke("get_contact", { account, contactId });
}

export async function createContact(request: SaveContactRequest): Promise<Contact> {
  return invoke("create_contact", { request });
}

export async function updateContact(request: SaveContactRequest): Promise<Contact> {
  return invoke("update_contact", { request });
}

export async function deleteContact(
  contactId: string,
  href?: string,
  account?: string
): Promise<void> {
  return invoke("delete_contact", { account, contactId, href });
}

export async function listAddressBooks(account?: string): Promise<AddressBook[]> {
  return invoke("list_address_books", { account });
}

export async function hasCardDAVConfig(account?: string): Promise<boolean> {
  return invoke("has_carddav_config", { account });
}
