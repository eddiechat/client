import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react";
import { getSetting, setSetting } from "../../tauri";

type ThemeMode = "light" | "dark" | "system";

interface ThemeContextValue {
  theme: ThemeMode;
  setTheme: (mode: ThemeMode) => void;
}

const ThemeContext = createContext<ThemeContextValue>({ theme: "light", setTheme: () => {} });

function applyTheme(mode: ThemeMode) {
  const isDark =
    mode === "dark" || (mode === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", isDark);
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeMode>("light");

  // Load persisted preference
  useEffect(() => {
    getSetting("dark_mode").then((v) => {
      if (v === "light" || v === "dark" || v === "system") {
        setThemeState(v);
        applyTheme(v);
      }
    });
  }, []);

  // Re-apply whenever theme changes & listen for OS changes in "system" mode
  useEffect(() => {
    applyTheme(theme);

    if (theme !== "system") return;

    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  const setTheme = useCallback((mode: ThemeMode) => {
    setThemeState(mode);
    setSetting("dark_mode", mode);
  }, []);

  return <ThemeContext.Provider value={{ theme, setTheme }}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}
