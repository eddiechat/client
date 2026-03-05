import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Message, ConnectAccountParams, OnboardingStatus, DiscoveryResult, ExistingAccount } from "./types";

export async function connectAccount(
  params: ConnectAccountParams
): Promise<string> {
  return invoke<string>("connect_account", params);
}

export async function fetchConversations(
  accountId: string
): Promise<Conversation[]> {
  return invoke<Conversation[]>("fetch_conversations", { accountId });
}

export async function syncNow(): Promise<string> {
  return invoke<string>("sync_now");
}

export async function fetchConversationMessages(
  accountId: string,
  conversationId: string
): Promise<Message[]> {
  return invoke<Message[]>("fetch_conversation_messages", {
    accountId,
    conversationId,
  });
}

export async function reclassify(accountId: string): Promise<string> {
  return invoke<string>("reclassify", { accountId });
}

export async function getSetting(key: string): Promise<string | null> {
  return invoke<string | null>("get_setting", { key });
}

export async function setSetting(key: string, value: string): Promise<void> {
  return invoke<void>("set_setting", { key, value });
}

export async function fetchRecentMessages(
  accountId: string,
  limit: number
): Promise<Message[]> {
  return invoke<Message[]>("fetch_recent_messages", { accountId, limit });
}

export async function getOnboardingStatus(
  accountId: string
): Promise<OnboardingStatus> {
  return invoke<OnboardingStatus>("get_onboarding_status", { accountId });
}

export async function discoverEmailConfig(
  email: string
): Promise<DiscoveryResult> {
  return invoke<DiscoveryResult>("discover_email_config", { email });
}

export async function getExistingAccount(): Promise<ExistingAccount | null> {
  return invoke<ExistingAccount | null>("get_existing_account");
}

export async function moveToLines(
  accountId: string,
  emails: string[]
): Promise<void> {
  return invoke<void>("move_to_lines", { accountId, emails });
}

export async function moveToPoints(
  accountId: string,
  emails: string[]
): Promise<void> {
  return invoke<void>("move_to_points", { accountId, emails });
}

export async function blockEntities(
  accountId: string,
  emails: string[]
): Promise<void> {
  return invoke<void>("block_entities", { accountId, emails });
}

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export async function fetchMessageHtml(
  messageId: string
): Promise<string | null> {
  return invoke<string | null>("fetch_message_html", { messageId });
}
