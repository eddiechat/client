import { useState, useEffect } from "react";
import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useData } from "../../shared/context";
import { fetchClusterThreads, fetchThreadMessages } from "../../tauri";
import type { Thread, Message } from "../../tauri";
import {
  relTime,
  fmtDate,
  lineEmoji,
  lineColor,
  avatarBg,
  avatarTextColor,
  initials,
} from "../../shared/lib";

export const Route = createFileRoute("/_app/cluster/$id")({
  component: ClusterView,
});

function ClusterView() {
  const { id } = Route.useParams();
  const router = useRouter();
  const { clusters } = useData();

  const [threads, setThreads] = useState<Thread[]>([]);
  const [loading, setLoading] = useState(false);
  const [expandedThreadId, setExpandedThreadId] = useState<string | null>(null);
  const [threadMessages, setThreadMessages] = useState<Record<string, Message[]>>({});
  const [expandedMsgId, setExpandedMsgId] = useState<string | null>(null);

  const cluster = clusters.find((c) => c.id === id);

  useEffect(() => {
    if (!cluster) return;
    setLoading(true);
    fetchClusterThreads(cluster.account_id, cluster.id)
      .then(setThreads)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [id, cluster]);

  async function toggleThread(t: Thread) {
    if (expandedThreadId === t.thread_id) {
      setExpandedThreadId(null);
      setExpandedMsgId(null);
      return;
    }
    setExpandedThreadId(t.thread_id);
    setExpandedMsgId(null);
    if (!threadMessages[t.thread_id] && cluster) {
      try {
        const msgs = await fetchThreadMessages(cluster.account_id, t.thread_id);
        setThreadMessages((prev) => ({ ...prev, [t.thread_id]: msgs }));
      } catch {
        /* ignore */
      }
    }
  }

  if (!cluster) {
    return (
      <div className="flex flex-col h-screen bg-bg-primary items-center justify-center text-text-muted">
        Cluster not found
      </div>
    );
  }

  const name = cluster.name;

  return (
    <div className="flex flex-col h-screen bg-bg-primary">
      {/* Header */}
      <div className="flex items-center gap-3 px-5 py-3 border-b border-divider shrink-0 bg-bg-secondary">
        <button className="border-none bg-transparent text-[22px] cursor-pointer text-accent-green p-0 leading-none" onClick={() => router.history.back()}>
          &#8249;
        </button>
        <div
          className="w-10 h-10 rounded-[10px] flex items-center justify-center text-[20px] shrink-0"
          style={{ background: `${lineColor(name)}20`, border: `1px solid ${lineColor(name)}40` }}
        >
          {lineEmoji(name)}
        </div>
        <div className="flex flex-col">
          <span className="font-semibold text-[15px] text-text-primary">{name}</span>
          <span className="text-[11px] text-text-muted">{threads.length} threads</span>
        </div>
      </div>

      {/* Thread list */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="text-center py-10 text-text-muted">Loading threads&hellip;</div>
        ) : (
          threads.map((t) => {
            const sender = t.from_name || t.from_address;
            const hasUnread = t.unread_count > 0;
            const isOpen = expandedThreadId === t.thread_id;
            const msgs = threadMessages[t.thread_id] || [];

            return (
              <div key={t.thread_id} className="border-b border-divider">
                {/* Thread row */}
                <div
                  className={`flex items-start gap-3 px-5 py-3 cursor-pointer ${isOpen ? "bg-bg-tertiary" : ""}`}
                  onClick={() => toggleThread(t)}
                >
                  {hasUnread && (
                    <div className="w-1.5 h-1.5 rounded-full bg-accent-green shrink-0 mt-4" />
                  )}
                  <div
                    className="w-9 h-9 rounded-[36%] flex items-center justify-center font-bold text-xs shrink-0 mt-0.5"
                    style={{ background: avatarBg(sender), color: avatarTextColor(sender) }}
                  >
                    {initials(sender)}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex justify-between items-baseline gap-2">
                      <span className={`text-[13px] truncate ${hasUnread ? "font-bold text-text-primary" : "font-semibold text-text-secondary"}`}>
                        {sender}
                      </span>
                      <span className={`text-[10px] shrink-0 ${hasUnread ? "text-accent-green font-semibold" : "text-text-dim"}`}>{relTime(t.last_activity)}</span>
                    </div>
                    <div className="flex justify-between items-center gap-2 mt-0.5">
                      <span className={`text-[12px] truncate flex-1 ${hasUnread ? "text-text-primary" : "text-text-secondary"}`}>
                        {t.subject || "(no subject)"}
                      </span>
                      {t.message_count > 1 && (
                        <span className="text-[10px] text-text-dim shrink-0">({t.message_count})</span>
                      )}
                    </div>
                    {!isOpen && t.preview && (
                      <div className="text-[12px] text-text-muted mt-0.5 truncate">{t.preview}</div>
                    )}
                  </div>
                </div>

                {/* Expanded thread — show messages */}
                {isOpen && (
                  <div className="bg-bg-secondary">
                    {msgs.map((m) => {
                      const msgSender = m.from_name || m.from_address;
                      const body = m.distilled_text || m.body_text || "";
                      const isMsgOpen = expandedMsgId === m.id;
                      const isUnread = !m.imap_flags.includes("Seen");

                      return (
                        <div
                          key={m.id}
                          className="border-t border-divider cursor-pointer"
                          onClick={(e) => { e.stopPropagation(); setExpandedMsgId(isMsgOpen ? null : m.id); }}
                        >
                          <div className={`flex items-start gap-2.5 px-5 py-2.5 pl-16 ${isMsgOpen ? "bg-bg-tertiary" : ""}`}>
                            {isUnread && (
                              <div className="w-1.5 h-1.5 rounded-full bg-accent-green shrink-0 mt-3" />
                            )}
                            <div className="flex-1 min-w-0">
                              <div className="flex justify-between items-baseline gap-2">
                                <span className={`text-[12px] truncate ${isUnread ? "font-bold text-text-primary" : "font-semibold text-text-secondary"}`}>
                                  {m.is_sent ? `To: ${m.to_addresses.split(",")[0]}` : msgSender}
                                </span>
                                <span className={`text-[10px] shrink-0 ${isUnread ? "text-accent-green font-semibold" : "text-text-dim"}`}>{relTime(m.date)}</span>
                              </div>
                              {!isMsgOpen && body && (
                                <div className="text-[11px] text-text-muted mt-0.5 truncate">{body.slice(0, 100)}</div>
                              )}
                            </div>
                            {m.has_attachments && (
                              <span className="text-xs shrink-0 mt-0.5">{"\uD83D\uDCCE"}</span>
                            )}
                          </div>

                          {isMsgOpen && (
                            <div className="px-5 pb-4 pl-16">
                              <div className="py-2 pb-3 border-b border-divider mb-3">
                                <div className="flex gap-2 py-0.5 text-[12px] leading-snug">
                                  <span className="text-text-dim min-w-14 shrink-0 text-right">From</span>
                                  <span className="text-text-primary break-all">
                                    {m.from_name ? `${m.from_name} <${m.from_address}>` : m.from_address}
                                  </span>
                                </div>
                                <div className="flex gap-2 py-0.5 text-[12px] leading-snug">
                                  <span className="text-text-dim min-w-14 shrink-0 text-right">To</span>
                                  <span className="text-text-primary break-all">{m.to_addresses}</span>
                                </div>
                                {m.cc_addresses && (
                                  <div className="flex gap-2 py-0.5 text-[12px] leading-snug">
                                    <span className="text-text-dim min-w-14 shrink-0 text-right">Cc</span>
                                    <span className="text-text-primary break-all">{m.cc_addresses}</span>
                                  </div>
                                )}
                                <div className="flex gap-2 py-0.5 text-[12px] leading-snug">
                                  <span className="text-text-dim min-w-14 shrink-0 text-right">Date</span>
                                  <span className="text-text-primary break-all">{fmtDate(m.date)}</span>
                                </div>
                              </div>
                              <div className="text-[13px] leading-relaxed text-text-secondary whitespace-pre-wrap break-words">{body}</div>
                            </div>
                          )}
                        </div>
                      );
                    })}
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
