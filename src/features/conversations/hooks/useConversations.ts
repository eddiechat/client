import { useState, useEffect, useCallback, useRef } from "react";
import {
  initSyncEngine,
  getSyncStatus,
  getCachedConversations,
  syncNow,
  onSyncEvent,
} from "../../../tauri";
import type { Conversation, SyncStatus } from "../../../tauri";

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
 * Hydrate a backend Conversation with frontend display helpers.
 * Parses participant_key (newline-separated emails) and
 * participant_names (JSON Record<email, name>) into arrays.
 */
function hydrateConversation(conv: Conversation): Conversation {
  // Parse participant emails from participant_key
  const emails = conv.participant_key
    ? conv.participant_key.split("\n").filter((e) => e.length > 0)
    : [];

  // Parse names from JSON string: { "email": "Display Name", ... }
  let namesMap: Record<string, string> = {};
  if (conv.participant_names) {
    try {
      namesMap = JSON.parse(conv.participant_names);
    } catch {
      // Fallback: treat as empty
    }
  }

  const displayNames = emails.map((email) => namesMap[email] || email);

  return {
    ...conv,
    participants: emails,
    participant_display_names: displayNames,
  };
}

/**
 * Hook for managing conversations using the sync engine.
 *
 * On first load, initializes the sync engine which:
 * 1. Seeds onboarding tasks for the account
 * 2. Wakes the background worker to start syncing
 * 3. Returns current sync status
 *
 * Subsequent loads read directly from the local cache.
 *
 * @param account - The account to load conversations for
 * @param tab - The active tab filter: 'connections' | 'all' | 'others'
 */
export function useConversations(account?: string, tab: 'connections' | 'all' | 'others' = 'connections'): UseConversationsResult {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const initRef = useRef(false);
  const pollRef = useRef<number | null>(null);

  // Refresh conversations from cache
  const refreshConversations = useCallback(async () => {
    try {
      const rawConvs = await getCachedConversations(tab, account);
      setConversations(rawConvs.map(hydrateConversation));
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
      const rawConvs = await getCachedConversations(tab, account);
      console.log(`[useConversations] Initial load: ${rawConvs.length} conversations for tab="${tab}"`, rawConvs);
      if (rawConvs.length > 0) {
        setConversations(rawConvs.map(hydrateConversation));
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
        status.state === "syncing" || status.state === "pending";
      setSyncing(isSyncing);

      if (status.is_online || status.last_sync) {
        const rawConvs = await getCachedConversations(tab, account);
        setConversations(rawConvs.map(hydrateConversation));
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
      await syncNow();
      await refreshConversations();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSyncing(false);
    }
  }, [refreshConversations]);

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
            status.state === "syncing" || status.state === "pending";
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

  return {
    conversations,
    loading,
    syncing,
    error,
    syncStatus,
    refresh: refreshConversations,
    triggerSync,
  };
}
