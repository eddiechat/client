import { useState, useEffect } from "react";
import { createFileRoute, Outlet, useNavigate, useLocation } from "@tanstack/react-router";
import { useAuth, useData, SearchContext } from "../../shared/context";
import { ComposeIcon } from "../../shared/components";
import { getAppVersion } from "../../tauri";

export const Route = createFileRoute("/_app/_tabs")({
  component: TabsLayout,
});

const TAB_SUBTITLES: Record<string, string> = {
  points: "Your people",
  circles: "Group vibes",
  lines: "Unpack your inbox",
};

function TabsLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { email } = useAuth();
  const { status } = useData();
  const [showAccountDrawer, setShowAccountDrawer] = useState(false);
  const [search, setSearch] = useState("");
  const [dismissed, setDismissed] = useState(false);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getAppVersion()
      .then((v) => {
        const devSuffix = import.meta.env.DEV ? " (dev)" : "";
        setVersion(`${v}${devSuffix}`);
      })
      .catch(() => {});
  }, []);

  const path = location.pathname;
  const activeTab = path.includes("/circles") ? "circles"
    : path.includes("/lines") ? "lines"
      : "points";

  const tabConfig: Record<string, string> = {
    points: "Points",
    circles: "Circles",
    lines: "Lines",
  };
  const title = tabConfig[activeTab];

  return (
    <div className="relative flex flex-col h-screen bg-bg-primary">
      <div className="flex-1 overflow-y-auto">
        {/* Header */}
        <div className="px-5 pb-2.5 bg-bg-secondary border-b border-divider" style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))' }}>
          <div className="flex justify-between items-center">
            <div className="flex items-center gap-3">
              <div
                className="w-8 h-8 rounded-xl flex items-center justify-center text-[13px] font-bold cursor-pointer bg-accent-green text-white shrink-0"
                onClick={() => setShowAccountDrawer(true)}
              >
                {email ? email[0].toUpperCase() : "E"}
              </div>
              <div>
                <h1 className="text-[22px] font-bold text-text-primary" style={{ letterSpacing: "-0.03em" }}>{title}</h1>
                <div className="text-[11px] text-text-muted -mt-0.5">{TAB_SUBTITLES[activeTab]}</div>
              </div>
            </div>
            <button
              className="w-10 h-10 rounded-xl border border-divider bg-bg-secondary flex items-center justify-center cursor-pointer shrink-0 hover:border-accent-green transition-colors"
            >
              <ComposeIcon />
            </button>
          </div>
          {/* Search */}
          <div className="mt-2.5 flex items-center gap-2 px-3 py-[9px] rounded-xl bg-bg-tertiary border border-divider">
            <span className="text-text-dim text-[13px]">{"\u2315"}</span>
            <input
              className="flex-1 bg-transparent border-none outline-none text-[12px] text-text-primary placeholder:text-text-dim"
              placeholder={"Search\u2026"}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
        </div>

        <SearchContext.Provider value={search}>
          <Outlet />
        </SearchContext.Provider>
      </div>

      {/* Bottom Tab Bar */}
      <nav className="relative flex border-t border-divider bg-bg-secondary px-0 pt-1.5 shrink-0" style={{ paddingBottom: 'calc(0.5rem + env(safe-area-inset-bottom, 0px))' }}>
        {status && !dismissed && (
          <div className="absolute left-3 right-3 -top-12 z-10 flex items-center gap-2 px-4 py-2.5 text-[12px] text-text-muted bg-bg-secondary border border-divider rounded-xl" style={{ boxShadow: "0 2px 8px rgba(0,0,0,0.06)" }}>
            <span className="flex-1 text-center">{status}</span>
            <button className="shrink-0 text-text-dim hover:text-text-secondary text-[14px] leading-none bg-transparent border-none cursor-pointer p-0" onClick={() => setDismissed(true)}>&times;</button>
          </div>
        )}
        <button
          className={`flex-1 flex flex-col items-center gap-0.5 py-1.5 border-none bg-transparent cursor-pointer text-[9px] font-semibold tracking-widest uppercase transition-colors ${activeTab === "points" ? "text-accent-green" : "text-text-dim"}`}
          onClick={() => navigate({ to: "/points" })}
        >
          <span className="flex items-center justify-center w-6 h-6 text-[16px]">{"\u25CF"}</span>
          Points
        </button>
        <button
          className={`flex-1 flex flex-col items-center gap-0.5 py-1.5 border-none bg-transparent cursor-pointer text-[9px] font-semibold tracking-widest uppercase transition-colors ${activeTab === "circles" ? "text-accent-purple" : "text-text-dim"}`}
          onClick={() => navigate({ to: "/circles" })}
        >
          <span className="flex items-center justify-center w-6 h-6 text-[16px]">{"\u25C9"}</span>
          Circles
        </button>
        <button
          className={`flex-1 flex flex-col items-center gap-0.5 py-1.5 border-none bg-transparent cursor-pointer text-[9px] font-semibold tracking-widest uppercase transition-colors ${activeTab === "lines" ? "text-accent-amber" : "text-text-dim"}`}
          onClick={() => navigate({ to: "/lines" })}
        >
          <span className="flex items-center justify-center w-6 h-6 text-[16px]">{"\u2261"}</span>
          Lines
        </button>
      </nav>

      {/* Account Drawer Overlay */}
      {showAccountDrawer && (
        <div className="absolute inset-0 z-50" style={{ background: "rgba(0,0,0,0.15)", backdropFilter: "blur(4px)" }} onClick={() => setShowAccountDrawer(false)}>
          <div
            className="absolute left-3 right-3 bg-bg-secondary border border-divider rounded-2xl overflow-hidden"
            style={{ top: 'calc(3.5rem + env(safe-area-inset-top, 0px))', boxShadow: "0 16px 48px rgba(0,0,0,0.08)" }}
            onClick={(e) => e.stopPropagation()}
          >
            {/* Active account */}
            <div className="px-4 pt-4 pb-3">
              <div className="text-[10px] font-bold text-text-dim tracking-[0.08em] mb-2.5">ACTIVE ACCOUNT</div>
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl flex items-center justify-center text-[16px] font-bold bg-accent-green text-white">
                  {email ? email[0].toUpperCase() : "E"}
                </div>
                <div className="flex-1">
                  <div className="text-[14px] font-semibold text-text-primary">Personal</div>
                  <div className="text-[12px] text-text-muted">{email}</div>
                </div>
                <div className="w-2 h-2 rounded-full bg-accent-green" style={{ boxShadow: "0 0 6px var(--color-green-glow)" }} />
              </div>
            </div>

            <div className="h-px bg-divider" />

            {/* Add account */}
            <div className="py-2">
              <div className="flex items-center gap-3 px-4 py-2.5 cursor-pointer">
                <div className="w-[34px] h-[34px] rounded-xl border-[1.5px] border-dashed border-text-dim flex items-center justify-center text-[16px] text-text-dim">+</div>
                <span className="text-[13px] text-text-muted font-medium">Add account</span>
              </div>
            </div>

            <div className="h-px bg-divider" />

            {/* Settings */}
            <div
              className="flex items-center gap-3 px-4 py-3.5 cursor-pointer"
              onClick={() => { setShowAccountDrawer(false); navigate({ to: "/settings" }); }}
            >
              <span className="text-[16px] text-text-muted">{"\u2699"}</span>
              <span className="text-[13px] font-semibold text-text-secondary">Settings</span>
              {version && <span className="ml-auto text-[11px] text-text-dim">v{version}</span>}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
