import { useState, useEffect } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useAuth } from "../shared/context";
import { discoverEmailConfig } from "../tauri";
import type { DiscoveryResult } from "../tauri";

export const Route = createFileRoute("/login")({
  component: LoginScreen,
});

type SetupStep = "email" | "discovering" | "auth" | "manual" | "saving";

function LoginScreen() {
  const navigate = useNavigate();
  const auth = useAuth();

  const [step, setStep] = useState<SetupStep>("email");
  const [error, setError] = useState<string | null>(null);
  const [discovery, setDiscovery] = useState<DiscoveryResult | null>(null);
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState(993);
  const [imapTls, setImapTls] = useState(true);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(587);
  const [smtpTls, setSmtpTls] = useState(true);

  useEffect(() => {
    if (auth.loggedIn) navigate({ to: "/onboarding" });
  }, [auth.loggedIn, navigate]);

  const handleEmailSubmit = async () => {
    const email = auth.email.trim();
    if (!email) {
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
    }
  };

  const handleSave = async () => {
    if (!auth.password.trim()) {
      setError("Please enter your password");
      return;
    }
    setError(null);
    setStep("saving");
    try {
      await auth.handleLogin({
        imapHost: discovery ? discovery.imap_host : imapHost,
        imapPort: discovery ? discovery.imap_port : imapPort,
        imapTls: discovery ? discovery.imap_tls : imapTls,
        smtpHost: discovery ? discovery.smtp_host : smtpHost,
        smtpPort: discovery ? discovery.smtp_port : smtpPort,
      });
      navigate({ to: "/onboarding" });
    } catch (e) {
      setError(typeof e === "string" ? e : String(e));
      setStep(discovery ? "auth" : "manual");
    }
  };

  const inputClass =
    "w-full py-3 px-3.5 border border-divider rounded-[10px] bg-bg-tertiary text-[14px] font-medium text-text-primary outline-none placeholder:text-text-dim focus:border-accent-green";

  return (
    <div className="h-screen overflow-y-auto" style={{ background: "var(--color-bg-gradient)", paddingTop: 'env(safe-area-inset-top, 0px)', paddingBottom: 'env(safe-area-inset-bottom, 0px)' }}>
      <div className="min-h-full flex flex-col items-center justify-center p-10">
      <div className="max-w-[380px] w-full flex flex-col items-center">
        <img src="/eddie-swirl-green.svg" alt="Eddie" className="w-[88px] h-[88px] mb-7" />
        <h1 className="text-[28px] font-black mb-2.5 text-text-primary" style={{ letterSpacing: "-0.5px" }}>
          Welcome to Eddie
        </h1>
        <p className="text-text-muted text-center mb-8 leading-relaxed text-[14px] font-medium whitespace-pre-line">
          {"Messaging built on email.\nNo account needed \u2014 just log in."}
        </p>

        {error && (
          <div className="w-full px-4 py-3 bg-accent-red/15 border border-accent-red/30 rounded-xl text-sm text-accent-red mb-4">
            {error}
          </div>
        )}

        {/* Step 1: Email */}
        {step === "email" && (
          <div className="w-full">
            <div className="bg-bg-secondary border border-divider rounded-2xl px-5 pt-5 pb-6 mb-5">
              <label className="block text-[10px] font-bold tracking-widest text-text-dim mb-2">
                EMAIL ADDRESS
              </label>
              <input
                className={inputClass}
                type="email"
                inputMode="email"
                autoFocus
                placeholder="you@email.com"
                value={auth.email}
                onChange={(e) => auth.setEmail(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleEmailSubmit()}
              />
            </div>
            <button
              type="button"
              className="w-full py-3.5 border-none rounded-[12px] bg-accent-green text-white text-[15px] font-extrabold cursor-pointer hover:brightness-95 disabled:opacity-60 disabled:cursor-not-allowed transition"
              disabled={!auth.email.trim()}
              onClick={handleEmailSubmit}
            >
              Continue
            </button>
          </div>
        )}

        {/* Step 2: Discovering */}
        {step === "discovering" && (
          <div className="w-full text-center py-8">
            <div className="w-8 h-8 border-3 border-accent-green/30 border-t-accent-green rounded-full animate-spin mx-auto mb-4" />
            <h3 className="text-base font-semibold text-text-primary mb-2">
              Detecting Email Settings
            </h3>
            <p className="text-sm text-text-muted">
              Finding the best configuration for your email...
            </p>
          </div>
        )}

        {/* Step 3: Auth (discovery succeeded) */}
        {step === "auth" && discovery && (
          <div className="w-full">
            <div className="bg-bg-secondary border border-divider rounded-2xl px-5 pt-5 pb-6 mb-5">
              {discovery.provider && (
                <div className="text-center mb-4">
                  <p className="text-sm text-accent-green font-medium">
                    Detected: {discovery.provider}
                  </p>
                </div>
              )}

              {discovery.requires_app_password ? (
                <>
                  <p className="text-sm text-text-muted mb-3">
                    {discovery.provider || "This provider"} requires an app-specific password.
                  </p>
                  {discovery.provider === "Gmail" && (
                    <a
                      href="https://myaccount.google.com/apppasswords"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block text-sm text-accent-green hover:underline mb-4"
                    >
                      Generate an app password at myaccount.google.com
                    </a>
                  )}
                  {discovery.provider === "iCloud" && (
                    <a
                      href="https://appleid.apple.com/account/manage"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block text-sm text-accent-green hover:underline mb-4"
                    >
                      Generate an app password at appleid.apple.com
                    </a>
                  )}
                  {discovery.provider === "Yahoo Mail" && (
                    <a
                      href="https://login.yahoo.com/account/security"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block text-sm text-accent-green hover:underline mb-4"
                    >
                      Generate an app password at login.yahoo.com
                    </a>
                  )}
                  <label className="block text-[10px] font-bold tracking-widest text-text-dim mb-2">
                    APP PASSWORD
                  </label>
                  <input
                    className={inputClass}
                    type="password"
                    autoFocus
                    placeholder="xxxx-xxxx-xxxx-xxxx"
                    value={auth.password}
                    onChange={(e) => auth.setPassword(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleSave()}
                  />
                </>
              ) : (
                <>
                  <label className="block text-[10px] font-bold tracking-widest text-text-dim mb-2">
                    PASSWORD
                  </label>
                  <input
                    className={inputClass}
                    type="password"
                    autoFocus
                    placeholder={"\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"}
                    value={auth.password}
                    onChange={(e) => auth.setPassword(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleSave()}
                  />
                </>
              )}

              <label className="block text-[10px] font-bold tracking-widest text-text-dim mb-2 mt-4">
                ALIASES (OPTIONAL)
              </label>
              <input
                className={inputClass}
                type="text"
                placeholder="alt@example.com, other@example.com"
                value={auth.aliases}
                onChange={(e) => auth.setAliases(e.target.value)}
              />
              <p className="text-[11px] text-text-dim mt-1.5">
                Other email addresses that belong to you, comma-separated.
              </p>

              <button
                type="button"
                className="block w-full text-left bg-transparent border-none text-text-dim text-[12px] cursor-pointer pt-3 mt-1"
                onClick={() => setStep("manual")}
              >
                Configure manually instead
              </button>
            </div>

            <div className="flex gap-3">
              <button
                type="button"
                className="flex-1 py-3.5 border border-divider rounded-[12px] bg-bg-secondary text-text-primary text-[14px] font-semibold cursor-pointer hover:bg-bg-tertiary transition"
                onClick={() => {
                  setStep("email");
                  setDiscovery(null);
                }}
              >
                Back
              </button>
              <button
                type="button"
                className="flex-2 py-3.5 border-none rounded-[12px] bg-accent-green text-white text-[15px] font-extrabold cursor-pointer hover:brightness-95 disabled:opacity-60 disabled:cursor-not-allowed transition"
                disabled={!auth.password.trim()}
                onClick={handleSave}
              >
                Log in
              </button>
            </div>
          </div>
        )}

        {/* Step 4: Manual config */}
        {step === "manual" && (
          <div className="w-full">
            <div className="bg-bg-secondary border border-divider rounded-2xl px-5 pt-5 pb-6 mb-5">
              <p className="text-[13px] text-text-muted mb-4 text-center font-medium">
                Enter your email server settings
              </p>
              {discovery && (
                <button
                  type="button"
                  className="block text-[13px] text-accent-green hover:underline mb-4 mx-auto font-semibold"
                  onClick={() => setStep("auth")}
                >
                  {"\u2190"} Back to auto-detected settings
                </button>
              )}

              <fieldset className="border border-divider rounded-[10px] p-4 mb-4">
                <legend className="px-2 text-[10px] font-bold tracking-widest text-text-dim">
                  IMAP (RECEIVING)
                </legend>
                <input
                  className={`${inputClass} mb-3`}
                  type="text"
                  placeholder="imap.example.com"
                  value={imapHost}
                  onChange={(e) => setImapHost(e.target.value)}
                />
                <div className="flex gap-3 items-center">
                  <input
                    className={`${inputClass} w-24`}
                    type="number"
                    inputMode="numeric"
                    value={imapPort}
                    onChange={(e) => setImapPort(parseInt(e.target.value) || 993)}
                  />
                  <label className="flex items-center gap-2 text-sm text-text-muted">
                    <input
                      type="checkbox"
                      checked={imapTls}
                      onChange={(e) => setImapTls(e.target.checked)}
                      className="w-4 h-4 rounded"
                    />
                    TLS
                  </label>
                </div>
              </fieldset>

              <fieldset className="border border-divider rounded-[10px] p-4 mb-4">
                <legend className="px-2 text-[10px] font-bold tracking-widest text-text-dim">
                  SMTP (SENDING)
                </legend>
                <input
                  className={`${inputClass} mb-3`}
                  type="text"
                  placeholder="smtp.example.com"
                  value={smtpHost}
                  onChange={(e) => setSmtpHost(e.target.value)}
                />
                <div className="flex gap-3 items-center">
                  <input
                    className={`${inputClass} w-24`}
                    type="number"
                    inputMode="numeric"
                    value={smtpPort}
                    onChange={(e) => setSmtpPort(parseInt(e.target.value) || 587)}
                  />
                  <label className="flex items-center gap-2 text-sm text-text-muted">
                    <input
                      type="checkbox"
                      checked={smtpTls}
                      onChange={(e) => setSmtpTls(e.target.checked)}
                      className="w-4 h-4 rounded"
                    />
                    TLS
                  </label>
                </div>
              </fieldset>

              <label className="block text-[10px] font-bold tracking-widest text-text-dim mb-2">
                PASSWORD
              </label>
              <input
                className={inputClass}
                type="password"
                placeholder={"\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"}
                value={auth.password}
                onChange={(e) => auth.setPassword(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleSave()}
              />

              <label className="block text-[10px] font-bold tracking-widest text-text-dim mb-2 mt-4">
                ALIASES (OPTIONAL)
              </label>
              <input
                className={inputClass}
                type="text"
                placeholder="alt@example.com, other@example.com"
                value={auth.aliases}
                onChange={(e) => auth.setAliases(e.target.value)}
              />
              <p className="text-[11px] text-text-dim mt-1.5">
                Other email addresses that belong to you, comma-separated.
              </p>
            </div>

            <div className="flex gap-3">
              <button
                type="button"
                className="flex-1 py-3.5 border border-divider rounded-[12px] bg-bg-secondary text-text-primary text-[14px] font-semibold cursor-pointer hover:bg-bg-tertiary transition"
                onClick={() => {
                  setStep(discovery ? "auth" : "email");
                }}
              >
                Back
              </button>
              <button
                type="button"
                className="flex-2 py-3.5 border-none rounded-[12px] bg-accent-green text-white text-[15px] font-extrabold cursor-pointer hover:brightness-95 disabled:opacity-60 disabled:cursor-not-allowed transition"
                disabled={!imapHost || !smtpHost || !auth.password.trim()}
                onClick={handleSave}
              >
                Log in
              </button>
            </div>
          </div>
        )}

        {/* Step 5: Saving */}
        {step === "saving" && (
          <div className="w-full text-center py-8">
            <div className="w-8 h-8 border-3 border-accent-green/30 border-t-accent-green rounded-full animate-spin mx-auto mb-4" />
            <h3 className="text-base font-semibold text-text-primary mb-2">
              Connecting...
            </h3>
            <p className="text-sm text-text-muted">
              Setting up your account...
            </p>
          </div>
        )}

        <p className="text-text-muted text-[13px] text-center mt-6 leading-relaxed max-w-[320px]">
          Your trust network will be derived locally from your email history. Nothing leaves your device.
        </p>
      </div>
      </div>
    </div>
  );
}
