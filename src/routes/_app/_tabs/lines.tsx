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
          <div className="bg-bg-secondary border border-divider rounded-[16px] p-5 w-70" style={{ boxShadow: "var(--shadow-card)" }} onClick={(e) => e.stopPropagation()}>
            <div className="text-[16px] font-extrabold text-text-primary mb-3">Name this group</div>
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
              className="w-full px-3 py-2 rounded-[10px] border border-divider bg-bg-tertiary text-[16px] font-medium text-text-primary outline-none focus:border-accent-amber"
            />
            <div className="flex gap-2 mt-3 justify-end">
              <button
                className="px-3 py-1.5 rounded-[10px] text-[15px] text-text-secondary cursor-pointer bg-transparent border border-divider font-semibold"
                onClick={() => { setShowNamePopup(false); setGroupName(""); }}
              >
                Cancel
              </button>
              <button
                className="px-3 py-1.5 rounded-[10px] text-[15px] text-white cursor-pointer bg-accent-amber border-none font-bold disabled:opacity-40"
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
      <div className="flex gap-1.5 px-3 py-2 overflow-x-auto">
        <button
          className="px-2.5 py-1 rounded-[8px] border-[1.5px] border-dashed border-text-dim bg-transparent text-[13px] font-bold whitespace-nowrap cursor-pointer text-text-dim"
          onClick={() => navigate({ to: "/skills/hub" })}
        >
          + Skill
        </button>
        {showGroup && (
          <button
            className="px-2.5 py-1 rounded-[8px] border-[1.5px] border-accent-amber bg-amber-bg text-[13px] whitespace-nowrap cursor-pointer text-accent-amber font-bold"
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
            className="px-2.5 py-1 rounded-[8px] border-[1.5px] border-accent-red bg-red-bg text-[13px] whitespace-nowrap cursor-pointer text-accent-red font-bold"
            onClick={handleUngroup}
          >
            Ungroup
          </button>
        )}
        {suggestedSkillBadges.map((s) => (
          <button
            key={s.name}
            className="px-2.5 py-1 rounded-[8px] border border-divider bg-bg-secondary text-[13px] font-semibold whitespace-nowrap cursor-pointer text-text-secondary"
            onClick={() => navigate({ to: "/skills/hub" })}
          >
            {"\u26A1"} {s.name}
          </button>
        ))}
      </div>

      <div className="flex flex-col gap-[5px] px-2.5 pb-2">
        {filteredClusters.length === 0 && (
          <div className="text-center py-15 px-5 text-text-muted text-[16px] font-semibold">No lanes yet</div>
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
            <div key={c.id} className="card-row overflow-hidden" style={isLastSkill ? { borderColor: "var(--color-accent-amber)", borderWidth: 1 } : undefined}>
              <div
                className={`flex items-center px-3 py-2.5 gap-2.5 cursor-pointer transition-colors ${isSelected ? "bg-amber-bg" : isExpanded ? "bg-bg-tertiary" : ""}`}
                onPointerDown={() => handlePointerDown(c)}
                onPointerUp={clearLongPress}
                onPointerCancel={clearLongPress}
                onPointerLeave={clearLongPress}
                onClick={() => handleClick(c)}
                onContextMenu={(e) => e.preventDefault()}
              >
                <div className={`relative shrink-0 ${c.is_join ? "w-15 h-13" : "w-11 h-11"}`}>
                  {c.is_join && (
                    <>
                      <div className="absolute w-11 h-11 rounded-[13px] top-0 left-0.5" style={{ background: `linear-gradient(${displayColor}10,${displayColor}10),var(--color-bg-primary)`, border: `1px solid ${displayColor}30`, transform: "rotate(-8deg)", transformOrigin: "center bottom" }} />
                      <div className="absolute w-11 h-11 rounded-[13px] top-0 left-0.5" style={{ background: `linear-gradient(${displayColor}15,${displayColor}15),var(--color-bg-primary)`, border: `1px solid ${displayColor}35`, transform: "rotate(4deg)", transformOrigin: "center bottom" }} />
                    </>
                  )}
                  <div
                    className={`w-11 h-11 rounded-[13px] flex items-center justify-center text-[20px] ${c.is_join ? "absolute bottom-0 left-1" : ""}`}
                    style={c.is_skill
                      ? { background: "var(--color-amber-bg)", border: "1px solid var(--color-accent-amber)" }
                      : { background: `linear-gradient(${displayColor}20,${displayColor}20),var(--color-bg-primary)`, border: `1px solid ${displayColor}40` }}
                  >
                    {c.is_skill
                      ? <span style={{ filter: "grayscale(1) brightness(0.6) sepia(1) hue-rotate(130deg) saturate(3)" }}>{displayIcon}</span>
                      : displayIcon}
                  </div>
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex justify-between items-baseline gap-2">
                    <span className={`text-[16px] truncate flex-1 ${c.unread_count > 0 ? "font-extrabold text-text-primary" : "font-semibold text-text-secondary"}`} style={{ letterSpacing: "-0.2px" }}>{c.name}</span>
                    <span className={`text-[11px] shrink-0 ${c.unread_count > 0 ? "font-extrabold text-text-primary" : "font-semibold text-text-dim"}`}>
                      {relTime(c.last_activity)}
                    </span>
                  </div>
                  <div className="flex justify-between items-center gap-2 mt-px">
                    <span className="text-[12px] text-text-muted truncate flex-1 font-medium">{subtitle}</span>
                    {c.unread_count > 0 && (
                      <span className="min-w-4.5 h-4.5 rounded-lg bg-accent-amber text-white text-[11px] font-extrabold flex items-center justify-center px-1 shrink-0">
                        {c.unread_count}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {isExpanded && (
                <div className="bg-bg-secondary border-t border-divider">
                  {previewMsgs.map((m) => {
                    const isMsgExpanded = expandedMsgId === m.id;
                    const body = m.distilled_text || m.body_text || "";
                    return (
                      <div
                        key={m.id}
                        className="py-2 px-3 pl-14 border-t border-divider cursor-pointer"
                        onClick={(e) => { e.stopPropagation(); setExpandedMsgId(isMsgExpanded ? null : m.id); }}
                      >
                        <div className="flex justify-between">
                          <span className="font-bold text-[13px] text-text-primary truncate">{m.subject || "(no subject)"}</span>
                          <span className="text-[11px] text-text-dim shrink-0 ml-2 font-semibold">{relTime(m.date)}</span>
                        </div>
                        <div className="text-[12px] text-text-secondary mt-px truncate font-medium">{body.slice(0, 80) || ""}</div>
                        {isMsgExpanded && body && (
                          <div className="mt-2 px-2.5 py-2 bg-bg-tertiary rounded-[8px] text-[13px] leading-relaxed text-text-muted border border-divider whitespace-pre-wrap break-words">
                            {body}
                          </div>
                        )}
                      </div>
                    );
                  })}
                  {msgCount > 3 && (
                    <div className="py-2 px-3 pl-14">
                      <button
                        className="text-[13px] text-accent-amber font-bold cursor-pointer bg-transparent border-none"
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
