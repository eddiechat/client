import { useState, useRef, useCallback } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useData, useTabSearch, useTheme } from "../../../shared/context";
import { fetchClusterMessages, groupDomains, ungroupDomains } from "../../../tauri";
import type { Message } from "../../../tauri";
import {
  relTime,
  lineEmoji,
  lineColor,
  dedup,
} from "../../../shared/lib";
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
  const [expandedMsgId, setExpandedMsgId] = useState<string | null>(null);
  const [lineMessages, setLineMessages] = useState<Record<string, Message[]>>({});
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
      if (!lineMessages[c.id]) {
        try {
          const msgs = await fetchClusterMessages(c.account_id, c.id);
          setLineMessages((prev) => ({ ...prev, [c.id]: msgs }));
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
    if (c.is_skill) return; // skill clusters can't be grouped/ungrouped
    didLongPress.current = false;
    longPressTimer.current = setTimeout(() => {
      didLongPress.current = true;
      toggleSelection(c.id);
      // For grouped clusters, toggle sender reveal
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
    const allSenders = selectedClusters.flatMap((c) => JSON.parse(c.domains) as string[]);
    await groupDomains(accountId, name.trim(), allSenders);
    setSelectedIds(new Set());
    setLineMessages({});
    setExpandedLines(new Set());
    setRevealedIds(new Set());
    await refresh(accountId);
  }, [accountId, selectedClusters, refresh]);

  const handleUngroup = useCallback(async () => {
    if (!accountId || selectedClusters.length !== 1) return;
    await ungroupDomains(accountId, selectedClusters[0].id);
    setSelectedIds(new Set());
    setLineMessages({});
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
        {filteredClusters.map((c, i) => {
          const isExpanded = expandedLines.has(c.id);
          const isSelected = selectedIds.has(c.id);
          const isRevealed = revealedIds.has(c.id);
          const msgs = lineMessages[c.id] || [];
          const uniqueMsgs = dedup([...msgs]).sort((a, b) => b.date - a.date);
          const msgCount = msgs.length > 0 ? uniqueMsgs.length : c.message_count;
          const previewMsgs = uniqueMsgs.slice(0, 3);

          // For grouped clusters when long-pressed, show sender list instead of message count
          const senderList = c.is_join ? (JSON.parse(c.domains) as string[]).join(", ") : "";
          const subtitle = c.is_join && isRevealed ? senderList : `${msgCount} messages`;
          const displayIcon = c.is_skill && c.icon ? c.icon : lineEmoji(c.name);
          const displayColor = c.is_skill && c.icon_bg ? c.icon_bg : lineColor(c.name);
          const isLastSkill = c.is_skill && (!filteredClusters[i + 1]?.is_skill);

          return (
            <div key={c.id} className="border-b border-divider" style={isLastSkill ? { borderBottomColor: "var(--color-accent-green)", borderBottomWidth: 1, borderImage: "linear-gradient(to right, transparent, var(--color-accent-green) 30%, var(--color-accent-green) 70%, transparent) 1" } : undefined}>
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
                      <div className="absolute w-9.5 h-9.5 rounded-[10px] top-0 left-0.5" style={{ background: `linear-gradient(${displayColor}10,${displayColor}10),var(--color-bg-primary)`, border: `1px solid ${displayColor}30`, transform: "rotate(-8deg)", transformOrigin: "center bottom" }} />
                      <div className="absolute w-9.5 h-9.5 rounded-[10px] top-0 left-0.5" style={{ background: `linear-gradient(${displayColor}15,${displayColor}15),var(--color-bg-primary)`, border: `1px solid ${displayColor}35`, transform: "rotate(4deg)", transformOrigin: "center bottom" }} />
                    </>
                  )}
                  <div
                    className={`w-9.5 h-9.5 rounded-[10px] flex items-center justify-center text-[20px] ${c.is_join ? "absolute bottom-0 left-1" : ""}`}
                    style={c.is_skill
                      ? { background: "var(--color-green-bg)", border: "1px solid var(--color-accent-green)" }
                      : { background: `linear-gradient(${displayColor}20,${displayColor}20),var(--color-bg-primary)`, border: `1px solid ${displayColor}40` }}
                  >
                    {c.is_skill
                      ? <span style={{ filter: "grayscale(1) brightness(0.6) sepia(1) hue-rotate(90deg) saturate(3)" }}>{displayIcon}</span>
                      : displayIcon}
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
                  {previewMsgs.map((m) => {
                    const isMsgExpanded = expandedMsgId === m.id;
                    const body = m.distilled_text || m.body_text || "";
                    return (
                      <div
                        key={m.id}
                        className="py-2.5 px-5 pl-[70px] border-t border-divider cursor-pointer"
                        onClick={(e) => { e.stopPropagation(); setExpandedMsgId(isMsgExpanded ? null : m.id); }}
                      >
                        <div className="flex justify-between">
                          <span className="font-semibold text-[14px] text-text-primary truncate">{m.subject || "(no subject)"}</span>
                          <span className="text-[11px] text-text-dim shrink-0 ml-2">{relTime(m.date)}</span>
                        </div>
                        <div className="text-[13px] text-text-secondary mt-px truncate">{body.slice(0, 80) || ""}</div>
                        {isMsgExpanded && body && (
                          <div className="mt-2 px-3 py-2.5 bg-bg-tertiary rounded-lg text-[13px] leading-relaxed text-text-muted border border-divider whitespace-pre-wrap break-words">
                            {body}
                          </div>
                        )}
                      </div>
                    );
                  })}
                  {msgCount > 3 && (
                    <div className="py-2 px-5 pl-[70px]">
                      <button
                        className="text-[12px] text-accent-amber font-semibold cursor-pointer bg-transparent border-none"
                        onClick={(e) => { e.stopPropagation(); navigate({ to: "/cluster/$id", params: { id: c.id } }); }}
                      >
                        View all {msgCount} messages {"\u2192"}
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
