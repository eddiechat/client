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
      console.log("useAccounts: Fetching accounts...");
      setLoading(true);
      const [accountList, defaultAccount] = await Promise.all([
        api.listAccounts(),
        api.getDefaultAccount(),
      ]);
      console.log("useAccounts: Got", accountList.length, "accounts, default:", defaultAccount);
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
