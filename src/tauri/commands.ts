import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Message, ConnectAccountParams, Cluster, Thread, Skill, OllamaModels, OnboardingStatus, DiscoveryResult, ExistingAccount } from "./types";

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

export async function fetchClusters(
  accountId: string
): Promise<Cluster[]> {
  return invoke<Cluster[]>("fetch_clusters", { accountId });
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

export async function fetchClusterMessages(
  accountId: string,
  clusterId: string
): Promise<Message[]> {
  return invoke<Message[]>("fetch_cluster_messages", {
    accountId,
    clusterId,
  });
}

export async function reclassify(accountId: string): Promise<string> {
  return invoke<string>("reclassify", { accountId });
}

export async function listSkills(accountId: string): Promise<Skill[]> {
  return invoke<Skill[]>("list_skills", { accountId });
}

export async function getSkill(skillId: string): Promise<Skill> {
  return invoke<Skill>("get_skill", { skillId });
}

export async function createSkill(
  accountId: string,
  name: string,
  icon: string,
  iconBg: string,
  prompt: string,
  modifiers: string,
  settings: string,
): Promise<string> {
  return invoke<string>("create_skill", { accountId, name, icon, iconBg, prompt, modifiers, settings });
}

export async function updateSkill(
  id: string,
  name: string,
  icon: string,
  iconBg: string,
  prompt: string,
  modifiers: string,
  settings: string,
): Promise<void> {
  return invoke<void>("update_skill", { id, name, icon, iconBg, prompt, modifiers, settings });
}

export async function toggleSkill(skillId: string, enabled: boolean): Promise<void> {
  return invoke<void>("toggle_skill", { skillId, enabled });
}

export async function deleteSkill(skillId: string): Promise<void> {
  return invoke<void>("delete_skill", { skillId });
}

export async function fetchClusterThreads(
  accountId: string,
  clusterId: string
): Promise<Thread[]> {
  return invoke<Thread[]>("fetch_cluster_threads", { accountId, clusterId });
}

export async function fetchThreadMessages(
  accountId: string,
  threadId: string
): Promise<Message[]> {
  return invoke<Message[]>("fetch_thread_messages", { accountId, threadId });
}

export async function groupDomains(accountId: string, name: string, domains: string[]): Promise<string> {
  return invoke<string>("group_domains", { accountId, name, domains });
}

export async function ungroupDomains(accountId: string, groupId: string): Promise<void> {
  return invoke<void>("ungroup_domains", { accountId, groupId });
}

export async function getSetting(key: string): Promise<string | null> {
  return invoke<string | null>("get_setting", { key });
}

export async function setSetting(key: string, value: string): Promise<void> {
  return invoke<void>("set_setting", { key, value });
}

export async function getOllamaModels(key: string): Promise<OllamaModels> {
  return invoke<OllamaModels>("get_ollama_models", { key });
}

export async function fetchRecentMessages(
  accountId: string,
  limit: number
): Promise<Message[]> {
  return invoke<Message[]>("fetch_recent_messages", { accountId, limit });
}

export async function ollamaComplete(
  url: string,
  model: string,
  systemPrompt: string,
  userPrompt: string,
  temperature: number = 0
): Promise<string> {
  return invoke<string>("ollama_complete", { url, model, systemPrompt, userPrompt, temperature });
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
