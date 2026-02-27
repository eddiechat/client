import { useState, useEffect } from "react";
import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useData, useTheme } from "../../shared/context";
import { fetchClusterMessages } from "../../tauri";
import type { Message } from "../../tauri";
import {
  relTime,
  fmtDate,
  dedup,
  lineEmoji,
  lineColor,
} from "../../shared/lib";
import { Avatar } from "../../shared/components";

export const Route = createFileRoute("/_app/cluster/$id")({
  component: ClusterView,
});

function ClusterView() {
  useTheme(); // subscribe to theme changes for avatar colors
  const { id } = Route.useParams();
  const router = useRouter();
  const { clusters } = useData();

  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [expandedMsgId, setExpandedMsgId] = useState<string | null>(null);

  const cluster = clusters.find((c) => c.id === id);

  useEffect(() => {
    if (!cluster) return;
    setLoading(true);
    fetchClusterMessages(cluster.account_id, cluster.id)
      .then(setMessages)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [id, cluster]);

  if (!cluster) {
    return (
      <div className="flex flex-col h-screen items-center justify-center text-text-muted font-semibold" style={{ background: "var(--color-bg-gradient)" }}>
        Cluster not found
      </div>
    );
  }

  const name = cluster.name;
  const sorted = dedup([...messages]).sort((a, b) => b.date - a.date);

  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-gradient)" }}>
      {/* Header */}
      <div className="flex items-center gap-3 px-4 pb-3 border-b border-divider shrink-0" style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))' }}>
        <button className="border-none bg-transparent text-[28px] cursor-pointer text-text-muted min-w-10 min-h-10 flex items-center justify-center -ml-1 font-bold" onClick={() => router.history.back()}>
          &#8249;
        </button>
        <div
          className="w-9 h-9 rounded-[11px] flex items-center justify-center text-[17px] shrink-0"
          style={{ background: `${lineColor(name)}20`, border: `1px solid ${lineColor(name)}40` }}
        >
          {lineEmoji(name)}
        </div>
        <div className="flex flex-col min-w-0">
          <span className="font-extrabold text-[13px] text-text-primary truncate" style={{ letterSpacing: "-0.2px" }}>{name}</span>
          <span className="text-[10px] text-text-muted font-medium">{sorted.length} messages</span>
        </div>
      </div>

      {/* Message list */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="text-center py-10 text-text-muted font-semibold text-[13px]">Loading messages&hellip;</div>
        ) : (
          sorted.map((m) => {
            const isOpen = expandedMsgId === m.id;
            const sender = m.from_name || m.from_address;
            const body = m.distilled_text || m.body_text || "";
            const isUnread = !m.imap_flags.includes("Seen");

            return (
              <div
                key={m.id}
                className="border-b border-divider cursor-pointer"
                onClick={() => setExpandedMsgId(isOpen ? null : m.id)}
              >
                {/* Collapsed row */}
                <div className={`flex items-start gap-2.5 px-4 py-2.5 ${isOpen ? "bg-bg-tertiary" : ""}`}>
                  {isUnread && (
                    <div className="w-1.5 h-1.5 rounded-full bg-accent-amber shrink-0 mt-3.5" />
                  )}
                  <Avatar name={sender} email={m.from_address} size={9} fontSize="text-[12px]" className="shrink-0 mt-0.5" />
                  <div className="flex-1 min-w-0">
                    <div className="flex justify-between items-baseline gap-2">
                      <span className={`text-[12px] truncate ${isUnread ? "font-extrabold text-text-primary" : "font-semibold text-text-secondary"}`} style={{ letterSpacing: "-0.2px" }}>
                        {m.is_sent ? `To: ${m.to_addresses.split(",")[0]}` : sender}
                      </span>
                      <span className={`text-[9px] shrink-0 font-semibold ${isUnread ? "text-accent-amber" : "text-text-dim"}`}>{relTime(m.date)}</span>
                    </div>
                    <div className="text-[11px] text-text-secondary mt-0.5 truncate font-medium">
                      {m.subject || "(no subject)"}
                    </div>
                    {!isOpen && body && (
                      <div className="text-[10px] text-text-muted mt-0.5 truncate font-medium">
                        {body.slice(0, 120)}
                      </div>
                    )}
                  </div>
                  {m.has_attachments && (
                    <span className="text-xs shrink-0 mt-1">{"\uD83D\uDCCE"}</span>
                  )}
                </div>

                {/* Expanded detail */}
                {isOpen && (
                  <div className="px-4 pb-4 pl-14">
                    <div className="py-2 pb-3 border-b border-divider mb-3">
                      <div className="flex gap-2 py-0.5 text-[11px] leading-snug font-medium">
                        <span className="text-text-dim min-w-12 shrink-0 text-right">From</span>
                        <span className="text-text-primary break-all">
                          {m.from_name ? `${m.from_name} <${m.from_address}>` : m.from_address}
                        </span>
                      </div>
                      <div className="flex gap-2 py-0.5 text-[11px] leading-snug font-medium">
                        <span className="text-text-dim min-w-12 shrink-0 text-right">To</span>
                        <span className="text-text-primary break-all">{m.to_addresses}</span>
                      </div>
                      {m.cc_addresses && (
                        <div className="flex gap-2 py-0.5 text-[11px] leading-snug font-medium">
                          <span className="text-text-dim min-w-12 shrink-0 text-right">Cc</span>
                          <span className="text-text-primary break-all">{m.cc_addresses}</span>
                        </div>
                      )}
                      <div className="flex gap-2 py-0.5 text-[11px] leading-snug font-medium">
                        <span className="text-text-dim min-w-12 shrink-0 text-right">Date</span>
                        <span className="text-text-primary break-all">{fmtDate(m.date)}</span>
                      </div>
                    </div>
                    <div className="text-[12px] leading-relaxed text-text-secondary whitespace-pre-wrap break-words font-medium">{body}</div>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
