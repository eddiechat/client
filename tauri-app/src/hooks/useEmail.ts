import { useState, useEffect, useCallback } from "react";
import * as api from "../lib/api";
import type {
  Account,
  Envelope,
  Folder,
  Message,
  ListEnvelopesRequest,
} from "../types";

// Hook for managing accounts
export function useAccounts() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [currentAccount, setCurrentAccount] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAccounts = useCallback(async () => {
    try {
      setLoading(true);
      const [accountList, defaultAccount] = await Promise.all([
        api.listAccounts(),
        api.getDefaultAccount(),
      ]);
      setAccounts(accountList);
      setCurrentAccount(defaultAccount);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAccounts();
  }, [fetchAccounts]);

  return {
    accounts,
    currentAccount,
    setCurrentAccount,
    loading,
    error,
    refresh: fetchAccounts,
  };
}

// Hook for managing folders
export function useFolders(account?: string) {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [currentFolder, setCurrentFolder] = useState<string>("INBOX");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchFolders = useCallback(async () => {
    try {
      setLoading(true);
      const folderList = await api.listFolders(account);
      setFolders(folderList);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [account]);

  useEffect(() => {
    fetchFolders();
  }, [fetchFolders]);

  return {
    folders,
    currentFolder,
    setCurrentFolder,
    loading,
    error,
    refresh: fetchFolders,
  };
}

// Hook for managing envelopes (email list)
export function useEnvelopes(account?: string, folder?: string) {
  const [envelopes, setEnvelopes] = useState<Envelope[]>([]);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Reset page to 1 when account or folder changes
  useEffect(() => {
    setPage(1);
  }, [account, folder]);

  const fetchEnvelopes = useCallback(async () => {
    try {
      setLoading(true);
      const request: ListEnvelopesRequest = {
        account,
        folder,
        page,
        page_size: pageSize,
        query: query || undefined,
      };
      const response = await api.listEnvelopes(request);
      setEnvelopes(response.envelopes);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [account, folder, page, pageSize, query]);

  useEffect(() => {
    fetchEnvelopes();
  }, [fetchEnvelopes]);

  return {
    envelopes,
    page,
    setPage,
    pageSize,
    setPageSize,
    query,
    setQuery,
    loading,
    error,
    refresh: fetchEnvelopes,
  };
}

// Hook for reading a single message
export function useMessage(
  id: string | null,
  account?: string,
  folder?: string
) {
  const [message, setMessage] = useState<Message | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchMessage = useCallback(async () => {
    if (!id) {
      setMessage(null);
      return;
    }

    try {
      setLoading(true);
      const msg = await api.readMessage({
        account,
        folder,
        id,
        preview: false,
      });
      setMessage(msg);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [id, account, folder]);

  useEffect(() => {
    fetchMessage();
  }, [fetchMessage]);

  return {
    message,
    loading,
    error,
    refresh: fetchMessage,
  };
}

// Hook for email actions
export function useEmailActions(
  account?: string,
  folder?: string,
  onSuccess?: () => void
) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleAction = async <T>(action: () => Promise<T>): Promise<T | null> => {
    try {
      setLoading(true);
      setError(null);
      const result = await action();
      onSuccess?.();
      return result;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return null;
    } finally {
      setLoading(false);
    }
  };

  return {
    loading,
    error,
    deleteMessages: (ids: string[]) =>
      handleAction(() => api.deleteMessages(ids, account, folder)),
    moveMessages: (ids: string[], targetFolder: string) =>
      handleAction(() => api.moveMessages(ids, targetFolder, account, folder)),
    copyMessages: (ids: string[], targetFolder: string) =>
      handleAction(() => api.copyMessages(ids, targetFolder, account, folder)),
    markAsRead: (ids: string[]) =>
      handleAction(() => api.markAsRead(ids, account, folder)),
    markAsUnread: (ids: string[]) =>
      handleAction(() => api.markAsUnread(ids, account, folder)),
    toggleFlagged: (id: string, isFlagged: boolean) =>
      handleAction(() => api.toggleFlagged(id, isFlagged, account, folder)),
    downloadAttachments: (id: string) =>
      handleAction(() => api.downloadAttachments(id, account, folder)),
  };
}
