import { useState, useEffect, useCallback } from "react";
import { ReadOnlyToggle } from "../../../shared/components";
import {
  getOllamaConfig,
  saveOllamaConfig,
  testOllamaConnection,
  reclassifyWithOllama,
} from "../../../tauri";

interface SettingsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  currentAccount: string | null;
}

export function SettingsDialog({
  isOpen,
  onClose,
  currentAccount,
}: SettingsDialogProps) {
  // Ollama state
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [ollamaModel, setOllamaModel] = useState("mistral:latest");
  const [ollamaEnabled, setOllamaEnabled] = useState(false);

  // Track initial values to detect changes
  const [initialUrl, setInitialUrl] = useState("");
  const [initialModel, setInitialModel] = useState("");
  const [initialEnabled, setInitialEnabled] = useState(false);

  // UI state
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [reclassifying, setReclassifying] = useState(false);
  const [reclassifyCount, setReclassifyCount] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Load settings on open
  useEffect(() => {
    if (isOpen) {
      setError(null);
      setTestResult(null);
      setReclassifyCount(null);
      getOllamaConfig()
        .then((config) => {
          setOllamaUrl(config.url);
          setOllamaModel(config.model);
          setOllamaEnabled(config.enabled);
          setInitialUrl(config.url);
          setInitialModel(config.model);
          setInitialEnabled(config.enabled);
        })
        .catch(console.error);
    }
  }, [isOpen]);

  // Escape key handling
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && isOpen && !saving && !reclassifying) onClose();
    };
    if (isOpen) {
      document.addEventListener("keydown", handleEscape);
      return () => document.removeEventListener("keydown", handleEscape);
    }
  }, [isOpen, saving, reclassifying, onClose]);

  const handleTestConnection = useCallback(async () => {
    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      const result = await testOllamaConnection(ollamaUrl, ollamaModel);
      setTestResult(result);
    } catch (err) {
      setTestResult(false);
      if (typeof err === "object" && err !== null && "message" in err) {
        setError(String((err as { message: string }).message));
      } else {
        setError(String(err));
      }
    } finally {
      setTesting(false);
    }
  }, [ollamaUrl, ollamaModel]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setError(null);
    setReclassifyCount(null);
    try {
      await saveOllamaConfig(ollamaUrl, ollamaModel, ollamaEnabled);

      // If enabling or changing model/url, trigger reclassification
      const configChanged =
        ollamaUrl !== initialUrl || ollamaModel !== initialModel;
      if (ollamaEnabled && (configChanged || !initialEnabled)) {
        setReclassifying(true);
        try {
          const count = await reclassifyWithOllama(
            currentAccount ?? undefined
          );
          setReclassifyCount(count);
        } catch (err) {
          console.error("Reclassification error:", err);
          // Non-fatal: settings were saved, reclassification failed
          if (typeof err === "object" && err !== null && "message" in err) {
            setError(
              `Settings saved, but reclassification failed: ${(err as { message: string }).message}`
            );
          }
        } finally {
          setReclassifying(false);
        }
      }

      // Update initial values
      setInitialUrl(ollamaUrl);
      setInitialModel(ollamaModel);
      setInitialEnabled(ollamaEnabled);
    } catch (err) {
      if (typeof err === "object" && err !== null && "message" in err) {
        setError(String((err as { message: string }).message));
      } else {
        setError(String(err));
      }
    } finally {
      setSaving(false);
    }
  }, [
    ollamaUrl,
    ollamaModel,
    ollamaEnabled,
    initialUrl,
    initialModel,
    initialEnabled,
    currentAccount,
  ]);

  if (!isOpen) return null;

  const inputClass =
    "w-full px-3.5 py-2.5 bg-bg-tertiary border border-divider rounded-lg text-text-primary text-[15px] outline-none focus:border-accent-blue transition-colors placeholder:text-text-muted disabled:opacity-50";
  const btnPrimary =
    "px-5 py-2.5 rounded-lg text-sm font-medium bg-bubble-sent text-white hover:brightness-110 transition-all disabled:opacity-50";
  const btnSecondary =
    "px-4 py-2.5 rounded-lg text-sm font-medium bg-bg-tertiary text-text-primary hover:bg-bg-hover transition-colors disabled:opacity-50";

  const isProcessing = saving || reclassifying;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4 safe-y">
      <div className="w-full max-w-lg bg-bg-secondary rounded-2xl flex flex-col max-h-[90vh] overflow-hidden shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-divider">
          <h2 className="text-lg font-semibold text-text-primary">Settings</h2>
          <button
            className="w-8 h-8 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors text-xl text-text-muted"
            onClick={onClose}
            disabled={isProcessing}
          >
            x
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-5 flex flex-col gap-4">
          {error && (
            <div className="px-4 py-3 bg-accent-red/15 border border-accent-red/30 rounded-lg text-sm text-accent-red">
              {error}
            </div>
          )}

          {reclassifyCount !== null && !error && (
            <div className="px-4 py-3 bg-green-600/15 border border-green-600/30 rounded-lg text-sm text-green-400">
              Reclassified {reclassifyCount} messages with Ollama.
            </div>
          )}

          {/* General section */}
          <fieldset className="border border-divider rounded-lg p-4 flex flex-col gap-3">
            <legend className="px-2 text-sm font-medium text-text-muted">
              General
            </legend>
            <ReadOnlyToggle />
          </fieldset>

          {/* Ollama section */}
          <fieldset className="border border-divider rounded-lg p-4 flex flex-col gap-3">
            <legend className="px-2 text-sm font-medium text-text-muted">
              AI Classification (Ollama)
            </legend>

            {/* Enable toggle */}
            <div className="flex items-center justify-between">
              <label className="text-sm font-medium text-text-primary">
                Enable Ollama classification
              </label>
              <button
                type="button"
                role="switch"
                aria-checked={ollamaEnabled}
                onClick={() => setOllamaEnabled(!ollamaEnabled)}
                disabled={isProcessing}
                className={`
                  relative inline-flex h-6 w-14 items-center rounded-full
                  transition-colors duration-200 ease-in-out
                  focus:outline-none focus:ring-2 focus:ring-offset-2
                  disabled:opacity-50 disabled:cursor-not-allowed
                  ${
                    ollamaEnabled
                      ? "bg-green-600 focus:ring-green-500"
                      : "bg-bg-tertiary focus:ring-accent-blue"
                  }
                `}
              >
                <span
                  className={`
                    inline-block h-4 w-4 transform rounded-full bg-white
                    transition-transform duration-200 ease-in-out shadow-md
                    ${ollamaEnabled ? "translate-x-9" : "translate-x-1"}
                  `}
                />
              </button>
            </div>

            {/* URL */}
            <div className="flex flex-col gap-1">
              <label className="text-xs text-text-muted">Server URL</label>
              <input
                type="text"
                value={ollamaUrl}
                onChange={(e) => setOllamaUrl(e.target.value)}
                placeholder="http://localhost:11434"
                className={inputClass}
                disabled={isProcessing}
              />
            </div>

            {/* Model */}
            <div className="flex flex-col gap-1">
              <label className="text-xs text-text-muted">Model</label>
              <input
                type="text"
                value={ollamaModel}
                onChange={(e) => setOllamaModel(e.target.value)}
                placeholder="mistral:latest"
                className={inputClass}
                disabled={isProcessing}
              />
            </div>

            {/* Test Connection */}
            <div className="flex items-center gap-3">
              <button
                className={btnSecondary}
                onClick={handleTestConnection}
                disabled={isProcessing || testing || !ollamaUrl}
              >
                {testing ? "Testing..." : "Test Connection"}
              </button>
              {testResult !== null && (
                <span
                  className={`text-sm font-medium ${testResult ? "text-green-400" : "text-accent-red"}`}
                >
                  {testResult ? "Connected" : "Failed"}
                </span>
              )}
            </div>
          </fieldset>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 px-5 py-4 border-t border-divider">
          <button
            className={btnSecondary}
            onClick={onClose}
            disabled={isProcessing}
          >
            Close
          </button>
          <button
            className={btnPrimary}
            onClick={handleSave}
            disabled={isProcessing}
          >
            {saving
              ? "Saving..."
              : reclassifying
                ? "Reclassifying..."
                : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
