import type { Account } from "../../../tauri";

interface SidebarHeaderProps {
  accounts: Account[];
  currentAccount: string | null;
  onEditAccount: () => void;
  onCompose: () => void;
}

export function SidebarHeader({
  accounts,
  currentAccount,
  onEditAccount,
  onCompose,
}: SidebarHeaderProps) {
  return (
    <div
      className="flex items-center justify-between px-4"
      style={{
        minHeight: "4rem",
        paddingTop: "calc(0.75rem + env(safe-area-inset-top))",
        paddingBottom: "0.75rem",
      }}
    >
      <div className="flex flex-col gap-0.5">
        <div className="flex items-center gap-2">
          <img src="/eddie-swirl-green.svg" alt="Eddie logo" className="w-6 h-6" />
          <h1 className="text-xl font-semibold text-text-primary tracking-tight">
            eddie
          </h1>
        </div>
        {accounts.length > 0 && (
          <span
            className="text-xs text-text-muted cursor-pointer hover:text-accent-blue transition-colors"
            onClick={onEditAccount}
          >
            {currentAccount || "No account"}
          </span>
        )}
      </div>
      <button
        className="w-9 h-9 rounded-full bg-bg-tertiary flex items-center justify-center hover:bg-bg-hover transition-colors"
        onClick={onCompose}
        title="New message"
      >
        <svg
          className="w-5 h-5 text-text-primary"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
        </svg>
      </button>
    </div>
  );
}
