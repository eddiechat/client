import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SyncStatus, ConversationsUpdated } from "./types";

export async function onSyncStatus(
  callback: (status: SyncStatus) => void
): Promise<UnlistenFn> {
  return listen<SyncStatus>("sync:status", (event) => {
    callback(event.payload);
  });
}

export async function onConversationsUpdated(
  callback: (data: ConversationsUpdated) => void
): Promise<UnlistenFn> {
  return listen<ConversationsUpdated>(
    "sync:conversations-updated",
    (event) => {
      callback(event.payload);
    }
  );
}

export async function onOnboardingComplete(
  callback: (data: { account_id: string }) => void
): Promise<UnlistenFn> {
  return listen<{ account_id: string }>(
    "onboarding:complete",
    (event) => {
      callback(event.payload);
    }
  );
}
