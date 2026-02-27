import { useState, useEffect } from "react";
import { createFileRoute, Outlet, useNavigate, useLocation } from "@tanstack/react-router";
import { useAuth, useData, SearchContext, ChatFilterContext } from "../../shared/context";
import type { ChatFilter } from "../../shared/context";
import { participantCount } from "../../shared/lib";
import { Avatar, LogoPill } from "../../shared/components";
import { getAppVersion, getSetting } from "../../tauri";

export const Route = createFileRoute("/_app/_tabs")({
  component: TabsLayout,
});


const TAB_ACCENT: Record<string, string> = {
  points: "var(--color-accent-green)",
  circles: "var(--color-accent-purple)",
  lines: "var(--color-accent-amber)",
};

function ChatBubbleIcon() {
  return (
    <svg width="30" height="28" viewBox="0 0 26 24" fill="none">
      <path
        fill="#5BBCF5"
        d="M 4 0 L 22 0 Q 26 0 26 4 L 26 15 Q 26 19 22 19 L 12 19 L 7 24 L 7 19 L 4 19 Q 0 19 0 15 L 0 4 Q 0 0 4 0 Z"
      />
    </svg>
  );
}

function LetterIcon() {
  return (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
      <rect width="24" height="24" rx="5" fill="#F5C43A" />
      <rect x="5" y="7" width="14" height="2" rx="1" fill="white" />
      <rect x="5" y="11" width="14" height="2" rx="1" fill="white" />
      <rect x="5" y="15" width="9" height="2" rx="1" fill="white" />
    </svg>
  );
}

function TabsLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { email } = useAuth();
  const { status, conversations, clusters } = useData();
  const [showAccountDrawer, setShowAccountDrawer] = useState(false);
  const [search, setSearch] = useState("");
  const [chatFilter, setChatFilter] = useState<ChatFilter>("all");
  const [dismissed, setDismissed] = useState(false);
  const [showToaster, setShowToaster] = useState(false);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getSetting("show_toaster").then((v) => {
      if (v !== null) setShowToaster(v === "true");
    });
  }, []);

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
    <div
      className="relative flex flex-col h-screen"
      style={{ background: "var(--color-bg-gradient)" }}
    >
      <div className="flex-1 overflow-y-auto">
        {/* Header */}
        <div className="px-4 pb-2.5" style={{ paddingTop: 'calc(0.5rem + env(safe-area-inset-top, 0px))' }}>
          <div className="flex items-center gap-3" onClick={() => setShowAccountDrawer(true)} style={{ cursor: "pointer" }}>
            <LogoPill height={54} />
            <div className="flex flex-col">
              <span style={{ color: "#5BBCF5", fontSize: "34px", fontWeight: 900, letterSpacing: "-0.5px", lineHeight: 1 }}>eddie</span>
              <span style={{ color: "#F5C43A", fontSize: "12px", fontWeight: 700, marginTop: "3px" }}>{subtitle}</span>
            </div>
          </div>
        </div>
        {/* Search + Filter */}
        <div className="px-2.5 pb-2 flex items-center gap-2">
          <div className="flex-1 flex items-center gap-2 px-3 py-2 rounded-[10px] border border-divider">
            <span className="text-text-dim text-[16px]">{"\u2315"}</span>
            <input
              className="flex-1 bg-transparent border-none outline-none text-[16px] font-medium text-text-primary placeholder:text-text-dim"
              placeholder={"Search\u2026"}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          {activeTab === "points" && (
            <button
              className="shrink-0 w-12 self-stretch rounded-[10px] card-row text-accent-green text-[13px] font-extrabold cursor-pointer text-center"
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
        className="absolute right-3.5 z-10 w-14 h-14 rounded-[17px] flex items-center justify-center cursor-pointer text-white text-[38px] font-light leading-none"
        style={{
          bottom: 'calc(4.5rem + 0.875rem)',
          background: TAB_ACCENT[activeTab],
          boxShadow: `0 4px 14px color-mix(in srgb, ${TAB_ACCENT[activeTab]} 40%, transparent)`,
        }}
      >
        +
      </div>

      {/* Bottom Tab Bar */}
      <nav className="relative flex flex-col border-t border-divider bg-bg-secondary shrink-0">
        {showToaster && status && !dismissed && (
          <div className="absolute left-3 right-3 -top-12 z-10 flex items-center gap-2 px-4 py-2.5 text-[15px] text-text-muted bg-bg-secondary border border-divider rounded-[10px]" style={{ boxShadow: "var(--shadow-card)" }}>
            <span className="flex-1 text-center">{status}</span>
            <button className="shrink-0 text-text-dim hover:text-text-secondary text-[18px] leading-none bg-transparent border-none cursor-pointer p-0" onClick={() => setDismissed(true)}>&times;</button>
          </div>
        )}
        <div className="flex">
          <button
            className="flex-1 flex flex-col items-center gap-0.5 py-3.5 border-none bg-transparent cursor-pointer text-[10px] font-extrabold tracking-wide transition-colors"
            style={{ color: "#5BBCF5" }}
            onClick={() => navigate({ to: "/points" })}
          >
            <span className="flex items-center justify-center w-8 h-8">
              <ChatBubbleIcon />
            </span>
            Chats
            {activeTab === "points" && <span className="w-1 h-1 rounded-full" style={{ background: "#5BBCF5" }} />}
          </button>
          <button
            className="flex-1 flex flex-col items-center gap-0.5 py-3.5 border-none bg-transparent text-[10px] font-extrabold tracking-wide opacity-30"
            style={{ color: "#A78BFA", cursor: "default" }}
            disabled
          >
            <span className="flex items-center justify-center w-8 h-8">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
                <rect width="24" height="24" rx="5" fill="#A78BFA" />
                <text x="12" y="17" textAnchor="middle" fill="white" fontSize="14" fontWeight="900" fontFamily="system-ui, sans-serif">#</text>
              </svg>
            </span>
            Groups
          </button>
          <button
            className="flex-1 flex flex-col items-center gap-0.5 py-3.5 border-none bg-transparent cursor-pointer text-[10px] font-extrabold tracking-wide transition-colors"
            style={{ color: "#F5C43A" }}
            onClick={() => navigate({ to: "/lines" })}
          >
            <span className="flex items-center justify-center w-8 h-8">
              <LetterIcon />
            </span>
            Lanes
            {activeTab === "lines" && <span className="w-1 h-1 rounded-full" style={{ background: "#F5C43A" }} />}
          </button>
        </div>
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
              <div className="text-[10px] font-extrabold text-text-dim tracking-[0.1em] mb-2.5">ACTIVE ACCOUNT</div>
              <div className="flex items-center gap-3">
                <Avatar name={email || "E"} email={email || undefined} size={13} fontSize="text-[22px]" />
                <div className="flex-1">
                  <div className="text-[18px] font-bold text-text-primary">Personal</div>
                  <div className="text-[15px] text-text-muted">{email}</div>
                </div>
                <div className="w-2 h-2 rounded-full bg-accent-green" style={{ boxShadow: "0 0 6px var(--color-green-glow)" }} />
              </div>
            </div>

            <div className="h-px bg-divider" />

            {/* Add account */}
            <div className="py-2">
              <div className="flex items-center gap-3 px-4 py-2.5 cursor-pointer">
                <div className="w-11 h-11 rounded-[13px] border-[1.5px] border-dashed border-text-dim flex items-center justify-center text-[22px] text-text-dim font-light leading-none">+</div>
                <span className="text-[16px] text-text-muted font-semibold">Add account</span>
              </div>
            </div>

            <div className="h-px bg-divider" />

            {/* Settings */}
            <div
              className="flex items-center gap-3 px-4 py-3.5 cursor-pointer"
              onClick={() => { setShowAccountDrawer(false); navigate({ to: "/settings" }); }}
            >
              <span className="text-[22px] text-text-muted">{"\u2699"}</span>
              <span className="text-[16px] font-bold text-text-secondary">Settings</span>
              {version && <span className="ml-auto text-[13px] text-text-dim">v{version}</span>}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
