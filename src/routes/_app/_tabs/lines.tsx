import { useState, useRef, useCallback } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useData, useTabSearch, useTheme } from "../../../shared/context";
import { fetchClusterThreads, groupDomains, ungroupDomains } from "../../../tauri";
import type { Thread } from "../../../tauri";
import {
  relTime,
  lineEmoji,
  lineColor,
} from "../../../shared/lib";
import { Avatar } from "../../../shared/components";
import type { Cluster } from "../../../tauri";
import { useAuth } from "../../../shared/context";

export const Route = createFileRoute("/_app/_tabs/lines")({
  component: LinesList,
});

const LONG_PRESS_MS = 500;

function LinesList() {
  useTheme(); // subscribe to theme changes for avatar colors
  const navigate = useNavigate();
  const search = useTabSearch();
  const { clusters, refresh } = useData();
  const { accountId } = useAuth();

  const [expandedLines, setExpandedLines] = useState<Set<string>>(new Set());
  const [lineThreads, setLineThreads] = useState<Record<string, Thread[]>>({});
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showNamePopup, setShowNamePopup] = useState(false);
  const [groupName, setGroupName] = useState("");
  const [revealedIds, setRevealedIds] = useState<Set<string>>(new Set());

  // Long-press refs
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const didLongPress = useRef(false);

  const q = search.toLowerCase();
  const filteredClusters = clusters.filter(
    (c) => !q || c.name.toLowerCase().includes(q)
  );

  const suggestedSkillBadges = [
    { icon: "\u2708\uFE0F", name: "Travel" },
    { icon: "\uD83D\uDCE6", name: "Packages" },
    { icon: "\uD83D\uDCBC", name: "Job Apps" },
  ];

  async function toggleLine(c: Cluster) {
    const next = new Set(expandedLines);
    if (next.has(c.id)) {
      next.delete(c.id);
    } else {
      next.add(c.id);
      if (!lineThreads[c.id]) {
        try {
          const threads = await fetchClusterThreads(c.account_id, c.id);
          setLineThreads((prev) => ({ ...prev, [c.id]: threads }));
        } catch {
          /* ignore */
        }
      }
    }
    setExpandedLines(next);
  }

  function toggleSelection(id: string) {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function handlePointerDown(c: Cluster) {
    didLongPress.current = false;
    longPressTimer.current = setTimeout(() => {
      didLongPress.current = true;
      toggleSelection(c.id);
      // For grouped clusters, toggle domain reveal
      if (c.is_join) {
        setRevealedIds((prev) => {
          const next = new Set(prev);
          if (next.has(c.id)) next.delete(c.id);
          else next.add(c.id);
          return next;
        });
      }
    }, LONG_PRESS_MS);
  }

  function clearLongPress() {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }

  function handleClick(c: Cluster) {
    if (didLongPress.current) {
      didLongPress.current = false;
      return;
    }
    if (selectedIds.size > 0) {
      // In selection mode: tap toggles selection
      toggleSelection(c.id);
    } else {
      // Normal mode: expand/collapse
      toggleLine(c);
    }
  }

  // Determine toolbar action
  const selectedClusters = filteredClusters.filter((c) => selectedIds.has(c.id));
  const showGroup = selectedIds.size >= 2;
  const showUngroup = selectedIds.size === 1 && selectedClusters.length === 1 && selectedClusters[0].is_join;

  const handleGroup = useCallback(async (name: string) => {
    if (!accountId || selectedClusters.length < 2 || !name.trim()) return;
    const allDomains = selectedClusters.flatMap((c) => JSON.parse(c.domains) as string[]);
    await groupDomains(accountId, name.trim(), allDomains);
    setSelectedIds(new Set());
    setLineThreads({});
    setExpandedLines(new Set());
    setRevealedIds(new Set());
    await refresh(accountId);
  }, [accountId, selectedClusters, refresh]);

  const handleUngroup = useCallback(async () => {
    if (!accountId || selectedClusters.length !== 1) return;
    await ungroupDomains(accountId, selectedClusters[0].id);
    setSelectedIds(new Set());
    setLineThreads({});
    setExpandedLines(new Set());
    setRevealedIds(new Set());
    await refresh(accountId);
  }, [accountId, selectedClusters, refresh]);

  return (
    <>
      {/* Name popup */}
      {showNamePopup && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={() => setShowNamePopup(false)}>
          <div className="bg-bg-primary border border-divider rounded-xl p-5 w-70 shadow-lg" onClick={(e) => e.stopPropagation()}>
            <div className="text-[15px] font-semibold text-text-primary mb-3">Name this group</div>
            <input
              autoFocus
              type="text"
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && groupName.trim()) {
                  setShowNamePopup(false);
                  handleGroup(groupName);
                  setGroupName("");
                }
              }}
              placeholder="e.g. Shopping, Work, Travel..."
              className="w-full px-3 py-2 rounded-lg border border-divider bg-bg-secondary text-[14px] text-text-primary outline-none focus:border-accent-green"
            />
            <div className="flex gap-2 mt-3 justify-end">
              <button
                className="px-3 py-1.5 rounded-lg text-[13px] text-text-secondary cursor-pointer bg-transparent border border-divider"
                onClick={() => { setShowNamePopup(false); setGroupName(""); }}
              >
                Cancel
              </button>
              <button
                className="px-3 py-1.5 rounded-lg text-[13px] text-white cursor-pointer bg-accent-green border-none font-semibold disabled:opacity-40"
                disabled={!groupName.trim()}
                onClick={() => { setShowNamePopup(false); handleGroup(groupName); setGroupName(""); }}
              >
                Group
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Toolbar */}
      <div className="flex gap-1.5 px-5 py-2 overflow-x-auto">
        <button
          className="px-2.5 py-[5px] rounded-lg border-[1.5px] border-dashed border-text-dim bg-transparent text-[12px] whitespace-nowrap cursor-pointer text-text-dim"
          onClick={() => navigate({ to: "/skills/hub" })}
        >
          + Skill
        </button>
        {showGroup && (
          <button
            className="px-2.5 py-[5px] rounded-lg border-[1.5px] border-accent-green bg-green-bg text-[12px] whitespace-nowrap cursor-pointer text-accent-green font-semibold"
            onClick={() => {
              const existing = selectedClusters.find((c) => c.is_join);
              setGroupName(existing ? existing.name : "");
              setShowNamePopup(true);
            }}
          >
            Group
          </button>
        )}
        {showUngroup && (
          <button
            className="px-2.5 py-[5px] rounded-lg border-[1.5px] border-accent-red bg-red-bg text-[12px] whitespace-nowrap cursor-pointer text-accent-red font-semibold"
            onClick={handleUngroup}
          >
            Ungroup
          </button>
        )}
        {suggestedSkillBadges.map((s) => (
          <button
            key={s.name}
            className="px-2.5 py-[5px] rounded-lg border border-divider bg-bg-secondary text-[12px] whitespace-nowrap cursor-pointer text-text-secondary"
            onClick={() => navigate({ to: "/skills/hub" })}
          >
            {"\u26A1"} {s.name}
          </button>
        ))}
      </div>

      <div>
        {filteredClusters.length === 0 && (
          <div className="text-center py-15 px-5 text-text-muted text-[14px]">No lines yet</div>
        )}
        {filteredClusters.map((c) => {
          const isExpanded = expandedLines.has(c.id);
          const isSelected = selectedIds.has(c.id);
          const isRevealed = revealedIds.has(c.id);
          const threads = lineThreads[c.id] || [];
          const threadCount = threads.length > 0 ? threads.length : c.thread_count;
          const previewThreads = threads.slice(0, 3);

          // For grouped clusters when long-pressed, show domain list instead of thread count
          const domainList = c.is_join ? (JSON.parse(c.domains) as string[]).join(", ") : "";
          const subtitle = c.is_join && isRevealed ? domainList : `${threadCount} threads`;

          return (
            <div key={c.id} className="border-b border-divider">
              <div
                className={`flex items-center px-5 py-3 gap-3 cursor-pointer transition-colors ${isSelected ? "bg-bg-hover" : isExpanded ? "bg-bg-tertiary" : ""}`}
                onPointerDown={() => handlePointerDown(c)}
                onPointerUp={clearLongPress}
                onPointerCancel={clearLongPress}
                onPointerLeave={clearLongPress}
                onClick={() => handleClick(c)}
                onContextMenu={(e) => e.preventDefault()}
              >
                <div className={`relative shrink-0 ${c.is_join ? "w-12 h-11" : "w-9.5 h-9.5"}`}>
                  {c.is_join && (
                    <>
                      <div className="absolute w-9.5 h-9.5 rounded-[10px] top-0 left-0.5" style={{ background: `linear-gradient(${lineColor(c.name)}10,${lineColor(c.name)}10),var(--color-bg-primary)`, border: `1px solid ${lineColor(c.name)}30`, transform: "rotate(-8deg)", transformOrigin: "center bottom" }} />
                      <div className="absolute w-9.5 h-9.5 rounded-[10px] top-0 left-0.5" style={{ background: `linear-gradient(${lineColor(c.name)}15,${lineColor(c.name)}15),var(--color-bg-primary)`, border: `1px solid ${lineColor(c.name)}35`, transform: "rotate(4deg)", transformOrigin: "center bottom" }} />
                    </>
                  )}
                  <div
                    className={`w-9.5 h-9.5 rounded-[10px] flex items-center justify-center text-[20px] ${c.is_join ? "absolute bottom-0 left-1" : ""}`}
                    style={{ background: `linear-gradient(${lineColor(c.name)}20,${lineColor(c.name)}20),var(--color-bg-primary)`, border: `1px solid ${lineColor(c.name)}40` }}
                  >
                    {lineEmoji(c.name)}
                  </div>
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex justify-between items-baseline gap-2">
                    <span className={`text-[15.5px] truncate flex-1 ${c.unread_count > 0 ? "font-semibold text-text-primary" : "font-normal text-text-secondary"}`}>{c.name}</span>
                    <span className={`text-[12px] shrink-0 ${c.unread_count > 0 ? "text-accent-green font-semibold" : "text-text-dim"}`}>
                      {relTime(c.last_activity)}
                    </span>
                  </div>
                  <div className="flex justify-between items-center gap-2 mt-0.5">
                    <span className="text-[14px] text-text-muted truncate flex-1">{subtitle}</span>
                    {c.unread_count > 0 && (
                      <span className="min-w-[20px] h-5 rounded-[10px] bg-accent-green text-white text-[11px] font-bold flex items-center justify-center px-1.5 shrink-0">
                        {c.unread_count}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {isExpanded && (
                <div className="bg-bg-secondary">
                  {previewThreads.map((t) => {
                    const sender = t.from_name || t.from_address;
                    const hasUnread = t.unread_count > 0;
                    return (
                      <div
                        key={t.thread_id}
                        className="flex items-start gap-2.5 py-2.5 px-5 pl-[70px] border-t border-divider cursor-pointer"
                        onClick={() => navigate({ to: "/cluster/$id", params: { id: c.id } })}
                      >
                        <Avatar name={sender} email={t.from_address} size={8} fontSize="text-[11px]" className="shrink-0 mt-0.5" />
                        <div className="flex-1 min-w-0">
                          <div className="flex justify-between items-center">
                            <div className="flex items-center gap-1.5 min-w-0">
                              {hasUnread && <div className="w-1.5 h-1.5 rounded-full bg-accent-green shrink-0" />}
                              <span className={`text-[14px] truncate ${hasUnread ? "font-bold text-text-primary" : "font-semibold text-text-secondary"}`}>{sender}</span>
                            </div>
                            <span className={`text-[11px] shrink-0 ml-2 ${hasUnread ? "text-accent-green font-semibold" : "text-text-dim"}`}>{relTime(t.last_activity)}</span>
                          </div>
                          <div className="flex justify-between items-center gap-2 mt-px">
                            <span className={`text-[13px] truncate flex-1 ${hasUnread ? "text-text-primary" : "text-text-secondary"}`}>
                              {t.subject || "(no subject)"}
                            </span>
                            {t.message_count > 1 && (
                              <span className="text-[11px] text-text-dim shrink-0">({t.message_count})</span>
                            )}
                          </div>
                          {t.preview && (
                            <div className="text-[12px] text-text-muted mt-0.5 truncate">{t.preview}</div>
                          )}
                        </div>
                      </div>
                    );
                  })}
                  {threadCount > 3 && (
                    <div className="py-2 px-5 pl-[70px]">
                      <button
                        className="text-[12px] text-accent-amber font-semibold cursor-pointer bg-transparent border-none"
                        onClick={(e) => { e.stopPropagation(); navigate({ to: "/cluster/$id", params: { id: c.id } }); }}
                      >
                        View all {threadCount} threads {"\u2192"}
                      </button>
                    </div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </>
  );
}
