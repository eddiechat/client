import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useRef, useState } from "react";
import { useAuth, useData, useTabSearch, useTheme, useChatFilter } from "../../../shared/context";
import {
  displayName,
  participantCount,
  participantEntries,
  participantEmails,
  relTime,
  avatarGroupPalette,
  storeConversationColors,
  previewPrefix,
} from "../../../shared/lib";
import { Avatar, PartitionedAvatar } from "../../../shared/components";
import { moveToPoints, blockEntities } from "../../../tauri";

export const Route = createFileRoute("/_app/_tabs/requests")({
  component: RequestsList,
});

const SWIPE_THRESHOLD = 60;
const LONG_PRESS_MS = 500;

function RequestsList() {
  useTheme(); // subscribe to theme changes for avatar colors
  const navigate = useNavigate();
  const search = useTabSearch();
  const chatFilter = useChatFilter();
  const { accountId } = useAuth();
  const { conversations, refresh } = useData();

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const touchStartX = useRef(0);
  const touchDeltaX = useRef(0);
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const didLongPress = useRef(false);

  const inSelectMode = selectedIds.size > 0;

  function toggleSelected(id: string) {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  const reqs = conversations.filter(
    (c) => c.classification === "others" && participantEmails(c).length > 0 && displayName(c).trim().length > 0
  );
  const q = search.toLowerCase();
  const base = q
    ? reqs
    : chatFilter === "1:1"
      ? reqs.filter((c) => participantCount(c) === 1)
      : chatFilter === "3+"
        ? reqs.filter((c) => participantCount(c) > 1)
        : reqs;
  const filtered = base.filter(
    (c) => !q || displayName(c).toLowerCase().includes(q)
  );

  function emailsForConversation(c: (typeof filtered)[0]): string[] {
    return participantCount(c) > 1 && c.initial_sender_email
      ? [c.initial_sender_email]
      : participantEmails(c);
  }

  function allSelectedEmails(): string[] {
    const set = new Set<string>();
    for (const c of filtered) {
      if (selectedIds.has(c.id)) {
        for (const e of emailsForConversation(c)) set.add(e);
      }
    }
    return [...set];
  }

  async function handleMove() {
    if (!accountId) return;
    const emails = allSelectedEmails();
    if (emails.length === 0) return;
    await moveToPoints(accountId, emails);
    setConfirmDeleteId(null);
    setSelectedIds(new Set());
    await refresh(accountId);
  }

  async function handleBlock() {
    if (!accountId) return;
    if (!confirmDeleteId) {
      setConfirmDeleteId("all");
      return;
    }
    const emails = allSelectedEmails();
    if (emails.length === 0) return;
    await blockEntities(accountId, emails);
    setConfirmDeleteId(null);
    setSelectedIds(new Set());
    await refresh(accountId);
  }

  function clearLongPress() {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }

  return (
    <div className="flex flex-col gap-[5.5px] px-2.75 py-2.25 select-none">
      <div
        className="card-row flex items-center px-3.25 py-1.5 rounded-xl cursor-pointer gap-1.5"
        style={{ opacity: 0.5 }}
        onClick={() => navigate({ to: "/points" })}
      >
        <span className="text-[13px] text-text-dim">‹</span>
        <span className="text-[13px] font-semibold text-text-muted" style={{ letterSpacing: "-0.1px" }}>
          Back to chats
        </span>
      </div>
      {filtered.length === 0 && (
        <div className="text-center py-15 px-5 text-text-muted text-[16px] font-semibold">No requests yet</div>
      )}
      {filtered.map((c) => {
        const name = displayName(c);
        const hasUnread = c.unread_count > 0;
        const isSelected = selectedIds.has(c.id);

        return (
          <div
            key={c.id}
            className="relative overflow-hidden rounded-2xl"
          >
            {/* Action buttons behind the row */}
            <div className="absolute inset-y-0 right-0 flex items-center">
              <button
                className="h-full px-4 bg-green-600 text-white text-[14px] font-bold"
                onClick={(e) => {
                  e.stopPropagation();
                  handleMove();
                }}
              >
                Accept
              </button>
              <button
                className={`h-full px-4 text-white text-[14px] font-bold rounded-r-2xl ${confirmDeleteId ? "bg-red-700" : "bg-accent-red"}`}
                onClick={(e) => {
                  e.stopPropagation();
                  handleBlock();
                }}
              >
                {confirmDeleteId ? "Sure?" : "Block"}
              </button>
            </div>

            {/* Sliding foreground row */}
            <div
              className="relative card-row flex items-center px-3.25 py-3.25 cursor-pointer gap-3.25 transition-transform duration-200"
              style={{ transform: isSelected ? "translateX(-140px)" : "translateX(0)" }}
              onTouchStart={(e) => {
                touchStartX.current = e.touches[0].clientX;
                touchDeltaX.current = 0;
              }}
              onTouchMove={(e) => {
                touchDeltaX.current = e.touches[0].clientX - touchStartX.current;
              }}
              onTouchEnd={() => {
                if (touchDeltaX.current < -SWIPE_THRESHOLD) {
                  if (!selectedIds.has(c.id)) toggleSelected(c.id);
                } else if (touchDeltaX.current > SWIPE_THRESHOLD) {
                  if (selectedIds.has(c.id)) toggleSelected(c.id);
                }
              }}
              onPointerDown={() => {
                didLongPress.current = false;
                longPressTimer.current = setTimeout(() => {
                  didLongPress.current = true;
                  toggleSelected(c.id);
                }, LONG_PRESS_MS);
              }}
              onPointerUp={clearLongPress}
              onPointerCancel={clearLongPress}
              onPointerLeave={clearLongPress}
              onContextMenu={(e) => e.preventDefault()}
              onClick={() => {
                if (didLongPress.current) {
                  didLongPress.current = false;
                  return;
                }
                if (inSelectMode) {
                  toggleSelected(c.id);
                  setConfirmDeleteId(null);
                } else {
                  navigate({ to: "/conversation/$id", params: { id: c.id } });
                }
              }}
            >
              {participantCount(c) > 1 ? (
                <PartitionedAvatar participants={participantEntries(c)} sizePx={54} conversationId={c.id} />
              ) : (
                <div className="relative shrink-0">
                  <Avatar name={name} email={participantEmails(c)[0]} size={13} fontSize="text-[17px]" color={(() => {
                    const email = participantEmails(c)[0];
                    const p = avatarGroupPalette(name.split("").reduce((a, ch) => a + ch.charCodeAt(0), 0));
                    if (email) storeConversationColors(c.id, p, [[email, name]]);
                    return p[0];
                  })()}/>
                </div>
              )}

              <div className="flex-1 min-w-0">
                <div className="flex justify-between items-baseline gap-2">
                  <span className={`text-[16px] truncate flex-1 ${hasUnread ? "font-extrabold text-text-primary" : "font-semibold text-text-secondary"}`} style={{ letterSpacing: "-0.2px" }}>{name}</span>
                  <span className={`text-[11px] shrink-0 ${hasUnread ? "font-extrabold text-text-primary" : "font-semibold text-text-dim"}`}>
                    {relTime(c.last_message_date)}
                  </span>
                </div>
                <div className="flex justify-between items-center gap-2 mt-px">
                  <span className="text-[12px] text-text-muted truncate flex-1 font-medium">
                    {previewPrefix(c)}
                  </span>
                  {hasUnread && (
                    <span className="min-w-4.5 h-4.5 rounded-[9px] bg-accent-purple text-white text-[11px] font-extrabold flex items-center justify-center px-1 shrink-0">
                      {c.unread_count}
                    </span>
                  )}
                </div>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
