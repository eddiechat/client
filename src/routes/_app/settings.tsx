import { useState, useEffect, useCallback } from "react";
import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useAuth, useTheme } from "../../shared/context";
import { SettingsToggle, SettingsSelect, Avatar } from "../../shared/components";
import { getSetting, setSetting } from "../../tauri";

export const Route = createFileRoute("/_app/settings")({
  component: SettingsScreen,
});

const SETTING_KEYS = {
  hideOlderChats: "hide_older_chats",
  showToaster: "show_toaster",
} as const;

const TOGGLE_DEFAULTS: Record<string, boolean> = {
  [SETTING_KEYS.showToaster]: false,
};

const CHAT_AGE_STEPS = ["1", "2", "3", "4", "all"] as const;
const CHAT_AGE_LABELS: Record<string, string> = { "1": "1w", "2": "2w", "3": "3w", "4": "4w", "all": "All" };

const THEME_OPTIONS = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

function SettingsScreen() {
  const router = useRouter();
  const { email } = useAuth();
  const { theme, setTheme } = useTheme();
  const [toggles, setToggles] = useState<Record<string, boolean>>(TOGGLE_DEFAULTS);
  const [chatAge, setChatAge] = useState<string>("all");

  useEffect(() => {
    async function load() {
      const keys = Object.values(SETTING_KEYS);
      const results = await Promise.all(keys.map((k) => getSetting(k).then((v) => [k, v] as const)));
      for (const [k, v] of results) {
        if (v === null) continue;
        if (k === SETTING_KEYS.hideOlderChats) setChatAge(v);
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
    {
      section: "Appearance", items: [
        { label: "Show status toaster", desc: "Show sync status at the bottom", key: SETTING_KEYS.showToaster },
      ]
    },
  ];


  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-gradient)" }}>
      {/* Header */}
      <div className="flex items-center gap-3 px-4 pb-3 border-b border-divider shrink-0" style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))' }}>
        <button className="border-none bg-transparent text-[28px] cursor-pointer text-text-muted min-w-10 min-h-10 flex items-center justify-center -ml-1 font-bold" onClick={() => router.history.back()}>
          &#8249;
        </button>
        <span className="font-extrabold text-[15px] text-text-primary" style={{ letterSpacing: "-0.2px" }}>Settings</span>
      </div>

      <div className="flex-1 overflow-y-auto flex flex-col">
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

        {/* Setting sections */}
        {settingsSections.map((group) => (
          <div key={group.section} className="px-5 pb-2">
            <div className="text-[11px] font-bold text-text-dim tracking-[0.08em] mb-2 mt-2">{group.section.toUpperCase()}</div>
            {group.section === "Appearance" && (<>
              <SettingsSelect label="Theme" desc="Choose light, dark, or system" value={theme} options={THEME_OPTIONS} onChange={(v) => setTheme(v as "light" | "dark" | "system")} />
              <div className="py-3 border-b border-divider">
                <div className="flex justify-between items-baseline">
                  <div>
                    <div className="text-[14px] font-medium text-text-primary">Show chats from</div>
                    <div className="text-[12px] text-text-dim mt-px">How far back to show in the list</div>
                  </div>
                  <span className="text-[13px] font-bold text-accent-green">{CHAT_AGE_LABELS[chatAge]}</span>
                </div>
                <div className="flex items-center gap-0 mt-3">
                  {CHAT_AGE_STEPS.map((step, i) => (
                    <button
                      key={step}
                      className={`flex-1 py-1.5 text-[11px] font-bold border border-divider cursor-pointer transition-colors ${chatAge === step
                        ? "bg-accent-green text-white border-accent-green"
                        : "bg-bg-tertiary text-text-muted"
                        } ${i === 0 ? "rounded-l-lg" : ""} ${i === CHAT_AGE_STEPS.length - 1 ? "rounded-r-lg" : ""} ${i > 0 ? "-ml-px" : ""}`}
                      onClick={() => { setChatAge(step); setSetting(SETTING_KEYS.hideOlderChats, step); }}
                    >
                      {CHAT_AGE_LABELS[step]}
                    </button>
                  ))}
                </div>
              </div>
            </>)}
            {group.items.map((item) => (
              <SettingsToggle key={item.key} label={item.label} desc={item.desc} value={toggles[item.key]} onChange={(v) => persistToggle(item.key, v)} />
            ))}
          </div>
        ))}

        <div className="py-4 text-center mt-auto">
          <span className="text-[12px] text-text-dim">Eddie is open source • We value privacy • We never touch your data.</span>
        </div>
      </div>
    </div>
  );
}
