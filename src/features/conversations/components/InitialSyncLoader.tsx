import { LoadingSpinner } from "../../../shared/components";
import type { SyncStatus } from "../../../tauri";

interface InitialSyncLoaderProps {
  syncStatus: SyncStatus | null;
}

export function InitialSyncLoader({ syncStatus }: InitialSyncLoaderProps) {
  // Get the current progress message, or show a default
  const message = syncStatus?.progress?.message || "Connecting to mail server";

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 bg-bg-secondary">
      <div className="flex flex-col items-center gap-6 max-w-sm">
        <LoadingSpinner className="w-12 h-12" />

        <div className="text-center space-y-2">
          <h2 className="text-xl font-semibold text-text-primary">
            Setting up your inbox
          </h2>
          <p className="text-base text-text-secondary">
            {message}
          </p>
        </div>
      </div>
    </div>
  );
}
