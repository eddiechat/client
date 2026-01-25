import type { Account } from "../types";

interface AccountSelectorProps {
  accounts: Account[];
  currentAccount: string | null;
  onAccountChange: (account: string) => void;
  onEditAccount?: () => void;
  loading?: boolean;
}

export function AccountSelector({
  accounts,
  currentAccount,
  onAccountChange,
  onEditAccount,
  loading,
}: AccountSelectorProps) {
  if (loading) {
    return <div className="account-selector loading">Loading...</div>;
  }

  if (accounts.length === 0) {
    return (
      <div className="account-selector empty">
        No accounts configured
      </div>
    );
  }

  const currentAccountData = accounts.find((a) => a.name === currentAccount);

  return (
    <div className="account-selector">
      {accounts.length === 1 ? (
        <span className="account-name">
          {currentAccountData?.name} ({currentAccountData?.backend})
        </span>
      ) : (
        <select
          value={currentAccount || ""}
          onChange={(e) => onAccountChange(e.target.value)}
        >
          {accounts.map((account) => (
            <option key={account.name} value={account.name}>
              {account.name} ({account.backend})
              {account.is_default ? " ★" : ""}
            </option>
          ))}
        </select>
      )}
      {onEditAccount && currentAccount && (
        <button
          type="button"
          className="edit-account-btn"
          onClick={onEditAccount}
          title="Edit account"
        >
          Edit
        </button>
      )}
    </div>
  );
}
