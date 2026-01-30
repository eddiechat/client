import { createContext, useContext, type ReactNode } from "react";
import { useAccounts } from "../hooks/useAccounts";
import type { Account } from "../../../tauri";

interface AccountContextValue {
  accounts: Account[];
  currentAccount: string | null;
  setCurrentAccount: (account: string | null) => void;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

const AccountContext = createContext<AccountContextValue | null>(null);

interface AccountProviderProps {
  children: ReactNode;
}

export function AccountProvider({ children }: AccountProviderProps) {
  const accountState = useAccounts();

  return (
    <AccountContext.Provider value={accountState}>
      {children}
    </AccountContext.Provider>
  );
}

export function useAccountContext(): AccountContextValue {
  const context = useContext(AccountContext);
  if (!context) {
    throw new Error("useAccountContext must be used within an AccountProvider");
  }
  return context;
}
