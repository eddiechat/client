import { useState, useEffect, useRef } from "react";
import type { SaveEmailAccountRequest } from "../../../tauri";

export interface AccountEditData {
  name: string;
  email: string;
  display_name?: string;
  aliases?: string;
  imap_host: string;
  imap_port: number;
  imap_tls: boolean;
  imap_tls_cert?: string;
  smtp_host: string;
  smtp_port: number;
  smtp_tls: boolean;
  smtp_tls_cert?: string;
  username: string;
}

interface AccountConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (data: SaveEmailAccountRequest) => Promise<void>;
  onDelete?: (accountName: string) => Promise<void>;
  editData?: AccountEditData | null;
}

export function AccountConfigModal({
  isOpen,
  onClose,
  onSave,
  onDelete,
  editData,
}: AccountConfigModalProps) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [aliases, setAliases] = useState("");
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState(993);
  const [imapTls, setImapTls] = useState(true);
  const [imapTlsCert, setImapTlsCert] = useState("");
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(465);
  const [smtpTls, setSmtpTls] = useState(true);
  const [smtpTlsCert, setSmtpTlsCert] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isEditMode = !!editData;
  const initializedForRef = useRef<string | null>(null);

  // Initialize form when opened
  useEffect(() => {
    const editKey = editData?.name ?? null;
    if (isOpen && initializedForRef.current !== editKey) {
      initializedForRef.current = editKey;
      if (editData) {
        setName(editData.name);
        setEmail(editData.email);
        setDisplayName(editData.display_name || "");
        setAliases(editData.aliases || "");
        setImapHost(editData.imap_host);
        setImapPort(editData.imap_port);
        setImapTls(editData.imap_tls);
        setImapTlsCert(editData.imap_tls_cert || "");
        setSmtpHost(editData.smtp_host);
        setSmtpPort(editData.smtp_port);
        setSmtpTls(editData.smtp_tls);
        setSmtpTlsCert(editData.smtp_tls_cert || "");
        setUsername(editData.username);
        setPassword("");
      } else {
        resetForm();
      }
      setError(null);
    }
    if (!isOpen) {
      initializedForRef.current = null;
      setConfirmDelete(false);
    }
  }, [isOpen, editData]);

  // Handle escape key
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && isOpen && !saving && !deleting) onClose();
    };
    if (isOpen) {
      document.addEventListener("keydown", handleEscape);
      return () => document.removeEventListener("keydown", handleEscape);
    }
  }, [isOpen, saving, deleting, onClose]);

  const resetForm = () => {
    setName("");
    setEmail("");
    setDisplayName("");
    setAliases("");
    setImapHost("");
    setImapPort(993);
    setImapTls(true);
    setImapTlsCert("");
    setSmtpHost("");
    setSmtpPort(465);
    setSmtpTls(true);
    setSmtpTlsCert("");
    setUsername("");
    setPassword("");
  };

  const handleSave = async () => {
    setError(null);
    if (!name.trim()) {
      setError("Account name is required");
      return;
    }
    if (!email.trim()) {
      setError("Email address is required");
      return;
    }
    if (!imapHost.trim()) {
      setError("IMAP host is required");
      return;
    }
    if (!smtpHost.trim()) {
      setError("SMTP host is required");
      return;
    }
    if (!username.trim()) {
      setError("Username is required");
      return;
    }
    if (!password && !isEditMode) {
      setError("Password is required");
      return;
    }

    setSaving(true);
    try {
      await onSave({
        name: name.trim(),
        email: email.trim(),
        display_name: displayName.trim() || undefined,
        aliases: aliases.trim() || undefined,
        imap_host: imapHost.trim(),
        imap_port: imapPort,
        imap_tls: imapTls,
        imap_tls_cert: imapTlsCert.trim() || undefined,
        smtp_host: smtpHost.trim(),
        smtp_port: smtpPort,
        smtp_tls: smtpTls,
        smtp_tls_cert: smtpTlsCert.trim() || undefined,
        username: username.trim(),
        password: password || undefined,
      });
      onClose();
    } catch (err) {
      // Handle Tauri errors which are objects with a message property
      if (typeof err === 'object' && err !== null && 'message' in err) {
        setError(String(err.message));
      } else if (err instanceof Error) {
        setError(err.message);
      } else {
        setError(String(err));
      }
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteConfirm = async () => {
    if (!onDelete || !editData) return;
    setDeleting(true);
    try {
      await onDelete(editData.name);
      onClose();
    } catch (err) {
      // Handle Tauri errors which are objects with a message property
      if (typeof err === 'object' && err !== null && 'message' in err) {
        setError(String(err.message));
      } else if (err instanceof Error) {
        setError(err.message);
      } else {
        setError(String(err));
      }
    } finally {
      setDeleting(false);
      setConfirmDelete(false);
    }
  };

  const handleClearForm = () => {
    if (isEditMode && editData) {
      setName(editData.name);
      setEmail(editData.email);
      setDisplayName(editData.display_name || "");
      setImapHost(editData.imap_host);
      setImapPort(editData.imap_port);
      setImapTls(editData.imap_tls);
      setImapTlsCert(editData.imap_tls_cert || "");
      setSmtpHost(editData.smtp_host);
      setSmtpPort(editData.smtp_port);
      setSmtpTls(editData.smtp_tls);
      setSmtpTlsCert(editData.smtp_tls_cert || "");
      setUsername(editData.username);
      setPassword("");
    } else {
      resetForm();
    }
    setError(null);
  };

  const isProcessing = saving || deleting;

  if (!isOpen) return null;

  const inputClass =
    "w-full px-3.5 py-2.5 bg-bg-tertiary border border-divider rounded-lg text-text-primary text-[15px] outline-none focus:border-accent-blue transition-colors placeholder:text-text-muted disabled:opacity-50";
  const btnPrimary =
    "px-5 py-2.5 rounded-lg text-sm font-medium bg-bubble-sent text-white hover:brightness-110 transition-all disabled:opacity-50";
  const btnSecondary =
    "px-4 py-2.5 rounded-lg text-sm font-medium bg-bg-tertiary text-text-primary hover:bg-bg-hover transition-colors disabled:opacity-50";
  const btnDanger =
    "px-4 py-2.5 rounded-lg text-sm font-medium bg-accent-red/15 border border-accent-red/30 text-accent-red hover:bg-accent-red/25 transition-colors disabled:opacity-50";

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4 safe-y">
      <div className="w-full max-w-lg bg-bg-secondary rounded-2xl flex flex-col max-h-[90vh] overflow-hidden shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-divider">
          <h2 className="text-lg font-semibold text-text-primary">
            {isEditMode ? "Edit Account" : "Configure Email Account"}
          </h2>
          <button
            className="w-8 h-8 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors text-xl text-text-muted"
            onClick={onClose}
            disabled={isProcessing}
          >
            x
          </button>
        </div>

        {/* Form */}
        <div className="flex-1 overflow-y-auto p-5 flex flex-col gap-4">
          {error && (
            <div className="px-4 py-3 bg-accent-red/15 border border-accent-red/30 rounded-lg text-sm text-accent-red">
              {error}
            </div>
          )}

          <fieldset className="border border-divider rounded-lg p-4 flex flex-col gap-3">
            <legend className="px-2 text-sm font-medium text-text-muted">
              Account Information
            </legend>
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="name"
                className="text-sm font-medium text-text-muted"
              >
                Account Name:
              </label>
              <input
                id="name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g., Personal, Work"
                disabled={isEditMode}
                className={inputClass}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="email"
                className="text-sm font-medium text-text-muted"
              >
                Email Address:
              </label>
              <input
                id="email"
                type="email"
                inputMode="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                className={inputClass}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="displayName"
                className="text-sm font-medium text-text-muted"
              >
                Display Name (optional):
              </label>
              <input
                id="displayName"
                type="text"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                placeholder="Your Name"
                className={inputClass}
              />
            </div>

            <div>
              <label
                htmlFor="aliases"
                className="text-sm font-medium text-text-muted"
              >
                Aliases (optional):
              </label>
              <input
                id="aliases"
                type="text"
                value={aliases}
                onChange={(e) => setAliases(e.target.value)}
                placeholder="alias1@example.com, alias2@example.com"
                className={inputClass}
              />
              <p className="text-xs text-text-muted mt-1">
                Comma-separated list of email aliases for this account
              </p>
            </div>
          </fieldset>

          <fieldset className="border border-divider rounded-lg p-4 flex flex-col gap-3">
            <legend className="px-2 text-sm font-medium text-text-muted">
              IMAP (Receiving)
            </legend>
            <input
              type="text"
              value={imapHost}
              onChange={(e) => setImapHost(e.target.value)}
              placeholder="imap.example.com"
              className={inputClass}
            />
            <div className="flex gap-3 items-center">
              <input
                type="number"
                inputMode="numeric"
                value={imapPort}
                onChange={(e) => setImapPort(parseInt(e.target.value) || 993)}
                className={`${inputClass} w-24`}
              />
              <label className="flex items-center gap-2 text-sm text-text-muted">
                <input
                  type="checkbox"
                  checked={imapTls}
                  onChange={(e) => setImapTls(e.target.checked)}
                  className="w-4 h-4 rounded"
                />
                Use TLS
              </label>
            </div>
            <input
              type="text"
              value={imapTlsCert}
              onChange={(e) => setImapTlsCert(e.target.value)}
              placeholder="TLS Certificate path (optional)"
              className={inputClass}
            />
          </fieldset>

          <fieldset className="border border-divider rounded-lg p-4 flex flex-col gap-3">
            <legend className="px-2 text-sm font-medium text-text-muted">
              SMTP (Sending)
            </legend>
            <input
              type="text"
              value={smtpHost}
              onChange={(e) => setSmtpHost(e.target.value)}
              placeholder="smtp.example.com"
              className={inputClass}
            />
            <div className="flex gap-3 items-center">
              <input
                type="number"
                inputMode="numeric"
                value={smtpPort}
                onChange={(e) => setSmtpPort(parseInt(e.target.value) || 465)}
                className={`${inputClass} w-24`}
              />
              <label className="flex items-center gap-2 text-sm text-text-muted">
                <input
                  type="checkbox"
                  checked={smtpTls}
                  onChange={(e) => setSmtpTls(e.target.checked)}
                  className="w-4 h-4 rounded"
                />
                Use TLS
              </label>
            </div>
            <input
              type="text"
              value={smtpTlsCert}
              onChange={(e) => setSmtpTlsCert(e.target.value)}
              placeholder="TLS Certificate path (optional)"
              className={inputClass}
            />
          </fieldset>

          <fieldset className="border border-divider rounded-lg p-4 flex flex-col gap-3">
            <legend className="px-2 text-sm font-medium text-text-muted">
              Authentication
            </legend>
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="username"
                className="text-sm font-medium text-text-muted"
              >
                Username:
              </label>
              <input
                id="username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Usually your email address"
                className={inputClass}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="password"
                className="text-sm font-medium text-text-muted"
              >
                Password{isEditMode ? " (leave blank to keep current)" : ""}:
              </label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className={inputClass}
              />
            </div>
          </fieldset>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-4 border-t border-divider">
          <div className="flex gap-2 items-center">
            {isEditMode && onDelete && !confirmDelete && (
              <button
                type="button"
                onClick={() => setConfirmDelete(true)}
                disabled={isProcessing}
                className={btnDanger}
              >
                Delete Account
              </button>
            )}
            {isEditMode && onDelete && confirmDelete && (
              <>
                <span className="text-sm text-accent-red mr-2">
                  Delete this account?
                </span>
                <button
                  type="button"
                  onClick={handleDeleteConfirm}
                  disabled={deleting}
                  className={btnDanger}
                >
                  {deleting ? "Deleting..." : "Yes, Delete"}
                </button>
                <button
                  type="button"
                  onClick={() => setConfirmDelete(false)}
                  disabled={deleting}
                  className={btnSecondary}
                >
                  No
                </button>
              </>
            )}
            {!confirmDelete && (
              <button
                type="button"
                onClick={handleClearForm}
                disabled={isProcessing}
                className={btnSecondary}
                title="Clear all form fields"
              >
                Clear Form
              </button>
            )}
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onClose}
              disabled={isProcessing}
              className={btnSecondary}
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSave}
              disabled={isProcessing}
              className={btnPrimary}
            >
              {saving ? "Saving..." : "Save Account"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
