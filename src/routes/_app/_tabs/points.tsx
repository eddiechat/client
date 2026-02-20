import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useData, useTabSearch } from "../../../shared/context";
import {
  displayName,
  participantCount,
  relTime,
  avatarBg,
  avatarTextColor,
  initials,
} from "../../../shared/lib";

export const Route = createFileRoute("/_app/_tabs/points")({
  component: PointsList,
});

function PointsList() {
  const navigate = useNavigate();
  const search = useTabSearch();
  const { conversations } = useData();

  const conns = conversations.filter((c) => c.classification === "connections");
  const points = conns.filter((c) => participantCount(c) === 1);
  const q = search.toLowerCase();
  const filtered = points.filter(
    (c) => !q || displayName(c).toLowerCase().includes(q)
  );

  return (
    <ul className="list-none">
      {filtered.length === 0 && (
        <li className="text-center py-15 px-5 text-text-muted text-[13px]">No conversations yet</li>
      )}
      {filtered.map((c) => {
        const name = displayName(c);
        const hasUnread = c.unread_count > 0;

        return (
          <li
            key={c.id}
            className="flex items-center px-5 py-3 border-b border-divider cursor-pointer gap-3 transition-colors"
            onClick={() => navigate({ to: "/conversation/$id", params: { id: c.id } })}
          >
            <div className="relative shrink-0">
              <div
                className="w-11 h-11 rounded-[36%] flex items-center justify-center font-bold text-[15px]"
                style={{ background: avatarBg(name), color: avatarTextColor(name) }}
              >
                {initials(name)}
              </div>
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
          </li>
        );
      })}
    </ul>
  );
}
