import { useState, useEffect, useCallback } from "react";
import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useAuth, useTheme } from "../../shared/context";
import { SettingsToggle, SettingsSelect, Avatar } from "../../shared/components";
import { getSetting, setSetting, getOllamaModels } from "../../tauri";

export const Route = createFileRoute("/_app/settings")({
  component: SettingsScreen,
});

const SETTING_KEYS = {
  ollamaUrl: "ollama_url",
  notifPoints: "notif_points",
  notifCircles: "notif_circles",
  notifLines: "notif_lines",
  compactList: "compact_list",
} as const;

const TOGGLE_DEFAULTS: Record<string, boolean> = {
  [SETTING_KEYS.notifPoints]: true,
  [SETTING_KEYS.notifCircles]: true,
  [SETTING_KEYS.notifLines]: false,
  [SETTING_KEYS.compactList]: false,
};

const THEME_OPTIONS = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

function SettingsScreen() {
  const router = useRouter();
  const { email } = useAuth();
  const { theme, setTheme } = useTheme();
  const [ollamaUrl, setOllamaUrl] = useState("");
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [toggles, setToggles] = useState<Record<string, boolean>>(TOGGLE_DEFAULTS);

  useEffect(() => {
    getOllamaModels("__DEFAULT__").then((data) => {
      setOllamaModels(data.models);
      setSelectedModel(data.selected_model);
    });
  }, []);

  useEffect(() => {
    async function load() {
      const keys = Object.values(SETTING_KEYS);
      const results = await Promise.all(keys.map((k) => getSetting(k).then((v) => [k, v] as const)));
      for (const [k, v] of results) {
        if (v === null) continue;
        if (k === SETTING_KEYS.ollamaUrl) setOllamaUrl(v);
        else setToggles((prev) => ({ ...prev, [k]: v === "true" }));
      }
    }
    load();
  }, []);

  const persistToggle = useCallback((key: string, value: boolean) => {
    setToggles((prev) => ({ ...prev, [key]: value }));
    setSetting(key, String(value));
  }, []);

  const settingsSections = [
    { section: "Notifications", items: [
      { label: "Point messages", desc: "Direct conversations", key: SETTING_KEYS.notifPoints },
      { label: "Circle messages", desc: "Groups and communities", key: SETTING_KEYS.notifCircles },
      { label: "Line updates", desc: "New matches in your Lines", key: SETTING_KEYS.notifLines },
    ]},
    { section: "Appearance", items: [
      { label: "Compact list", desc: "Reduce spacing in lists", key: SETTING_KEYS.compactList },
    ]},
  ];

  return (
    <div className="flex flex-col h-screen bg-bg-primary">
      {/* Header */}
      <div className="flex items-center gap-3 px-5 pb-3 border-b border-divider shrink-0 bg-bg-secondary" style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))' }}>
        <button className="border-none bg-transparent text-[24px] cursor-pointer text-accent-green p-0 leading-none" onClick={() => router.history.back()}>
          &#8249;
        </button>
        <span className="font-bold text-[19px] text-text-primary">Settings</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {/* Account card */}
        <div className="p-5">
          <div className="p-4 rounded-2xl bg-bg-secondary border border-divider flex items-center gap-3.5">
            <Avatar name={email || "E"} email={email || undefined} size={13} fontSize="text-[22px]" className="shrink-0" />
            <div>
              <div className="text-[16px] font-bold text-text-primary">Personal</div>
              <div className="text-[13px] text-text-muted">{email}</div>
              <div className="flex items-center gap-1 mt-0.5">
                <div className="w-[5px] h-[5px] rounded-full bg-accent-green" />
                <span className="text-[11px] text-accent-green font-semibold">IMAP connected</span>
              </div>
            </div>
          </div>
        </div>

        {/* Ollama section */}
        <div className="px-5 pb-2">
          <div className="text-[11px] font-bold text-text-dim tracking-[0.08em] mb-2 mt-2">OLLAMA</div>
          <div className="flex flex-col gap-1.5 py-3 border-b border-divider">
            <div>
              <div className="text-[14px] font-medium text-text-primary">URL</div>
              <div className="text-[12px] text-text-dim mt-px">Default Ollama server endpoint</div>
            </div>
            <input
              className="w-full px-3 py-2 rounded-lg border border-divider bg-bg-tertiary text-[14px] text-text-primary font-(--font-body) outline-none transition-colors focus:border-accent-green placeholder:text-text-dim"
              placeholder="http://localhost:11434"
              value={ollamaUrl}
              onChange={(e) => setOllamaUrl(e.target.value)}
              onBlur={() => setSetting(SETTING_KEYS.ollamaUrl, ollamaUrl)}
            />
          </div>
          <div className="flex flex-col gap-1.5 py-3">
            <div>
              <div className="text-[14px] font-medium text-text-primary">Model</div>
              <div className="text-[12px] text-text-dim mt-px">Default model for classification</div>
            </div>
            {ollamaModels.length > 0 ? (
              <select
                className="w-full px-3 h-10 rounded-lg border border-divider bg-bg-tertiary text-[14px] text-text-primary font-(--font-body) outline-none transition-colors focus:border-accent-green appearance-none"
                value={selectedModel ?? ""}
                onChange={(e) => { setSelectedModel(e.target.value); setSetting("ollama_model", e.target.value); }}
              >
                {!selectedModel && <option value="" disabled>Select model</option>}
                {ollamaModels.map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
            ) : (
              <select
                className="w-full px-3 h-10 rounded-lg border border-divider bg-bg-tertiary text-[14px] text-text-dim font-(--font-body) outline-none appearance-none opacity-60 cursor-not-allowed"
                disabled
              >
                <option>Ollama not found</option>
              </select>
            )}
          </div>
        </div>

        {/* Setting sections */}
        {settingsSections.map((group) => (
          <div key={group.section} className="px-5 pb-2">
            <div className="text-[11px] font-bold text-text-dim tracking-[0.08em] mb-2 mt-2">{group.section.toUpperCase()}</div>
            {group.section === "Appearance" && (
              <SettingsSelect label="Theme" desc="Choose light, dark, or system" value={theme} options={THEME_OPTIONS} onChange={(v) => setTheme(v as "light" | "dark" | "system")} />
            )}
            {group.items.map((item) => (
              <SettingsToggle key={item.key} label={item.label} desc={item.desc} value={toggles[item.key]} onChange={(v) => persistToggle(item.key, v)} />
            ))}
          </div>
        ))}

        {/* Open source info */}
        <div className="px-5 pt-4">
          <div className="p-3.5 rounded-xl bg-green-bg border border-green-border">
            <div className="text-[13px] font-bold text-accent-green mb-1">{"\uD83D\uDD12"} Eddie is open source</div>
            <div className="text-[12px] text-text-muted leading-relaxed">Audit the code, contribute, or fork it. Your data never touches our servers.</div>
          </div>
        </div>

        <div className="py-4 text-center">
          <span className="text-[12px] text-text-dim">Eddie v0.4.2 &middot; Built on email</span>
        </div>
      </div>
    </div>
  );
}
