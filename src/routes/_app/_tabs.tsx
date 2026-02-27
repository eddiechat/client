import { useState, useEffect } from "react";
import { createFileRoute, Outlet, useNavigate, useLocation } from "@tanstack/react-router";
import { useAuth, useData, SearchContext, ChatFilterContext } from "../../shared/context";
import type { ChatFilter } from "../../shared/context";
import { participantCount } from "../../shared/lib";
import { Avatar } from "../../shared/components";
import { getAppVersion } from "../../tauri";

export const Route = createFileRoute("/_app/_tabs")({
  component: TabsLayout,
});

const TAB_TITLES: Record<string, { emoji: string; label: string }> = {
  points: { emoji: "\uD83D\uDCAC", label: "Chats" },
  circles: { emoji: "\uD83D\uDC65", label: "Groups" },
  lines: { emoji: "\uD83C\uDFF7", label: "Lanes" },
};

const TAB_ACCENT: Record<string, string> = {
  points: "var(--color-accent-green)",
  circles: "var(--color-accent-purple)",
  lines: "var(--color-accent-amber)",
};

function TabsLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { email } = useAuth();
  const { status, conversations, clusters } = useData();
  const [showAccountDrawer, setShowAccountDrawer] = useState(false);
  const [search, setSearch] = useState("");
  const [chatFilter, setChatFilter] = useState<ChatFilter>("all");
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

  const { label: title } = TAB_TITLES[activeTab];
  const conns = conversations.filter((c) => c.classification === "connections");
  const tabUnread = activeTab === "lines"
    ? clusters.reduce((sum, c) => sum + c.unread_count, 0)
    : chatFilter === "1:1"
      ? conns.filter((c) => participantCount(c) === 1).reduce((sum, c) => sum + c.unread_count, 0)
      : chatFilter === "3+"
        ? conns.filter((c) => participantCount(c) > 1).reduce((sum, c) => sum + c.unread_count, 0)
        : conns.reduce((sum, c) => sum + c.unread_count, 0);
  const subtitle = tabUnread > 0 ? `${tabUnread} unread messages` : "All caught up";

  return (
    <div className="relative flex flex-col h-screen bg-bg-primary">
      <div className="flex-1 overflow-y-auto">
        {/* Header */}
        <div className="px-4 pb-2.5 bg-bg-primary" style={{ paddingTop: 'calc(0.5rem + env(safe-area-inset-top, 0px))' }}>
          <div className="flex items-start gap-3">
            <div onClick={() => setShowAccountDrawer(true)} className="cursor-pointer shrink-0 mt-0.5">
              <Avatar name={email || "E"} email={email || undefined} size={9} fontSize="text-[13px]" />
            </div>
            <h1 className="text-[28px] text-text-primary" style={{ letterSpacing: "-0.5px", fontWeight: 900 }}>{title}</h1>
          </div>
          <div className="text-[10px] font-semibold text-text-muted mt-0.5">{subtitle}</div>
        </div>
        {/* Search + Filter */}
        <div className="px-2.5 pb-2 flex items-center gap-2">
          <div className="flex-1 flex items-center gap-2 px-3 py-2 rounded-[10px] bg-bg-tertiary border border-divider">
            <span className="text-text-dim text-[13px]">{"\u2315"}</span>
            <input
              className="flex-1 bg-transparent border-none outline-none text-[13px] font-medium text-text-primary placeholder:text-text-dim"
              placeholder={"Search\u2026"}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          {activeTab === "points" && (
            <button
              className="shrink-0 w-12 self-stretch rounded-[10px] card-row text-accent-green text-[11px] font-extrabold cursor-pointer text-center"
              onClick={() => setChatFilter((prev) => prev === "all" ? "1:1" : prev === "1:1" ? "3+" : "all")}
            >
              {chatFilter === "all" ? "All" : chatFilter}
            </button>
          )}
        </div>

        <SearchContext.Provider value={search}>
          <ChatFilterContext.Provider value={chatFilter}>
            <Outlet />
          </ChatFilterContext.Provider>
        </SearchContext.Provider>
      </div>

      {/* FAB — Compose button */}
      <div
        className="absolute right-3.5 z-10 w-12 h-12 rounded-[15px] flex items-center justify-center cursor-pointer text-white text-[34px] font-light leading-none"
        style={{
          bottom: 'calc(5.5rem + env(safe-area-inset-bottom, 0px))',
          background: TAB_ACCENT[activeTab],
          boxShadow: `0 4px 14px color-mix(in srgb, ${TAB_ACCENT[activeTab]} 40%, transparent)`,
        }}
      >
        +
      </div>

      {/* Bottom Tab Bar */}
      <nav className="relative flex border-t border-divider bg-bg-secondary px-0 pt-1.5 shrink-0" style={{ paddingBottom: 'calc(0.5rem + env(safe-area-inset-bottom, 0px))' }}>
        {import.meta.env.DEV && status && !dismissed && (
          <div className="absolute left-3 right-3 -top-12 z-10 flex items-center gap-2 px-4 py-2.5 text-[12px] text-text-muted bg-bg-secondary border border-divider rounded-[10px]" style={{ boxShadow: "var(--shadow-card)" }}>
            <span className="flex-1 text-center">{status}</span>
            <button className="shrink-0 text-text-dim hover:text-text-secondary text-[15px] leading-none bg-transparent border-none cursor-pointer p-0" onClick={() => setDismissed(true)}>&times;</button>
          </div>
        )}
        <button
          className={`flex-1 flex flex-col items-center gap-0.5 py-1.5 border-none bg-transparent cursor-pointer text-[8.5px] font-extrabold tracking-wide transition-colors ${activeTab === "points" ? "text-accent-green" : "text-text-dim"}`}
          onClick={() => navigate({ to: "/points" })}
        >
          <span className="flex items-center justify-center w-7 h-7 text-[17px]">{"\uD83D\uDCAC"}</span>
          Chats
          {activeTab === "points" && <span className="w-1 h-1 rounded-full bg-accent-green" />}
        </button>
        <button
          className={`flex-1 flex flex-col items-center gap-0.5 py-1.5 border-none bg-transparent cursor-pointer text-[8.5px] font-extrabold tracking-wide transition-colors ${activeTab === "lines" ? "text-accent-amber" : "text-text-dim"}`}
          onClick={() => navigate({ to: "/lines" })}
        >
          <span className="flex items-center justify-center w-7 h-7 text-[17px]">{"\uD83C\uDFF7"}</span>
          Lanes
          {activeTab === "lines" && <span className="w-1 h-1 rounded-full bg-accent-amber" />}
        </button>
      </nav>

      {/* Account Drawer Overlay */}
      {showAccountDrawer && (
        <div className="absolute inset-0 z-50" style={{ background: "rgba(0,0,0,0.25)", backdropFilter: "blur(4px)" }} onClick={() => setShowAccountDrawer(false)}>
          <div
            className="absolute left-3 right-3 bg-bg-secondary rounded-[20px] overflow-hidden"
            style={{ top: 'calc(3.5rem + env(safe-area-inset-top, 0px))', boxShadow: "0 16px 48px rgba(0,0,0,0.12)" }}
            onClick={(e) => e.stopPropagation()}
          >
            {/* Active account */}
            <div className="px-4 pt-4 pb-3">
              <div className="text-[9px] font-extrabold text-text-dim tracking-[0.1em] mb-2.5">ACTIVE ACCOUNT</div>
              <div className="flex items-center gap-3">
                <Avatar name={email || "E"} email={email || undefined} size={11} fontSize="text-[17px]" />
                <div className="flex-1">
                  <div className="text-[15px] font-bold text-text-primary">Personal</div>
                  <div className="text-[12px] text-text-muted">{email}</div>
                </div>
                <div className="w-2 h-2 rounded-full bg-accent-green" style={{ boxShadow: "0 0 6px var(--color-green-glow)" }} />
              </div>
            </div>

            <div className="h-px bg-divider" />

            {/* Add account */}
            <div className="py-2">
              <div className="flex items-center gap-3 px-4 py-2.5 cursor-pointer">
                <div className="w-9.5 h-9.5 rounded-[11px] border-[1.5px] border-dashed border-text-dim flex items-center justify-center text-[18px] text-text-dim font-light leading-none">+</div>
                <span className="text-[13px] text-text-muted font-semibold">Add account</span>
              </div>
            </div>

            <div className="h-px bg-divider" />

            {/* Settings */}
            <div
              className="flex items-center gap-3 px-4 py-3.5 cursor-pointer"
              onClick={() => { setShowAccountDrawer(false); navigate({ to: "/settings" }); }}
            >
              <span className="text-[18px] text-text-muted">{"\u2699"}</span>
              <span className="text-[13px] font-bold text-text-secondary">Settings</span>
              {version && <span className="ml-auto text-[11px] text-text-dim">v{version}</span>}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
