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

  const [swipedId, setSwipedId] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const touchStartX = useRef(0);
  const touchDeltaX = useRef(0);
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const didLongPress = useRef(false);

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

  async function handleMove(emails: string[]) {
    if (!accountId) return;
    await moveToPoints(accountId, emails);
    setConfirmDeleteId(null);
    setSwipedId(null);
    await refresh(accountId);
  }

  async function handleDelete(id: string, emails: string[]) {
    if (!accountId) return;
    if (confirmDeleteId !== id) {
      setConfirmDeleteId(id);
      return;
    }
    await blockEntities(accountId, emails);
    setConfirmDeleteId(null);
    setSwipedId(null);
    await refresh(accountId);
  }

  function clearLongPress() {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }

  return (
    <div className="flex flex-col gap-[5.5px] px-2.75 py-2.25">
      {filtered.length === 0 && (
        <div className="text-center py-15 px-5 text-text-muted text-[16px] font-semibold">No requests yet</div>
      )}
      {filtered.map((c) => {
        const name = displayName(c);
        const hasUnread = c.unread_count > 0;
        const isOpen = swipedId === c.id;

        return (
          <div
            key={c.id}
            className="relative overflow-hidden rounded-2xl"
          >
            {/* Action buttons behind the row */}
            <div className="absolute inset-y-0 right-0 flex items-center">
              <button
                className="h-full px-4 bg-accent-green text-white text-[14px] font-bold"
                onClick={(e) => {
                  e.stopPropagation();
                  handleMove(participantEmails(c));
                }}
              >
                Move
              </button>
              <button
                className={`h-full px-4 text-white text-[14px] font-bold rounded-r-2xl ${confirmDeleteId === c.id ? "bg-red-700" : "bg-accent-red"}`}
                onClick={(e) => {
                  e.stopPropagation();
                  handleDelete(c.id, participantEmails(c));
                }}
              >
                {confirmDeleteId === c.id ? "Sure?" : "Delete"}
              </button>
            </div>

            {/* Sliding foreground row */}
            <div
              className="relative card-row flex items-center px-3.25 py-3.25 cursor-pointer gap-3.25 transition-transform duration-200"
              style={{ transform: isOpen ? "translateX(-140px)" : "translateX(0)" }}
              onTouchStart={(e) => {
                touchStartX.current = e.touches[0].clientX;
                touchDeltaX.current = 0;
              }}
              onTouchMove={(e) => {
                touchDeltaX.current = e.touches[0].clientX - touchStartX.current;
              }}
              onTouchEnd={() => {
                if (touchDeltaX.current < -SWIPE_THRESHOLD) {
                  setSwipedId(c.id);
                } else if (touchDeltaX.current > SWIPE_THRESHOLD) {
                  setSwipedId(null);
                }
              }}
              onPointerDown={() => {
                didLongPress.current = false;
                longPressTimer.current = setTimeout(() => {
                  didLongPress.current = true;
                  setSwipedId((prev) => (prev === c.id ? null : c.id));
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
                if (isOpen) {
                  setSwipedId(null);
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
