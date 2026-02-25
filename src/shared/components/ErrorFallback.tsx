import { useState } from "react";
import { useRouter } from "@tanstack/react-router";

export function ErrorFallback({ error }: { error: Error }) {
  const [showDetails, setShowDetails] = useState(false);
  const router = useRouter();

  return (
    <div
      className="flex flex-col items-center justify-center min-h-screen bg-bg-primary px-6 text-center"
      style={{ paddingTop: "env(safe-area-inset-top, 0px)" }}
    >
      <div className="text-[40px] mb-4">:(</div>
      <h2 className="text-[18px] font-semibold text-text-primary mb-2">
        Something went wrong
      </h2>
      <p className="text-[14px] text-text-muted mb-6 max-w-[280px]">
        An unexpected error occurred. You can go back or return to the home screen.
      </p>

      <div className="flex gap-3 mb-6">
        <button
          className="px-4 py-2.5 rounded-xl bg-bg-secondary border border-divider text-[14px] font-medium text-text-secondary cursor-pointer"
          onClick={() => router.history.back()}
        >
          Go back
        </button>
        <button
          className="px-4 py-2.5 rounded-xl bg-accent-green text-white text-[14px] font-semibold cursor-pointer border-none"
          onClick={() => router.navigate({ to: "/points" })}
        >
          Go home
        </button>
      </div>

      <button
        className="text-[12px] text-text-dim bg-transparent border-none cursor-pointer underline"
        onClick={() => setShowDetails((d) => !d)}
      >
        {showDetails ? "Hide" : "Show"} error details
      </button>
      {showDetails && (
        <pre className="mt-3 p-3 rounded-lg bg-bg-tertiary border border-divider text-[11px] text-text-muted text-left max-w-full overflow-x-auto whitespace-pre-wrap break-words">
          {error.message}
        </pre>
      )}
    </div>
  );
}
