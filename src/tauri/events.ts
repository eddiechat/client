/**
 * Tauri event listeners and subscriptions.
 * Provides type-safe event subscription functions.
 */

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SyncEventPayload, SyncStatus } from "./types";

/**
 * Subscribe to sync engine events.
 * Returns an unlisten function to clean up the subscription.
 */
export async function onSyncEvent(
  callback: (payload: SyncEventPayload) => void
): Promise<UnlistenFn> {
  return listen<SyncEventPayload>("sync-event", (event) => {
    callback(event.payload);
  });
}

/**
 * Subscribe to sync status changes specifically.
 * Filters sync events to only status changes.
 */
export async function onSyncStatusChange(
  callback: (status: SyncStatus) => void
): Promise<UnlistenFn> {
  return listen<SyncEventPayload>("sync-event", (event) => {
    if ("StatusChanged" in event.payload && event.payload.StatusChanged) {
      callback(event.payload.StatusChanged);
    }
  });
}

/**
 * Subscribe to sync completion events.
 */
export async function onSyncComplete(
  callback: () => void
): Promise<UnlistenFn> {
  return listen<SyncEventPayload>("sync-event", (event) => {
    if ("SyncComplete" in event.payload) {
      callback();
    }
  });
}

/**
 * Subscribe to sync error events.
 */
export async function onSyncError(
  callback: (message: string) => void
): Promise<UnlistenFn> {
  return listen<SyncEventPayload>("sync-event", (event) => {
    if ("Error" in event.payload && event.payload.Error) {
      callback(event.payload.Error.message);
    }
  });
}

/**
 * Subscribe to conversation update events.
 */
export async function onConversationsUpdated(
  callback: (conversationIds: number[]) => void
): Promise<UnlistenFn> {
  return listen<SyncEventPayload>("sync-event", (event) => {
    if ("ConversationsUpdated" in event.payload && event.payload.ConversationsUpdated) {
      callback(event.payload.ConversationsUpdated.conversation_ids);
    }
  });
}

/**
 * Subscribe to new message events.
 */
export async function onNewMessages(
  callback: (folder: string, count: number) => void
): Promise<UnlistenFn> {
  return listen<SyncEventPayload>("sync-event", (event) => {
    if ("NewMessages" in event.payload && event.payload.NewMessages) {
      callback(event.payload.NewMessages.folder, event.payload.NewMessages.count);
    }
  });
}
