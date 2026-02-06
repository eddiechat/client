import { useState, useEffect } from "react";
import { getReadOnlyMode, setReadOnlyMode } from "../../tauri";

interface ReadOnlyToggleProps {
  label?: string;
}

export function ReadOnlyToggle({ label = "Protected read-only mode" }: ReadOnlyToggleProps) {
  const [readOnlyMode, setReadOnlyModeState] = useState<boolean>(true);
  const [isToggling, setIsToggling] = useState(false);

  useEffect(() => {
    // Fetch read-only mode on mount
    const fetchReadOnlyMode = async () => {
      try {
        const mode = await getReadOnlyMode();
        setReadOnlyModeState(mode);
      } catch (err) {
        console.error("Failed to get read-only mode:", err);
      }
    };

    fetchReadOnlyMode();
  }, []);

  const handleToggle = async () => {
    if (isToggling) return;

    setIsToggling(true);
    try {
      const newMode = !readOnlyMode;
      await setReadOnlyMode(newMode);
      setReadOnlyModeState(newMode);
    } catch (err) {
      console.error("Failed to toggle read-only mode:", err);
      alert(`Failed to toggle read-only mode: ${err}`);
    } finally {
      setIsToggling(false);
    }
  };

  return (
    <div className="flex items-center justify-between">
      <label
        htmlFor="read-only-toggle"
        className="text-sm font-medium text-text-primary"
      >
        {label}
      </label>
      <button
        id="read-only-toggle"
        type="button"
        role="switch"
        aria-checked={readOnlyMode}
        onClick={handleToggle}
        disabled={isToggling}
        className={`
          relative inline-flex h-6 w-14 items-center rounded-full
          transition-colors duration-200 ease-in-out
          focus:outline-none focus:ring-2 focus:ring-offset-2
          disabled:opacity-50 disabled:cursor-not-allowed
          ${readOnlyMode
            ? 'bg-green-600 focus:ring-green-500'
            : 'bg-red-600 focus:ring-red-500'
          }
        `}
      >
        {/* Text label inside toggle - centered in visible area */}
        <span
          className={`
            absolute left-1 right-auto flex items-center justify-center text-[11px] font-bold text-white uppercase tracking-tight
            transition-opacity duration-200 pointer-events-none w-7
            ${readOnlyMode ? 'opacity-100' : 'opacity-0'}
          `}
        >
          ON
        </span>
        <span
          className={`
            absolute right-1 left-auto flex items-center justify-center text-[11px] font-bold text-white uppercase tracking-tight
            transition-opacity duration-200 pointer-events-none w-7
            ${readOnlyMode ? 'opacity-0' : 'opacity-100'}
          `}
        >
          OFF
        </span>

        {/* Toggle circle */}
        <span
          className={`
            inline-block h-4 w-4 transform rounded-full bg-white
            transition-transform duration-200 ease-in-out
            shadow-md
            ${readOnlyMode ? 'translate-x-9' : 'translate-x-1'}
          `}
        />
      </button>
    </div>
  );
}
