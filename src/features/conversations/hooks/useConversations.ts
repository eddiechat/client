import { useState, useEffect, useCallback, useRef } from "react";
import {
  initSyncEngine,
  getSyncStatus,
  getCachedConversations,
  initialSync,
  onSyncEvent,
} from "../../../tauri";
import type { Conversation, SyncStatus, CachedConversation } from "../../../tauri";

interface UseConversationsResult {
  conversations: Conversation[];
  loading: boolean;
  syncing: boolean;
  error: string | null;
  syncStatus: SyncStatus | null;
  refresh: () => Promise<void>;
  triggerSync: () => Promise<void>;
}

/**
 * Convert cached conversation to display format.
 */
function formatConversation(cached: CachedConversation): Conversation {
  return {
    id: cached.participant_key,
    participants: cached.participants.map((p) => p.email),
    participant_names: cached.participants.map((p) => p.name || p.email),
    last_message_date: cached.last_message_date || "",
    last_message_preview: cached.last_message_preview || "",
    last_message_from: cached.last_message_from || "",
    unread_count: cached.unread_count,
    message_ids: [],
    is_outgoing: cached.is_outgoing,
    user_name: "",
    user_in_conversation: true,
    _cached_id: cached.id,
  };
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
 *
 * @param account - The account to load conversations for
 * @param tab - The active tab filter: 'connections' | 'all' | 'others'
 */
export function useConversations(account?: string, tab: 'connections' | 'all' | 'others' = 'connections'): UseConversationsResult {
  const [conversations, setConversations] = useState<CachedConversation[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const initRef = useRef(false);
  const pollRef = useRef<number | null>(null);

  // Refresh conversations from cache
  const refreshConversations = useCallback(async () => {
    try {
      const cachedConvs = await getCachedConversations(tab, account);
      console.log(`[useConversations] Refreshed ${cachedConvs.length} conversations for tab="${tab}"`, cachedConvs);
      setConversations(cachedConvs);
    } catch (e) {
      console.error("Failed to refresh conversations:", e);
    }
  }, [account, tab]);

  // Initialize sync engine and start syncing
  const initializeSync = useCallback(async () => {
    try {
      setLoading(true);
      setSyncing(true);
      setError(null);

      const status = await initSyncEngine(account);
      setSyncStatus(status);

      // Immediately try to load cached conversations
      const cachedConvs = await getCachedConversations(tab, account);
      console.log(`[useConversations] Initial load: ${cachedConvs.length} conversations for tab="${tab}"`, cachedConvs);
      if (cachedConvs.length > 0) {
        setConversations(cachedConvs);
        setLoading(false);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setLoading(false);
    }
  }, [account, tab]);

  // Poll for sync status
  const pollSyncStatus = useCallback(async () => {
    try {
      const status = await getSyncStatus(account);
      setSyncStatus(status);

      const isSyncing =
        status.state === "syncing" || status.state === "initial_sync";
      setSyncing(isSyncing);

      if (status.is_online || status.last_sync) {
        const cachedConvs = await getCachedConversations(tab, account);
        setConversations(cachedConvs);
        setLoading(false);
      }

      if (status.error) {
        setError(status.error);
      }
    } catch (e) {
      console.error("Failed to poll sync status:", e);
    }
  }, [account, tab]);

  // Trigger a manual sync
  const triggerSync = useCallback(async () => {
    try {
      setSyncing(true);
      await initialSync(account);
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

  // Refresh conversations when tab changes
  useEffect(() => {
    if (initRef.current) {
      refreshConversations();
    }
  }, [tab, refreshConversations]);

  // Listen for Tauri sync events
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      unlisten = await onSyncEvent((payload) => {
        if ("StatusChanged" in payload && payload.StatusChanged) {
          const status = payload.StatusChanged;
          setSyncStatus(status);

          const isSyncing =
            status.state === "syncing" || status.state === "initial_sync";
          setSyncing(isSyncing);

          if (status.error) {
            setError(status.error);
          }
        }

        if ("ConversationsUpdated" in payload) {
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

  // Fallback polling (60 seconds)
  useEffect(() => {
    pollRef.current = window.setInterval(pollSyncStatus, 60000);

    return () => {
      if (pollRef.current) {
        window.clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [pollSyncStatus]);

  // Format conversations for display
  const formattedConversations = conversations.map(formatConversation);

  return {
    conversations: formattedConversations,
    loading,
    syncing,
    error,
    syncStatus,
    refresh: refreshConversations,
    triggerSync,
  };
}
