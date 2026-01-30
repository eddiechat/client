import { useState, useEffect, useCallback } from "react";
import { listAccounts, getDefaultAccount } from "../../../tauri";
import type { EmailAccount } from "../../../tauri";

interface UseAccountsResult {
  accounts: EmailAccount[];
  currentAccount: string | null;
  setCurrentAccount: (account: string | null) => void;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Hook for managing email accounts.
 *
 * Provides:
 * - List of configured accounts
 * - Current active account
 * - Loading and error states
 * - Refresh functionality
 */
export function useAccounts(): UseAccountsResult {
  const [accounts, setAccounts] = useState<EmailAccount[]>([]);
  const [currentAccount, setCurrentAccount] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAccounts = useCallback(async () => {
    try {
      setLoading(true);
      const [accountList, defaultAccount] = await Promise.all([
        listAccounts(),
        getDefaultAccount(),
      ]);
      setAccounts(accountList);
      setCurrentAccount(defaultAccount);
      setError(null);
    } catch (e) {
      console.error("useAccounts: Error fetching accounts:", e);
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
