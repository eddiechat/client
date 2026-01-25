import { useState, useEffect, useRef } from "react";
import type { SaveAccountRequest } from "../types";

export interface AccountEditData {
  name: string;
  email: string;
  display_name?: string;
  imap_host: string;
  imap_port: number;
  imap_tls: boolean;
  imap_tls_cert?: string;
  smtp_host: string;
  smtp_port: number;
  smtp_tls: boolean;
  smtp_tls_cert?: string;
  username: string;
  // CardDAV settings
  carddav_url?: string;
  carddav_tls?: boolean;
  carddav_tls_cert?: string;
  carddav_username?: string;
}

interface AccountConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (data: SaveAccountRequest) => Promise<void>;
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
  // CardDAV settings
  const [enableCardDAV, setEnableCardDAV] = useState(false);
  const [carddavUrl, setCarddavUrl] = useState("");
  const [carddavTls, setCarddavTls] = useState(true);
  const [carddavTlsCert, setCarddavTlsCert] = useState("");
  const [carddavUsername, setCarddavUsername] = useState("");
  const [carddavPassword, setCarddavPassword] = useState("");
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isEditMode = !!editData;

  // Track which account we've initialized to avoid resetting on every render
  const initializedForRef = useRef<string | null>(null);

  // Reset form when modal opens with different data
  useEffect(() => {
    const editKey = editData?.name ?? null;

    // Only initialize if modal just opened or we're editing a different account
    if (isOpen && initializedForRef.current !== editKey) {
      initializedForRef.current = editKey;

      if (editData) {
        // Populate form with existing data
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
        setPassword(""); // Don't pre-fill password for security
        // CardDAV settings
        setEnableCardDAV(!!editData.carddav_url);
        setCarddavUrl(editData.carddav_url || "");
        setCarddavTls(editData.carddav_tls ?? true);
        setCarddavTlsCert(editData.carddav_tls_cert || "");
        setCarddavUsername(editData.carddav_username || "");
        setCarddavPassword(""); // Don't pre-fill password for security
      } else {
        // Reset to defaults for new account
        setName("");
        setEmail("");
        setDisplayName("");
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
        // CardDAV defaults
        setEnableCardDAV(false);
        setCarddavUrl("");
        setCarddavTls(true);
        setCarddavTlsCert("");
        setCarddavUsername("");
        setCarddavPassword("");
      }
      setError(null);
    }

    // Reset state when modal closes
    if (!isOpen) {
      initializedForRef.current = null;
      setConfirmDelete(false);
    }
  }, [isOpen, editData]);

  if (!isOpen) return null;

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
        imap_host: imapHost.trim(),
        imap_port: imapPort,
        imap_tls: imapTls,
        imap_tls_cert: imapTlsCert.trim() || undefined,
        smtp_host: smtpHost.trim(),
        smtp_port: smtpPort,
        smtp_tls: smtpTls,
        smtp_tls_cert: smtpTlsCert.trim() || undefined,
        username: username.trim(),
        password,
        // CardDAV settings (only if enabled)
        carddav_url: enableCardDAV && carddavUrl.trim() ? carddavUrl.trim() : undefined,
        carddav_tls: enableCardDAV ? carddavTls : undefined,
        carddav_tls_cert: enableCardDAV && carddavTlsCert.trim() ? carddavTlsCert.trim() : undefined,
        carddav_username: enableCardDAV && carddavUsername.trim() ? carddavUsername.trim() : undefined,
        carddav_password: enableCardDAV && carddavPassword ? carddavPassword : undefined,
      });
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteClick = () => {
    setConfirmDelete(true);
  };

  const handleDeleteConfirm = async () => {
    if (!onDelete || !editData) return;

    setDeleting(true);
    try {
      await onDelete(editData.name);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(false);
      setConfirmDelete(false);
    }
  };

  const handleDeleteCancel = () => {
    setConfirmDelete(false);
  };

  const isProcessing = saving || deleting;

  return (
    <div className="compose-modal-overlay">
      <div className="compose-modal account-config-modal">
        <div className="compose-header">
          <h2>{isEditMode ? "Edit Account" : "Configure Email Account"}</h2>
          <button className="close-btn" onClick={onClose} disabled={isProcessing}>
            x
          </button>
        </div>

        <div className="compose-form">
          {error && <div className="config-error-message">{error}</div>}

          <fieldset className="config-section">
            <legend>Account Information</legend>
            <div className="form-row">
              <label htmlFor="name">Account Name:</label>
              <input
                id="name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g., Personal, Work"
                disabled={isEditMode}
              />
            </div>
            <div className="form-row">
              <label htmlFor="email">Email Address:</label>
              <input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
              />
            </div>
            <div className="form-row">
              <label htmlFor="displayName">Display Name (optional):</label>
              <input
                id="displayName"
                type="text"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                placeholder="Your Name"
              />
            </div>
          </fieldset>

          <fieldset className="config-section">
            <legend>IMAP (Receiving)</legend>
            <div className="form-row">
              <label htmlFor="imapHost">Server:</label>
              <input
                id="imapHost"
                type="text"
                value={imapHost}
                onChange={(e) => setImapHost(e.target.value)}
                placeholder="imap.example.com"
              />
            </div>
            <div className="form-row-inline">
              <div className="form-row">
                <label htmlFor="imapPort">Port:</label>
                <input
                  id="imapPort"
                  type="number"
                  value={imapPort}
                  onChange={(e) => setImapPort(parseInt(e.target.value) || 993)}
                />
              </div>
              <div className="form-row checkbox-row">
                <label>
                  <input
                    type="checkbox"
                    checked={imapTls}
                    onChange={(e) => setImapTls(e.target.checked)}
                  />
                  Use TLS
                </label>
              </div>
            </div>
            <div className="form-row">
              <label htmlFor="imapTlsCert">TLS Certificate (optional):</label>
              <input
                id="imapTlsCert"
                type="text"
                value={imapTlsCert}
                onChange={(e) => setImapTlsCert(e.target.value)}
                placeholder="Path to certificate file (for self-signed certs)"
              />
            </div>
          </fieldset>

          <fieldset className="config-section">
            <legend>SMTP (Sending)</legend>
            <div className="form-row">
              <label htmlFor="smtpHost">Server:</label>
              <input
                id="smtpHost"
                type="text"
                value={smtpHost}
                onChange={(e) => setSmtpHost(e.target.value)}
                placeholder="smtp.example.com"
              />
            </div>
            <div className="form-row-inline">
              <div className="form-row">
                <label htmlFor="smtpPort">Port:</label>
                <input
                  id="smtpPort"
                  type="number"
                  value={smtpPort}
                  onChange={(e) => setSmtpPort(parseInt(e.target.value) || 465)}
                />
              </div>
              <div className="form-row checkbox-row">
                <label>
                  <input
                    type="checkbox"
                    checked={smtpTls}
                    onChange={(e) => setSmtpTls(e.target.checked)}
                  />
                  Use TLS
                </label>
              </div>
            </div>
            <div className="form-row">
              <label htmlFor="smtpTlsCert">TLS Certificate (optional):</label>
              <input
                id="smtpTlsCert"
                type="text"
                value={smtpTlsCert}
                onChange={(e) => setSmtpTlsCert(e.target.value)}
                placeholder="Path to certificate file (for self-signed certs)"
              />
            </div>
          </fieldset>

          <fieldset className="config-section">
            <legend>Authentication</legend>
            <div className="form-row">
              <label htmlFor="username">Username:</label>
              <input
                id="username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Usually your email address"
              />
            </div>
            <div className="form-row">
              <label htmlFor="password">
                Password{isEditMode ? " (leave blank to keep current)" : ""}:
              </label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
          </fieldset>

          <fieldset className="config-section">
            <legend>CardDAV (Contacts)</legend>
            <div className="form-row checkbox-row">
              <label>
                <input
                  type="checkbox"
                  checked={enableCardDAV}
                  onChange={(e) => setEnableCardDAV(e.target.checked)}
                />
                Enable CardDAV for contacts sync
              </label>
            </div>
            {enableCardDAV && (
              <>
                <div className="form-row">
                  <label htmlFor="carddavUrl">CardDAV URL:</label>
                  <input
                    id="carddavUrl"
                    type="text"
                    value={carddavUrl}
                    onChange={(e) => setCarddavUrl(e.target.value)}
                    placeholder="https://contacts.example.com/dav/"
                  />
                </div>
                <div className="form-row-inline">
                  <div className="form-row checkbox-row">
                    <label>
                      <input
                        type="checkbox"
                        checked={carddavTls}
                        onChange={(e) => setCarddavTls(e.target.checked)}
                      />
                      Use TLS
                    </label>
                  </div>
                </div>
                <div className="form-row">
                  <label htmlFor="carddavTlsCert">TLS Certificate (optional):</label>
                  <input
                    id="carddavTlsCert"
                    type="text"
                    value={carddavTlsCert}
                    onChange={(e) => setCarddavTlsCert(e.target.value)}
                    placeholder="Path to certificate file"
                  />
                </div>
                <div className="form-row">
                  <label htmlFor="carddavUsername">
                    Username (leave blank to use IMAP username):
                  </label>
                  <input
                    id="carddavUsername"
                    type="text"
                    value={carddavUsername}
                    onChange={(e) => setCarddavUsername(e.target.value)}
                    placeholder="Optional: separate CardDAV username"
                  />
                </div>
                <div className="form-row">
                  <label htmlFor="carddavPassword">
                    Password (leave blank to use IMAP password):
                  </label>
                  <input
                    id="carddavPassword"
                    type="password"
                    value={carddavPassword}
                    onChange={(e) => setCarddavPassword(e.target.value)}
                    placeholder="Optional: separate CardDAV password"
                  />
                </div>
              </>
            )}
          </fieldset>
        </div>

        <div className="compose-footer">
          {isEditMode && onDelete && !confirmDelete && (
            <button
              type="button"
              onClick={handleDeleteClick}
              disabled={isProcessing}
              className="delete-btn"
            >
              Delete Account
            </button>
          )}
          {isEditMode && onDelete && confirmDelete && (
            <>
              <span className="delete-confirm-text">Delete this account?</span>
              <button
                type="button"
                onClick={handleDeleteConfirm}
                disabled={deleting}
                className="delete-btn"
              >
                {deleting ? "Deleting..." : "Yes, Delete"}
              </button>
              <button
                type="button"
                onClick={handleDeleteCancel}
                disabled={deleting}
                className="cancel-btn"
              >
                No
              </button>
            </>
          )}
          <div className="footer-spacer" />
          <button type="button" onClick={onClose} disabled={isProcessing} className="cancel-btn">
            Cancel
          </button>
          <button type="button" onClick={handleSave} disabled={isProcessing} className="send-btn">
            {saving ? "Saving..." : "Save Account"}
          </button>
        </div>
      </div>
    </div>
  );
}
