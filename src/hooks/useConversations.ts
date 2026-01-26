import { useState, useEffect, useCallback, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import * as api from "../lib/api";
import type { Conversation, Message } from "../types";

/** Sync event payload from Tauri backend */
interface SyncEventPayload {
  StatusChanged?: api.SyncStatus;
  NewMessages?: { folder: string; count: number };
  MessagesDeleted?: { folder: string; uids: number[] };
  FlagsChanged?: { folder: string; uids: number[] };
  ConversationsUpdated?: { conversation_ids: number[] };
  Error?: { message: string };
  SyncComplete?: null;
}

/**
 * Hook for managing conversations using the sync engine.
 *
 * On first load, initializes the sync engine which:
 * 1. Creates a SQLite database for the account
 * 2. Fetches all messages from IMAP
 * 3. Builds conversations from cached data
 *
 * Subsequent loads read directly from the local cache.
 */
export function useConversations(account?: string) {
  const [conversations, setConversations] = useState<api.CachedConversation[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<api.SyncStatus | null>(null);
  const initRef = useRef(false);
  const pollRef = useRef<number | null>(null);

  // Initialize sync engine and start syncing
  const initializeSync = useCallback(async () => {
    try {
      setLoading(true);
      setSyncing(true);
      setError(null);

      // Initialize sync engine - this starts a background sync
      const status = await api.initSyncEngine(account);
      setSyncStatus(status);

      // Immediately try to load cached conversations
      const cachedConvs = await api.getCachedConversations(false, account);
      if (cachedConvs.length > 0) {
        setConversations(cachedConvs);
        setLoading(false);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setLoading(false);
    }
  }, [account]);

  // Poll for sync status and refresh conversations when sync completes
  const pollSyncStatus = useCallback(async () => {
    try {
      const status = await api.getSyncStatus(account);
      setSyncStatus(status);

      // Update syncing state
      const isSyncing = status.state === "syncing" || status.state === "initial_sync";
      setSyncing(isSyncing);

      // If sync completed or we have data, refresh conversations
      if (status.is_online || status.last_sync) {
        const cachedConvs = await api.getCachedConversations(false, account);
        setConversations(cachedConvs);
        setLoading(false);
      }

      // If there's an error, show it
      if (status.error) {
        setError(status.error);
      }
    } catch (e) {
      console.error("Failed to poll sync status:", e);
    }
  }, [account]);

  // Refresh conversations from cache
  const refreshConversations = useCallback(async () => {
    try {
      const cachedConvs = await api.getCachedConversations(false, account);
      setConversations(cachedConvs);
    } catch (e) {
      console.error("Failed to refresh conversations:", e);
    }
  }, [account]);

  // Trigger a manual sync
  const triggerSync = useCallback(async () => {
    try {
      setSyncing(true);
      await api.initialSync(account);
      await refreshConversations();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSyncing(false);
    }
  }, [account, refreshConversations]);

  // Initialize on mount
  useEffect(() => {
    if (!initRef.current) {
      initRef.current = true;
      initializeSync();
    }
  }, [initializeSync]);

  // Listen for Tauri sync events instead of polling
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<SyncEventPayload>("sync-event", (event) => {
        const payload = event.payload;

        // Handle different event types
        if ("StatusChanged" in payload && payload.StatusChanged) {
          const status = payload.StatusChanged;
          setSyncStatus(status);

          const isSyncing = status.state === "syncing" || status.state === "initial_sync";
          setSyncing(isSyncing);

          if (status.error) {
            setError(status.error);
          }
        }

        if ("ConversationsUpdated" in payload) {
          // Refresh conversations when they're updated
          refreshConversations();
        }

        if ("SyncComplete" in payload) {
          setSyncing(false);
          setLoading(false);
          refreshConversations();
        }

        if ("Error" in payload && payload.Error) {
          setError(payload.Error.message);
        }
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [refreshConversations]);

  // Fallback: poll infrequently in case events are missed
  useEffect(() => {
    // Poll every 60 seconds as a fallback
    pollRef.current = window.setInterval(pollSyncStatus, 60000);

    return () => {
      if (pollRef.current) {
        window.clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [pollSyncStatus]);

  // Convert cached conversations to the expected format
  const formattedConversations: Conversation[] = conversations.map((c) => ({
    id: c.participant_key,
    participants: c.participants.map((p) => p.email),
    participant_names: c.participants.map((p) => p.name || p.email),
    last_message_date: c.last_message_date || "",
    last_message_preview: c.last_message_preview || "",
    last_message_from: c.last_message_from || "",
    unread_count: c.unread_count,
    message_ids: [], // Not used when reading from cache
    is_outgoing: c.is_outgoing,
    user_name: "", // Will be set by the component
    user_in_conversation: true,
    // Store the conversation ID for fetching messages
    _cached_id: c.id,
  }));

  return {
    conversations: formattedConversations,
    loading,
    syncing,
    error,
    syncStatus,
    refresh: refreshConversations,
    triggerSync,
    listConversations: refreshConversations,
  };
}

/**
 * Hook for getting messages in a conversation.
 *
 * Uses the sync engine's cache - messages are fetched from SQLite.
 * If message bodies are not cached, they will be fetched on demand.
 */
export function useConversationMessages(
  conversation: Conversation | null,
  account?: string
) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchMessages = useCallback(async () => {
    // Check if we have a cached conversation ID
    const cachedId = (conversation as any)?._cached_id as number | undefined;

    if (!conversation && !cachedId) {
      setMessages([]);
      return;
    }

    try {
      setLoading(true);
      setError(null);

      if (cachedId) {
        // Fetch from cache using conversation ID
        const cachedMessages = await api.getCachedConversationMessages(cachedId, account);

        // Fetch bodies for messages that don't have them cached
        const messagesWithBodies = await Promise.all(
          cachedMessages.map(async (msg) => {
            if (!msg.body_cached && !msg.text_body && !msg.html_body) {
              try {
                const withBody = await api.fetchMessageBody(msg.id, account);
                return withBody;
              } catch (e) {
                console.warn("Failed to fetch body for message:", msg.id, e);
                return msg;
              }
            }
            return msg;
          })
        );

        // Convert to Message format
        const formattedMessages: Message[] = messagesWithBodies.map((m) => ({
          id: `${m.folder}:${m.uid}`,
          envelope: {
            id: m.uid.toString(),
            message_id: m.message_id || undefined,
            in_reply_to: undefined,
            from: m.from_name ? `${m.from_name} <${m.from_address}>` : m.from_address,
            to: m.to_addresses,
            subject: m.subject || "",
            date: m.date || "",
            flags: m.flags,
            has_attachment: m.has_attachment,
          },
          headers: [],
          text_body: m.text_body || undefined,
          html_body: m.html_body || undefined,
          attachments: [],
        }));

        setMessages(formattedMessages);
      } else if (conversation?.message_ids && conversation.message_ids.length > 0) {
        // Fallback to old method if no cached ID
        const messageList = await api.getConversationMessages(conversation.message_ids, account);
        setMessages(messageList);
      } else {
        setMessages([]);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [conversation, account]);

  useEffect(() => {
    fetchMessages();
  }, [fetchMessages]);

  return {
    messages,
    loading,
    error,
    refresh: fetchMessages,
    getConversationMessages: fetchMessages,
  };
}
