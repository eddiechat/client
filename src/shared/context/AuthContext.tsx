import { createContext, useContext, useState, useMemo, useCallback, useEffect } from "react";
import type { ReactNode } from "react";
import { connectAccount, getExistingAccount } from "../../tauri";

interface AuthContextValue {
  email: string;
  setEmail: (v: string) => void;
  aliases: string;
  setAliases: (v: string) => void;
  password: string;
  setPassword: (v: string) => void;
  imapHost: string;
  setImapHost: (v: string) => void;
  imapPort: number;
  setImapPort: (v: number) => void;
  smtpHost: string;
  setSmtpHost: (v: string) => void;
  smtpPort: number;
  setSmtpPort: (v: number) => void;
  loggedIn: boolean;
  loading: boolean;
  error: string;
  accountId: string | null;
  myAddrs: Set<string>;
  handleLogin: (overrides?: { imapHost: string; imapPort: number; imapTls?: boolean; smtpHost: string; smtpPort: number }) => Promise<string>;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [email, setEmail] = useState("");
  const [aliases, setAliases] = useState("");
  const [password, setPassword] = useState("");
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState(993);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(587);

  const [loggedIn, setLoggedIn] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [accountId, setAccountId] = useState<string | null>(null);

  // On mount, check if an account already exists in the database
  useEffect(() => {
    getExistingAccount().then((account) => {
      if (account) {
        setEmail(account.email);
        setAccountId(account.id);
        setLoggedIn(true);
      }
    }).catch(() => {
      // No existing account — stay on login screen
    });
  }, []);

  const myAddrs = useMemo(
    () =>
      new Set(
        [email, ...aliases.split(",").map((s) => s.trim())]
          .filter(Boolean)
          .map((s) => s.toLowerCase())
      ),
    [email, aliases]
  );

  const handleLogin = useCallback(async (overrides?: { imapHost: string; imapPort: number; imapTls?: boolean; smtpHost: string; smtpPort: number }) => {
    setLoading(true);
    setError("");
    try {
      const id = await connectAccount({
        email, password,
        imapHost: overrides?.imapHost ?? imapHost,
        imapPort: overrides?.imapPort ?? imapPort,
        imapTls: overrides?.imapTls,
        smtpHost: overrides?.smtpHost ?? smtpHost,
        smtpPort: overrides?.smtpPort ?? smtpPort,
        aliases: aliases || undefined,
      });
      if (overrides) {
        setImapHost(overrides.imapHost);
        setImapPort(overrides.imapPort);
        setSmtpHost(overrides.smtpHost);
        setSmtpPort(overrides.smtpPort);
      }
      setAccountId(id);
      setLoggedIn(true);
      return id;
    } catch (err) {
      setError(String(err));
      throw err;
    } finally {
      setLoading(false);
    }
  }, [email, password, imapHost, imapPort, smtpHost, smtpPort, aliases]);

  const value = useMemo<AuthContextValue>(
    () => ({
      email, setEmail,
      aliases, setAliases,
      password, setPassword,
      imapHost, setImapHost,
      imapPort, setImapPort,
      smtpHost, setSmtpHost,
      smtpPort, setSmtpPort,
      loggedIn, loading, error, accountId, myAddrs,
      handleLogin,
    }),
    [email, aliases, password, imapHost, imapPort, smtpHost, smtpPort, loggedIn, loading, error, accountId, myAddrs, handleLogin]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
