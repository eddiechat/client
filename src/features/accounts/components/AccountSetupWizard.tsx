import { useState, useEffect } from "react";
import {
  discoverEmailConfig,
  saveDiscoveredAccount,
} from "../../../tauri";
import type { DiscoveryResult } from "../../../tauri";

type SetupStep = "email" | "discovering" | "auth" | "manual" | "saving";

interface AccountSetupWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

export function AccountSetupWizard({
  isOpen,
  onClose,
  onSuccess,
}: AccountSetupWizardProps) {
  const [step, setStep] = useState<SetupStep>("email");
  const [error, setError] = useState<string | null>(null);
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [discovery, setDiscovery] = useState<DiscoveryResult | null>(null);
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState(993);
  const [imapTls, setImapTls] = useState(true);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(587);
  const [smtpTls, setSmtpTls] = useState(true);
  const [password, setPassword] = useState("");
  const [processing, setProcessing] = useState(false);

  // Reset form when modal closes
  useEffect(() => {
    if (!isOpen) {
      setStep("email");
      setError(null);
      setEmail("");
      setDisplayName("");
      setDiscovery(null);
      setImapHost("");
      setImapPort(993);
      setImapTls(true);
      setSmtpHost("");
      setSmtpPort(587);
      setSmtpTls(true);
      setPassword("");
      setProcessing(false);
    }
  }, [isOpen]);

  const handleEmailSubmit = async () => {
    if (!email.trim()) {
      setError("Please enter your email address");
      return;
    }
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(email)) {
      setError("Please enter a valid email address");
      return;
    }
    setError(null);
    setStep("discovering");
    setProcessing(true);
    try {
      const result = await discoverEmailConfig(email);
      setDiscovery(result);
      setImapHost(result.imap_host);
      setImapPort(result.imap_port);
      setImapTls(result.imap_tls);
      setSmtpHost(result.smtp_host);
      setSmtpPort(result.smtp_port);
      setSmtpTls(result.smtp_tls);
      setStep("auth");
    } catch {
      setStep("manual");
    } finally {
      setProcessing(false);
    }
  };

  const handleSave = async () => {
    setError(null);
    setStep("saving");
    setProcessing(true);
    try {
      const accountName = email.split("@")[0] || email;
      const config = discovery || {
        imap_host: imapHost,
        imap_port: imapPort,
        imap_tls: imapTls,
        smtp_host: smtpHost,
        smtp_port: smtpPort,
        smtp_tls: smtpTls,
        auth_method: "password",
      };
      await saveDiscoveredAccount({
        name: accountName,
        email,
        displayName: displayName || undefined,
        imapHost: config.imap_host,
        imapPort: config.imap_port,
        imapTls: config.imap_tls,
        smtpHost: config.smtp_host,
        smtpPort: config.smtp_port,
        smtpTls: config.smtp_tls,
        authMethod: config.auth_method,
        password: password || undefined,
      });
      onSuccess();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStep("auth");
    } finally {
      setProcessing(false);
    }
  };

  const canProceed = () => {
    switch (step) {
      case "email":
        return email.trim().length > 0;
      case "auth":
        return password.length > 0;
      case "manual":
        return imapHost && smtpHost && password;
      default:
        return false;
    }
  };

  if (!isOpen) return null;

  const inputClass =
    "w-full px-3.5 py-2.5 bg-bg-tertiary border border-divider rounded-lg text-text-primary text-[15px] outline-none focus:border-accent-blue transition-colors placeholder:text-text-muted";
  const btnPrimary =
    "px-5 py-2.5 rounded-lg text-sm font-medium bg-bubble-sent text-white hover:brightness-110 transition-all disabled:opacity-50 disabled:cursor-not-allowed";
  const btnSecondary =
    "px-4 py-2.5 rounded-lg text-sm font-medium bg-bg-tertiary text-text-primary hover:bg-bg-hover transition-colors disabled:opacity-50";

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4 safe-y">
      <div className="w-full max-w-md bg-bg-secondary rounded-2xl flex flex-col max-h-[90vh] overflow-hidden shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-divider">
          <h2 className="text-lg font-semibold text-text-primary">
            Add Email Account
          </h2>
          <button
            className="w-8 h-8 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors text-xl text-text-muted"
            onClick={onClose}
            disabled={processing}
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

          {step === "email" && (
            <>
              <div className="text-center mb-2">
                <h3 className="text-base font-semibold text-text-primary">
                  Add Email Account
                </h3>
                <p className="text-sm text-text-muted mt-1">
                  Enter your email address to get started
                </p>
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
                  autoFocus
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleEmailSubmit()}
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
            </>
          )}

          {step === "discovering" && (
            <div className="text-center py-8">
              <h3 className="text-base font-semibold text-text-primary mb-2">
                Detecting Email Settings
              </h3>
              <p className="text-sm text-text-muted mb-4">
                Please wait while we find the best configuration for your
                email...
              </p>
              <div className="spinner mx-auto" />
            </div>
          )}

          {step === "auth" && discovery && (
            <>
              <div className="text-center mb-2">
                <h3 className="text-base font-semibold text-text-primary">
                  {discovery.provider
                    ? `Sign in to ${discovery.provider}`
                    : "Authentication"}
                </h3>
                {discovery.provider && (
                  <p className="text-sm text-accent-green mt-1">
                    Detected: {discovery.provider}
                  </p>
                )}
              </div>

              {discovery.auth_method === "app_password" ||
              discovery.requires_app_password ? (
                <div className="flex flex-col gap-3">
                  <p className="text-sm text-text-muted">
                    {discovery.provider || "This provider"} requires an
                    app-specific password for third-party app access.
                  </p>
                  {discovery.provider === "iCloud" && (
                    <a
                      href="https://appleid.apple.com/account/manage"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm text-accent-blue hover:underline"
                    >
                      Generate an app-specific password at appleid.apple.com
                    </a>
                  )}
                  {discovery.provider === "Gmail" && (
                    <a
                      href="https://myaccount.google.com/apppasswords"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm text-accent-blue hover:underline"
                    >
                      Generate an app password at myaccount.google.com
                    </a>
                  )}
                  {discovery.provider === "Yahoo Mail" && (
                    <a
                      href="https://login.yahoo.com/account/security"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm text-accent-blue hover:underline"
                    >
                      Generate an app password at login.yahoo.com
                    </a>
                  )}
                  <div className="flex flex-col gap-1.5">
                    <label
                      htmlFor="appPassword"
                      className="text-sm font-medium text-text-muted"
                    >
                      App-Specific Password:
                    </label>
                    <input
                      id="appPassword"
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      placeholder="xxxx-xxxx-xxxx-xxxx"
                      className={inputClass}
                    />
                  </div>
                </div>
              ) : (
                <div className="flex flex-col gap-1.5">
                  <label
                    htmlFor="password"
                    className="text-sm font-medium text-text-muted"
                  >
                    Password:
                  </label>
                  <input
                    id="password"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    className={inputClass}
                  />
                </div>
              )}
              <button
                type="button"
                className="text-sm text-text-muted hover:text-accent-blue transition-colors text-left"
                onClick={() => setStep("manual")}
              >
                Configure manually instead
              </button>
            </>
          )}

          {step === "manual" && (
            <>
              <div className="text-center mb-2">
                <h3 className="text-base font-semibold text-text-primary">
                  Manual Configuration
                </h3>
                <p className="text-sm text-text-muted mt-1">
                  Enter your email server settings
                </p>
                {discovery && (
                  <button
                    type="button"
                    className="text-sm text-accent-blue hover:underline mt-1"
                    onClick={() => setStep("auth")}
                  >
                    ← Back to auto-detected settings
                  </button>
                )}
              </div>

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
                    onChange={(e) =>
                      setImapPort(parseInt(e.target.value) || 993)
                    }
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
                    onChange={(e) =>
                      setSmtpPort(parseInt(e.target.value) || 587)
                    }
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
              </fieldset>

              <fieldset className="border border-divider rounded-lg p-4 flex flex-col gap-3">
                <legend className="px-2 text-sm font-medium text-text-muted">
                  Authentication
                </legend>
                <input
                  id="manualPassword"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Password"
                  className={inputClass}
                />
              </fieldset>
            </>
          )}

          {step === "saving" && (
            <div className="text-center py-8">
              <h3 className="text-base font-semibold text-text-primary mb-2">
                Setting Up Account
              </h3>
              <p className="text-sm text-text-muted mb-4">
                Please wait while we configure your account...
              </p>
              <div className="spinner mx-auto" />
            </div>
          )}
        </div>

        {/* Footer */}
        {step !== "discovering" && step !== "saving" && (
          <div className="flex items-center justify-between px-5 py-4 border-t border-divider">
            <div className="flex gap-2">
              {(step === "auth" || step === "manual") && (
                <button
                  type="button"
                  onClick={() =>
                    setStep(step === "manual" && discovery ? "auth" : "email")
                  }
                  disabled={processing}
                  className={btnSecondary}
                >
                  ← Back
                </button>
              )}
            </div>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={onClose}
                disabled={processing}
                className={btnSecondary}
              >
                Cancel
              </button>
              {step === "email" && (
                <button
                  type="button"
                  onClick={handleEmailSubmit}
                  disabled={!canProceed() || processing}
                  className={btnPrimary}
                >
                  Continue
                </button>
              )}
              {(step === "auth" || step === "manual") && (
                <button
                  type="button"
                  onClick={handleSave}
                  disabled={!canProceed() || processing}
                  className={btnPrimary}
                >
                  {processing ? "Saving..." : "Add Account"}
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
