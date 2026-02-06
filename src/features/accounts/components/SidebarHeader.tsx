import type { EmailAccount } from "../../../tauri";
import { getAppVersion } from "../../../tauri";
import { useState, useEffect } from "react";
// import { dropAndResync } from "../../../tauri";

interface SidebarHeaderProps {
  accounts: EmailAccount[];
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
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    // Fetch version on mount
    const fetchVersion = async () => {
      try {
        const appVersion = await getAppVersion();
        const devSuffix = import.meta.env.DEV ? " (dev)" : "";
        setVersion(`${appVersion}${devSuffix}`);
      } catch (err) {
        console.error("Failed to get version:", err);
      }
    };

    fetchVersion();
  }, []);

  // const [isResyncing, setIsResyncing] = useState(false);

  // const handleResync = async () => {
  //   if (!confirm("This will delete your local database and re-fetch all messages. Continue?")) {
  //     return;
  //   }
  //   setIsResyncing(true);
  //   try {
  //     await dropAndResync(currentAccount || undefined);
  //   } catch (error) {
  //     console.error("Failed to resync:", error);
  //     alert(`Failed to resync: ${error}`);
  //   } finally {
  //     setIsResyncing(false);
  //   }
  // };

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
          <div className="flex items-baseline gap-1.5">
            <h1 className="text-xl font-semibold text-text-primary tracking-tight">
              eddie
            </h1>
            {version && (
              <span className="text-xs text-text-muted font-normal">
                v{version}
              </span>
            )}
          </div>
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
      <div className="flex gap-2 items-center">
        {/* <button
          className="w-9 h-9 rounded-full bg-bg-tertiary flex items-center justify-center hover:bg-bg-hover transition-colors disabled:opacity-50 text-lg"
          onClick={handleResync}
          disabled={isResyncing}
          title="Drop database and re-sync all messages"
        >
          ☠️
        </button> */}
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
    </div>
  );
}
