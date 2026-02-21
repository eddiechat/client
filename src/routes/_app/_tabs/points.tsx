import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useRef, useState } from "react";
import { useAuth, useData, useTabSearch, useTheme } from "../../../shared/context";
import {
  displayName,
  participantCount,
  participantEmails,
  relTime,
} from "../../../shared/lib";
import { Avatar } from "../../../shared/components";
import { moveToLines } from "../../../tauri";

export const Route = createFileRoute("/_app/_tabs/points")({
  component: PointsList,
});

const SWIPE_THRESHOLD = 60;
const LONG_PRESS_MS = 500;

function PointsList() {
  useTheme(); // subscribe to theme changes for avatar colors
  const navigate = useNavigate();
  const search = useTabSearch();
  const { accountId } = useAuth();
  const { conversations, refresh } = useData();

  const [swipedId, setSwipedId] = useState<string | null>(null);
  const touchStartX = useRef(0);
  const touchDeltaX = useRef(0);
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const didLongPress = useRef(false);

  const conns = conversations.filter((c) => c.classification === "connections");
  const points = conns.filter((c) => participantCount(c) === 1);
  const q = search.toLowerCase();
  const filtered = points.filter(
    (c) => !q || displayName(c).toLowerCase().includes(q)
  );

  function clearLongPress() {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }

  async function handleMove(emails: string[]) {
    if (!accountId) return;
    await moveToLines(accountId, emails);
    setSwipedId(null);
    await refresh(accountId);
  }

  return (
    <ul className="list-none">
      {filtered.length === 0 && (
        <li className="text-center py-15 px-5 text-text-muted text-[13px]">No conversations yet</li>
      )}
      {filtered.map((c) => {
        const name = displayName(c);
        const hasUnread = c.unread_count > 0;
        const isOpen = swipedId === c.id;

        return (
          <li
            key={c.id}
            className="relative overflow-hidden border-b border-divider"
          >
            {/* Move button behind the row */}
            <div className="absolute inset-y-0 right-0 flex items-center">
              <button
                className="h-full px-5 bg-red-500 text-white text-[13px] font-semibold"
                onClick={(e) => {
                  e.stopPropagation();
                  handleMove(participantEmails(c));
                }}
              >
                Move
              </button>
            </div>

            {/* Sliding foreground row */}
            <div
              className="relative flex items-center px-5 py-3 cursor-pointer gap-3 transition-transform duration-200 bg-bg-primary"
              style={{ transform: isOpen ? "translateX(-72px)" : "translateX(0)" }}
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
              <div className="relative shrink-0">
                <Avatar name={name} email={participantEmails(c)[0]} size={11} fontSize="text-[15px]" />
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex justify-between items-baseline gap-2">
                  <span className={`text-[14px] truncate flex-1 ${hasUnread ? "font-semibold text-text-primary" : "font-normal text-text-secondary"}`}>{name}</span>
                  <span className={`text-[11px] shrink-0 ${hasUnread ? "text-accent-green font-semibold" : "text-text-dim"}`}>
                    {relTime(c.last_message_date)}
                  </span>
                </div>
                <div className="flex justify-between items-center gap-2 mt-0.5">
                  <span className="text-[12.5px] text-text-muted truncate flex-1 whitespace-nowrap overflow-hidden text-ellipsis">
                    {c.last_message_preview || ""}
                  </span>
                  {hasUnread && (
                    <span className="min-w-[20px] h-5 rounded-[10px] bg-accent-green text-white text-[10px] font-bold flex items-center justify-center px-1.5 shrink-0">
                      {c.unread_count}
                    </span>
                  )}
                </div>
              </div>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
