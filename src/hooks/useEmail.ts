import { useState, useEffect, useCallback } from "react";
import * as api from "../lib/api";
import type { Account } from "../types";

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
