import { useState, useEffect, useCallback } from "react";
import { open } from "@tauri-apps/plugin-opener";
import type { DiscoveryResult } from "../types";
import {
  discoverEmailConfig,
  startOAuthFlow,
  completeOAuthFlow,
  saveDiscoveredAccount,
  checkOAuthStatus,
} from "../lib/api";

type SetupStep = "email" | "discovering" | "auth" | "manual" | "saving";

interface AccountSetupWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

const OAUTH_REDIRECT_URI = "http://localhost:8765/oauth/callback";

export function AccountSetupWizard({
  isOpen,
  onClose,
  onSuccess,
}: AccountSetupWizardProps) {
  // Wizard state
  const [step, setStep] = useState<SetupStep>("email");
  const [error, setError] = useState<string | null>(null);

  // Account data
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [discovery, setDiscovery] = useState<DiscoveryResult | null>(null);

  // Manual configuration
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState(993);
  const [imapTls, setImapTls] = useState(true);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(587);
  const [smtpTls, setSmtpTls] = useState(true);

  // Auth data
  const [password, setPassword] = useState("");
  const [oauthPending, setOauthPending] = useState(false);
  const [oauthComplete, setOauthComplete] = useState(false);

  // Processing state
  const [processing, setProcessing] = useState(false);

  // Reset state when modal opens/closes
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
      setOauthPending(false);
      setOauthComplete(false);
      setProcessing(false);
    }
  }, [isOpen]);

  // Handle email submission and autodiscovery
  const handleEmailSubmit = async () => {
    if (!email.trim()) {
      setError("Please enter your email address");
      return;
    }

    // Basic email validation
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

      // Pre-fill manual fields from discovery
      setImapHost(result.imap_host);
      setImapPort(result.imap_port);
      setImapTls(result.imap_tls);
      setSmtpHost(result.smtp_host);
      setSmtpPort(result.smtp_port);
      setSmtpTls(result.smtp_tls);

      setStep("auth");
    } catch (e) {
      // Discovery failed, show manual config
      console.error("Autodiscovery failed:", e);
      setStep("manual");
    } finally {
      setProcessing(false);
    }
  };

  // Handle OAuth flow
  const handleStartOAuth = async () => {
    if (!discovery?.oauth_provider) {
      setError("OAuth provider not configured");
      return;
    }

    setError(null);
    setProcessing(true);
    setOauthPending(true);

    try {
      const authUrl = await startOAuthFlow(
        discovery.oauth_provider,
        email,
        OAUTH_REDIRECT_URI
      );

      // Open browser for OAuth
      await open(authUrl);

      // Note: In a real implementation, we would need to:
      // 1. Start a local HTTP server to receive the callback
      // 2. Or use deep links to handle the redirect
      // For now, we show a manual code entry fallback
      setError(
        "Please complete the sign-in in your browser. " +
        "Once complete, click 'Check Status' to continue."
      );
    } catch (e) {
      console.error("OAuth flow failed:", e);
      setError(e instanceof Error ? e.message : String(e));
      setOauthPending(false);
    } finally {
      setProcessing(false);
    }
  };

  // Check OAuth status after browser flow
  const handleCheckOAuthStatus = async () => {
    setProcessing(true);
    try {
      const status = await checkOAuthStatus(email);
      if (status.has_tokens) {
        setOauthComplete(true);
        setOauthPending(false);
        setError(null);
      } else {
        setError("OAuth not complete. Please try signing in again.");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setProcessing(false);
    }
  };

  // Handle save
  const handleSave = async () => {
    setError(null);
    setStep("saving");
    setProcessing(true);

    try {
      // Determine account name from email
      const accountName = email.split("@")[0] || email;

      // Use discovery data or manual config
      const config = discovery || {
        imap_host: imapHost,
        imap_port: imapPort,
        imap_tls: imapTls,
        smtp_host: smtpHost,
        smtp_port: smtpPort,
        smtp_tls: smtpTls,
        auth_method: "password",
        oauth_provider: undefined,
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
        oauthProvider: config.oauth_provider,
        password: password || undefined,
      });

      onSuccess();
      onClose();
    } catch (e) {
      console.error("Failed to save account:", e);
      setError(e instanceof Error ? e.message : String(e));
      setStep("auth");
    } finally {
      setProcessing(false);
    }
  };

  // Render step content
  const renderEmailStep = () => (
    <>
      <div className="wizard-step-header">
        <h3>Add Email Account</h3>
        <p>Enter your email address to get started</p>
      </div>
      <div className="form-row">
        <label htmlFor="email">Email Address:</label>
        <input
          id="email"
          type="email"
          inputMode="email"
          autoFocus
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleEmailSubmit()}
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
    </>
  );

  const renderDiscoveringStep = () => (
    <div className="wizard-step-header">
      <h3>Detecting Email Settings</h3>
      <p>Please wait while we find the best configuration for your email...</p>
      <div className="spinner" />
    </div>
  );

  const renderAuthStep = () => {
    if (!discovery) return null;

    const isOAuth = discovery.auth_method === "oauth2";
    const isAppPassword = discovery.auth_method === "app_password" || discovery.requires_app_password;

    return (
      <>
        <div className="wizard-step-header">
          <h3>
            {discovery.provider
              ? `Sign in to ${discovery.provider}`
              : "Authentication"}
          </h3>
          {discovery.provider && (
            <p className="provider-detected">
              Detected: {discovery.provider}
            </p>
          )}
        </div>

        {isOAuth && !oauthComplete ? (
          <div className="oauth-section">
            <p>
              {discovery.provider} requires secure sign-in through your browser.
            </p>
            <button
              className="oauth-btn"
              onClick={handleStartOAuth}
              disabled={processing || oauthPending}
            >
              {processing
                ? "Opening browser..."
                : oauthPending
                ? "Waiting for sign-in..."
                : `Sign in with ${discovery.provider}`}
            </button>
            {oauthPending && (
              <button
                className="check-status-btn"
                onClick={handleCheckOAuthStatus}
                disabled={processing}
              >
                Check Status
              </button>
            )}
          </div>
        ) : isOAuth && oauthComplete ? (
          <div className="oauth-success">
            <p>Successfully signed in with {discovery.provider}!</p>
          </div>
        ) : isAppPassword ? (
          <div className="app-password-section">
            <p>
              {discovery.provider || "This provider"} requires an app-specific
              password. You'll need to generate one in your account settings.
            </p>
            {discovery.provider === "iCloud" && (
              <a
                href="https://appleid.apple.com/account/manage"
                target="_blank"
                rel="noopener noreferrer"
                className="help-link"
              >
                Generate an app-specific password at appleid.apple.com
              </a>
            )}
            <div className="form-row">
              <label htmlFor="appPassword">App-Specific Password:</label>
              <input
                id="appPassword"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="xxxx-xxxx-xxxx-xxxx"
              />
            </div>
          </div>
        ) : (
          <div className="password-section">
            <div className="form-row">
              <label htmlFor="password">Password:</label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
          </div>
        )}

        <button
          type="button"
          className="manual-config-link"
          onClick={() => setStep("manual")}
        >
          Configure manually instead
        </button>
      </>
    );
  };

  const renderManualStep = () => (
    <>
      <div className="wizard-step-header">
        <h3>Manual Configuration</h3>
        <p>Enter your email server settings</p>
      </div>

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
              inputMode="numeric"
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
              inputMode="numeric"
              value={smtpPort}
              onChange={(e) => setSmtpPort(parseInt(e.target.value) || 587)}
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
      </fieldset>

      <fieldset className="config-section">
        <legend>Authentication</legend>
        <div className="form-row">
          <label htmlFor="manualPassword">Password:</label>
          <input
            id="manualPassword"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>
      </fieldset>
    </>
  );

  const renderSavingStep = () => (
    <div className="wizard-step-header">
      <h3>Setting Up Account</h3>
      <p>Please wait while we configure your account...</p>
      <div className="spinner" />
    </div>
  );

  // Determine if we can proceed
  const canProceed = () => {
    switch (step) {
      case "email":
        return email.trim().length > 0;
      case "auth":
        if (!discovery) return false;
        if (discovery.auth_method === "oauth2") return oauthComplete;
        return password.length > 0;
      case "manual":
        return imapHost && smtpHost && password;
      default:
        return false;
    }
  };

  if (!isOpen) return null;

  return (
    <div className="compose-modal-overlay">
      <div className="compose-modal account-setup-wizard">
        <div className="compose-header">
          <h2>Add Email Account</h2>
          <button
            className="close-btn"
            onClick={onClose}
            disabled={processing}
          >
            x
          </button>
        </div>

        <div className="compose-form">
          {error && <div className="config-error-message">{error}</div>}

          {step === "email" && renderEmailStep()}
          {step === "discovering" && renderDiscoveringStep()}
          {step === "auth" && renderAuthStep()}
          {step === "manual" && renderManualStep()}
          {step === "saving" && renderSavingStep()}
        </div>

        <div className="compose-footer">
          {step !== "discovering" && step !== "saving" && (
            <>
              <button
                type="button"
                onClick={onClose}
                disabled={processing}
                className="cancel-btn"
              >
                Cancel
              </button>
              {step === "email" && (
                <button
                  type="button"
                  onClick={handleEmailSubmit}
                  disabled={!canProceed() || processing}
                  className="send-btn"
                >
                  Continue
                </button>
              )}
              {(step === "auth" || step === "manual") && (
                <button
                  type="button"
                  onClick={handleSave}
                  disabled={!canProceed() || processing}
                  className="send-btn"
                >
                  {processing ? "Saving..." : "Add Account"}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
