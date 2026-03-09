import { useState, useEffect, useCallback } from "react";
import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useAuth, useTheme } from "../../shared/context";
import { SettingsToggle, SettingsSelect, Avatar } from "../../shared/components";
import { getSetting, setSetting, getAccount, updateAccount } from "../../tauri";
import type { AccountDetails } from "../../tauri";

export const Route = createFileRoute("/_app/settings")({
  component: SettingsScreen,
});

const SETTING_KEYS = {
  hideOlderChats: "hide_older_chats",
  showToaster: "show_toaster",
  readOnly: "read_only",
} as const;

const TOGGLE_DEFAULTS: Record<string, boolean> = {
  [SETTING_KEYS.showToaster]: false,
  [SETTING_KEYS.readOnly]: true,
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
  const { email, accountId } = useAuth();
  const { theme, setTheme } = useTheme();
  const [toggles, setToggles] = useState<Record<string, boolean>>(TOGGLE_DEFAULTS);
  const [chatAge, setChatAge] = useState<string>("all");
  const [editingAccount, setEditingAccount] = useState<AccountDetails | null>(null);
  const [accountForm, setAccountForm] = useState({
    displayName: "",
    password: "",
    imapHost: "",
    imapPort: "",
    imapTls: true,
    smtpHost: "",
    smtpPort: "",
    smtpTls: true,
    aliases: "",
  });
  const [saving, setSaving] = useState(false);

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

  const openAccountEdit = useCallback(async () => {
    if (!accountId) return;
    if (editingAccount) { setEditingAccount(null); return; }
    const details = await getAccount(accountId);
    setEditingAccount(details);
    setAccountForm({
      displayName: details.display_name ?? "",
      password: "",
      imapHost: details.imap_host,
      imapPort: String(details.imap_port),
      imapTls: details.imap_tls,
      smtpHost: details.smtp_host,
      smtpPort: String(details.smtp_port),
      smtpTls: details.smtp_tls,
      aliases: details.aliases.join(", "),
    });
  }, [accountId, editingAccount]);

  const saveAccountChanges = useCallback(async () => {
    if (!editingAccount) return;
    setSaving(true);
    try {
      await updateAccount({
        accountId: editingAccount.id,
        displayName: accountForm.displayName || undefined,
        password: accountForm.password || undefined,
        imapHost: accountForm.imapHost,
        imapPort: parseInt(accountForm.imapPort) || undefined,
        imapTls: accountForm.imapTls,
        smtpHost: accountForm.smtpHost,
        smtpPort: parseInt(accountForm.smtpPort) || undefined,
        smtpTls: accountForm.smtpTls,
        aliases: accountForm.aliases || undefined,
      });
      setEditingAccount(null);
    } finally {
      setSaving(false);
    }
  }, [editingAccount, accountForm]);

  const settingsSections = [
    {
      section: "Appearance", items: [
        { label: "Show status toaster", desc: "Show sync status at the bottom", key: SETTING_KEYS.showToaster },
      ]
    },
    {
      section: "Privacy", items: [
        { label: "Read-only mode", desc: "Prevent Eddie from modifying your mailbox", key: SETTING_KEYS.readOnly },
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
          <div
            className="rounded-2xl bg-bg-secondary border border-divider overflow-hidden cursor-pointer"
            onClick={!editingAccount ? openAccountEdit : undefined}
          >
            <div className="p-4 flex items-center gap-3.5">
              <Avatar name={email || "E"} email={email || undefined} size={13} fontSize="text-[22px]" className="shrink-0" />
              <div className="flex-1 min-w-0">
                <div className="text-[16px] font-bold text-text-primary">{editingAccount?.display_name || "Personal"}</div>
                <div className="text-[13px] text-text-muted">{email}</div>
                <div className="flex items-center gap-1 mt-0.5">
                  <div className="w-[5px] h-[5px] rounded-full bg-accent-green" />
                  <span className="text-[11px] text-accent-green font-semibold">IMAP connected</span>
                </div>
              </div>
              <span className={`text-[18px] text-text-dim transition-transform ${editingAccount ? "rotate-180" : ""}`}>&#9662;</span>
            </div>

            {editingAccount && (
              <div className="px-4 pb-4 border-t border-divider pt-3 flex flex-col gap-2.5" onClick={(e) => e.stopPropagation()}>
                <AccountField label="Display name" value={accountForm.displayName} placeholder="Your name"
                  onChange={(v) => setAccountForm((f) => ({ ...f, displayName: v }))} />
                <AccountField label="Password" value={accountForm.password} placeholder="••••••••" type="password"
                  onChange={(v) => setAccountForm((f) => ({ ...f, password: v }))} />

                <div className="text-[11px] font-bold text-text-dim tracking-[0.08em] mt-2">IMAP</div>
                <div className="flex gap-2">
                  <AccountField label="Host" value={accountForm.imapHost} className="flex-1"
                    onChange={(v) => setAccountForm((f) => ({ ...f, imapHost: v }))} />
                  <AccountField label="Port" value={accountForm.imapPort} className="w-20"
                    onChange={(v) => setAccountForm((f) => ({ ...f, imapPort: v }))} />
                </div>
                <label className="flex items-center gap-2 text-[13px] text-text-muted cursor-pointer">
                  <input type="checkbox" checked={accountForm.imapTls} onChange={(e) => setAccountForm((f) => ({ ...f, imapTls: e.target.checked }))}
                    className="accent-accent-green" />
                  Use TLS
                </label>

                <div className="text-[11px] font-bold text-text-dim tracking-[0.08em] mt-2">SMTP</div>
                <div className="flex gap-2">
                  <AccountField label="Host" value={accountForm.smtpHost} className="flex-1"
                    onChange={(v) => setAccountForm((f) => ({ ...f, smtpHost: v }))} />
                  <AccountField label="Port" value={accountForm.smtpPort} className="w-20"
                    onChange={(v) => setAccountForm((f) => ({ ...f, smtpPort: v }))} />
                </div>
                <label className="flex items-center gap-2 text-[13px] text-text-muted cursor-pointer">
                  <input type="checkbox" checked={accountForm.smtpTls} onChange={(e) => setAccountForm((f) => ({ ...f, smtpTls: e.target.checked }))}
                    className="accent-accent-green" />
                  Use TLS
                </label>

                <div className="text-[11px] font-bold text-text-dim tracking-[0.08em] mt-2">ALIASES</div>
                <AccountField label="Comma-separated emails" value={accountForm.aliases} placeholder="alt@example.com, other@example.com"
                  onChange={(v) => setAccountForm((f) => ({ ...f, aliases: v }))} />

                <div className="flex gap-2 mt-2">
                  <button onClick={() => setEditingAccount(null)}
                    className="flex-1 py-2.5 rounded-xl border border-divider bg-bg-tertiary text-[13px] font-semibold text-text-muted cursor-pointer">
                    Cancel
                  </button>
                  <button onClick={saveAccountChanges} disabled={saving}
                    className="flex-1 py-2.5 rounded-xl border-none bg-accent-green text-white text-[13px] font-semibold cursor-pointer disabled:opacity-50">
                    {saving ? "Saving..." : "Save"}
                  </button>
                </div>
              </div>
            )}
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

function AccountField({ label, value, onChange, placeholder, type = "text", className = "" }: {
  label: string; value: string; onChange: (v: string) => void;
  placeholder?: string; type?: string; className?: string;
}) {
  return (
    <div className={className}>
      <input
        type={type}
        className="w-full px-3 py-2 rounded-lg border border-divider bg-bg-tertiary text-[14px] text-text-primary font-(--font-body) outline-none transition-colors focus:border-accent-green placeholder:text-text-dim"
        placeholder={placeholder ?? label}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
