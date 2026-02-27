import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useData, useTabSearch, useTheme } from "../../../shared/context";
import {
  displayName,
  participantCount,
  participantEntries,
  relTime,
} from "../../../shared/lib";
import { PartitionedAvatar } from "../../../shared/components";

export const Route = createFileRoute("/_app/_tabs/circles")({
  component: CirclesList,
});

function CirclesList() {
  useTheme(); // subscribe to theme changes for avatar colors
  const navigate = useNavigate();
  const search = useTabSearch();
  const { conversations } = useData();

  const conns = conversations.filter((c) => c.classification === "connections");
  const circles = conns.filter((c) => participantCount(c) > 1);
  const q = search.toLowerCase();
  const filtered = circles.filter(
    (c) => !q || displayName(c).toLowerCase().includes(q)
  );

  return (
    <div className="flex flex-col gap-[5px] px-2.5 py-2">
      {filtered.length === 0 && (
        <div className="text-center py-15 px-5 text-text-muted text-[13px] font-semibold">No conversations yet</div>
      )}
      {filtered.map((c) => {
        const name = displayName(c);
        const hasUnread = c.unread_count > 0;
        const entries = participantEntries(c);

        return (
          <div
            key={c.id}
            className="card-row flex items-center px-3 py-2.5 cursor-pointer gap-2.5"
            onClick={() => navigate({ to: "/conversation/$id", params: { id: c.id } })}
          >
            <PartitionedAvatar participants={entries} />

            <div className="flex-1 min-w-0">
              <div className="flex justify-between items-baseline gap-2">
                <span className={`text-[13px] truncate flex-1 ${hasUnread ? "font-extrabold text-text-primary" : "font-semibold text-text-secondary"}`} style={{ letterSpacing: "-0.2px" }}>{name}</span>
                <span className={`text-[9px] shrink-0 ${hasUnread ? "font-extrabold text-text-primary" : "font-semibold text-text-dim"}`}>
                  {relTime(c.last_message_date)}
                </span>
              </div>
              <div className="flex justify-between items-center gap-2 mt-px">
                <span className="text-[10px] text-text-muted truncate flex-1 font-medium">
                  {c.last_message_preview || ""}
                </span>
                {hasUnread && (
                  <span className="min-w-4 h-4 rounded-lg bg-accent-purple text-white text-[9px] font-extrabold flex items-center justify-center px-1 shrink-0">
                    {c.unread_count}
                  </span>
                )}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
