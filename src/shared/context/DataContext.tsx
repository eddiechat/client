import { createContext, useContext, useState, useEffect, useCallback, useMemo } from "react";
import type { ReactNode } from "react";
import {
  fetchConversations,
  onSyncStatus,
  onConversationsUpdated,
} from "../../tauri";
import type { Conversation } from "../../tauri";

interface DataContextValue {
  conversations: Conversation[];
  status: string | undefined;
  refresh: (accountId: string) => Promise<void>;
}

const DataContext = createContext<DataContextValue | null>(null);

export function DataProvider({ children }: { children: ReactNode }) {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [status, setStatus] = useState<string | undefined>();

  useEffect(() => {
    const u = onSyncStatus((p) => setStatus(p.message || undefined));
    return () => { u.then((f) => f()); };
  }, []);

  useEffect(() => {
    const u = onConversationsUpdated(async (p) => {
      setConversations(await fetchConversations(p.account_id));
    });
    return () => { u.then((f) => f()); };
  }, []);

  const refresh = useCallback(async (accountId: string) => {
    setConversations(await fetchConversations(accountId));
  }, []);

  const value = useMemo<DataContextValue>(
    () => ({ conversations, status, refresh }),
    [conversations, status, refresh]
  );

  return <DataContext.Provider value={value}>{children}</DataContext.Provider>;
}

export function useData(): DataContextValue {
  const ctx = useContext(DataContext);
  if (!ctx) throw new Error("useData must be used within DataProvider");
  return ctx;
}
