import { useState, useEffect, useCallback, useRef } from "react";
import {
  SyncStatus,
  CachedConversation,
  CachedMessage,
  initSyncEngine,
  getSyncStatus,
  syncFolder,
  initialSync,
  getCachedConversations,
  getCachedConversationMessages,
  queueSyncAction,
  setSyncOnline,
  hasPendingSyncActions,
} from "../lib/api";

export interface UseSyncOptions {
  account?: string;
  autoInit?: boolean;
  pollInterval?: number; // Status poll interval in ms
}

export interface UseSyncReturn {
  // Status
  status: SyncStatus | null;
  isInitialized: boolean;
  isOnline: boolean;
  isSyncing: boolean;
  hasPendingActions: boolean;
  error: string | null;

  // Actions
  initialize: () => Promise<void>;
  sync: (folder?: string) => Promise<void>;
  runInitialSync: () => Promise<void>;
  setOnline: (online: boolean) => Promise<void>;

  // Data
  conversations: CachedConversation[];
  refreshConversations: () => Promise<void>;

  // Message actions with offline support
  markRead: (folder: string, uids: number[]) => Promise<void>;
  markUnread: (folder: string, uids: number[]) => Promise<void>;
  deleteMessages: (folder: string, uids: number[]) => Promise<void>;
  moveMessages: (folder: string, uids: number[], targetFolder: string) => Promise<void>;
}

export function useSync(options: UseSyncOptions = {}): UseSyncReturn {
  const { account, autoInit = true, pollInterval = 5000 } = options;

  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [conversations, setConversations] = useState<CachedConversation[]>([]);
  const [hasPending, setHasPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pollRef = useRef<number | null>(null);
  const initRef = useRef(false);

  // Initialize sync engine
  const initialize = useCallback(async () => {
    try {
      setError(null);
      const newStatus = await initSyncEngine(account);
      setStatus(newStatus);
      setIsInitialized(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setIsInitialized(false);
    }
  }, [account]);

  // Refresh status
  const refreshStatus = useCallback(async () => {
    if (!isInitialized) return;

    try {
      const [newStatus, pending] = await Promise.all([
        getSyncStatus(account),
        hasPendingSyncActions(account),
      ]);
      setStatus(newStatus);
      setHasPending(pending);
    } catch (e) {
      console.error("Failed to refresh sync status:", e);
    }
  }, [account, isInitialized]);

  // Sync a folder
  const sync = useCallback(
    async (folder?: string) => {
      if (!isInitialized) {
        await initialize();
      }

      try {
        setError(null);
        await syncFolder(folder, account);
        await refreshStatus();
        await refreshConversations();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [account, isInitialized, initialize]
  );

  // Run initial sync
  const runInitialSync = useCallback(async () => {
    if (!isInitialized) {
      await initialize();
    }

    try {
      setError(null);
      await initialSync(account);
      await refreshStatus();
      await refreshConversations();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [account, isInitialized, initialize]);

  // Set online status
  const setOnline = useCallback(
    async (online: boolean) => {
      try {
        await setSyncOnline(online, account);
        await refreshStatus();
      } catch (e) {
        console.error("Failed to set online status:", e);
      }
    },
    [account, refreshStatus]
  );

  // Refresh conversations from cache
  const refreshConversations = useCallback(async () => {
    if (!isInitialized) return;

    try {
      const convs = await getCachedConversations(false, account);
      setConversations(convs);
    } catch (e) {
      console.error("Failed to refresh conversations:", e);
    }
  }, [account, isInitialized]);

  // Mark messages as read
  const markRead = useCallback(
    async (folder: string, uids: number[]) => {
      try {
        await queueSyncAction("add_flags", folder, uids, ["\\Seen"], undefined, account);
        await refreshStatus();
      } catch (e) {
        console.error("Failed to mark read:", e);
      }
    },
    [account, refreshStatus]
  );

  // Mark messages as unread
  const markUnread = useCallback(
    async (folder: string, uids: number[]) => {
      try {
        await queueSyncAction("remove_flags", folder, uids, ["\\Seen"], undefined, account);
        await refreshStatus();
      } catch (e) {
        console.error("Failed to mark unread:", e);
      }
    },
    [account, refreshStatus]
  );

  // Delete messages
  const deleteMessages = useCallback(
    async (folder: string, uids: number[]) => {
      try {
        await queueSyncAction("delete", folder, uids, undefined, undefined, account);
        await refreshStatus();
        await refreshConversations();
      } catch (e) {
        console.error("Failed to delete messages:", e);
      }
    },
    [account, refreshStatus, refreshConversations]
  );

  // Move messages
  const moveMessages = useCallback(
    async (folder: string, uids: number[], targetFolder: string) => {
      try {
        await queueSyncAction("move", folder, uids, undefined, targetFolder, account);
        await refreshStatus();
        await refreshConversations();
      } catch (e) {
        console.error("Failed to move messages:", e);
      }
    },
    [account, refreshStatus, refreshConversations]
  );

  // Auto-initialize on mount
  useEffect(() => {
    if (autoInit && !initRef.current) {
      initRef.current = true;
      initialize();
    }
  }, [autoInit, initialize]);

  // Poll for status updates
  useEffect(() => {
    if (!isInitialized || pollInterval <= 0) return;

    const poll = () => {
      refreshStatus();
    };

    pollRef.current = window.setInterval(poll, pollInterval);

    return () => {
      if (pollRef.current) {
        window.clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [isInitialized, pollInterval, refreshStatus]);

  // Load conversations when initialized
  useEffect(() => {
    if (isInitialized) {
      refreshConversations();
    }
  }, [isInitialized, refreshConversations]);

  return {
    // Status
    status,
    isInitialized,
    isOnline: status?.is_online ?? false,
    isSyncing: status?.state === "syncing" || status?.state === "initial_sync",
    hasPendingActions: hasPending,
    error,

    // Actions
    initialize,
    sync,
    runInitialSync,
    setOnline,

    // Data
    conversations,
    refreshConversations,

    // Message actions
    markRead,
    markUnread,
    deleteMessages,
    moveMessages,
  };
}

// Hook to get messages for a specific conversation
export function useCachedConversationMessages(
  conversationId: number | null,
  account?: string
) {
  const [messages, setMessages] = useState<CachedMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (conversationId === null) {
      setMessages([]);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const msgs = await getCachedConversationMessages(conversationId, account);
      setMessages(msgs);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [conversationId, account]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { messages, loading, error, refresh };
}

export default useSync;
