import { useState, useEffect, useRef } from "react";
import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useAuth, useTheme } from "../../shared/context";
import { useData } from "../../shared/context";
import { fetchConversationMessages } from "../../tauri";
import type { Message } from "../../tauri";
import {
  displayName,
  participantCount,
  participantEmails,
  dedup,
  fmtTime,
  avatarBg,
} from "../../shared/lib";
import { Avatar } from "../../shared/components";

export const Route = createFileRoute("/_app/conversation/$id")({
  component: ConversationView,
});

function ConversationView() {
  useTheme(); // subscribe to theme changes for avatar colors
  const { id } = Route.useParams();
  const router = useRouter();
  const { myAddrs } = useAuth();
  const { conversations } = useData();

  const [messages, setMessages] = useState<Message[]>([]);
  const [messagesLoading, setMessagesLoading] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  const conversation = conversations.find((c) => c.id === id);

  useEffect(() => {
    if (!conversation) return;
    setMessagesLoading(true);
    fetchConversationMessages(conversation.account_id, conversation.id)
      .then(setMessages)
      .catch(() => {})
      .finally(() => setMessagesLoading(false));
  }, [id, conversation]);

  useEffect(() => {
    if (!messagesLoading && messages.length > 0) {
      bottomRef.current?.scrollIntoView();
    }
  }, [messagesLoading, messages]);

  if (!conversation) {
    return (
      <div className="flex flex-col h-screen bg-bg-primary items-center justify-center text-text-muted">
        Conversation not found
      </div>
    );
  }

  const name = displayName(conversation);
  const totalCount = conversation.total_count;
  const isMultiParticipant = participantCount(conversation) > 1;
  const participantMap: Record<string, string> = (() => {
    if (!conversation.participant_names) return {};
    try {
      return JSON.parse(conversation.participant_names) as Record<string, string>;
    } catch { return {}; }
  })();
  const oldestYear =
    messages.length > 0
      ? new Date(Math.min(...messages.map((m) => m.date))).getFullYear()
      : null;

  return (
    <div className="flex flex-col h-screen bg-bg-primary">
      {/* Header */}
      <div className="flex items-center gap-3 px-5 pb-3 border-b border-divider shrink-0 bg-bg-secondary" style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))' }}>
        <button className="border-none bg-transparent text-[24px] cursor-pointer text-accent-green p-0 leading-none" onClick={() => router.history.back()}>
          &#8249;
        </button>
        <Avatar name={name} email={participantEmails(conversation)[0]} size={11} fontSize="text-[15px]" className="shrink-0" />
        <div className="flex flex-col">
          <span className="font-semibold text-[17px] text-text-primary leading-tight">{name}</span>
          <span className="text-[12px] text-text-muted leading-tight">{participantEmails(conversation).join(", ")}</span>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-4 flex flex-col gap-1">
        <div className="self-center bg-bg-tertiary text-text-muted text-[13px] px-4 py-1 rounded-xl mb-4 border border-divider">
          Derived from {totalCount} emails{oldestYear ? ` since ${oldestYear}` : ""}
        </div>

        {messagesLoading ? (
          <div className="text-center py-10 text-text-muted">Loading messages&hellip;</div>
        ) : (
          dedup(messages).sort((a, b) => a.date - b.date).map((m, i, arr) => {
            const isSent = m.is_sent;
            const body = m.distilled_text || m.body_text || m.subject || "";
            const sender = m.from_name || m.from_address;
            const year = new Date(m.date).getFullYear();
            const prevYear = i > 0 ? new Date(arr[i - 1].date).getFullYear() : year;
            const missing = isMultiParticipant ? (() => {
              try {
                const msgAddrs = new Set([
                  m.from_address.toLowerCase(),
                  ...(JSON.parse(m.to_addresses || "[]") as string[]).map(e => e.toLowerCase()),
                  ...(JSON.parse(m.cc_addresses || "[]") as string[]).map(e => e.toLowerCase()),
                ]);
                return Object.entries(participantMap)
                  .filter(([email]) => !msgAddrs.has(email.toLowerCase()) && !myAddrs.has(email.toLowerCase()))
                  .map(([, n]) => (n || "").split(" ")[0])
                  .filter(Boolean);
              } catch { return []; }
            })() : [];
            return (
              <div key={m.id} className={`flex flex-col mb-0.5 ${isSent ? "items-end" : "items-start"}`}>
                {i > 0 && year !== prevYear && (
                  <div className="self-center text-text-dim text-[13px] px-4 py-1 rounded-xl my-3 border border-text-dim">
                    {year}
                  </div>
                )}
                {!isSent && isMultiParticipant && (
                  <span className="text-[12px] mb-0.5 px-1 ml-9">
                    <span className="font-bold" style={{ color: avatarBg(sender) }}>{sender}</span>
                    {missing.length > 0 && (
                      <span className="text-text-muted line-through ml-1">{missing.join(", ")}</span>
                    )}
                  </span>
                )}
                {isSent && isMultiParticipant && missing.length > 0 && (
                  <span className="text-[12px] text-text-muted line-through mb-0.5 px-1">{missing.join(", ")}</span>
                )}
                <div className={`flex items-end gap-2 ${isSent ? "flex-row-reverse" : ""} max-w-[85%]`}>
                  {!isSent && isMultiParticipant && (
                    <Avatar name={sender} email={m.from_address} size={8} fontSize="text-[12px]" className="shrink-0" />
                  )}
                  <div className={`px-3.5 py-2.5 text-[16px] leading-snug break-words ${isSent
                    ? "bg-accent-green text-white rounded-[18px_18px_4px_18px]"
                    : "bg-bg-secondary text-text-primary rounded-[18px_18px_18px_4px] border border-divider"
                    }`}>
                    {body}
                  </div>
                </div>
                <span className="text-[11px] text-text-dim px-1 pt-0.5">{fmtTime(m.date)}</span>
              </div>
            );
          })
        )}
        <div ref={bottomRef} />
      </div>

      {/* Compose */}
      <div className="flex items-center gap-2.5 px-4 pt-2.5 border-t border-divider shrink-0 bg-bg-secondary" style={{ paddingBottom: 'calc(0.875rem + env(safe-area-inset-bottom, 0px))' }}>
        <button className="w-10 h-10 rounded-xl border border-divider bg-transparent text-xl text-text-dim cursor-pointer flex items-center justify-center shrink-0 leading-none hover:border-accent-green hover:text-accent-green">+</button>
        <input
          className="flex-1 py-2 px-3.5 border border-divider rounded-xl text-[16px] outline-none bg-bg-primary text-text-primary placeholder:text-text-dim focus:border-accent-green"
          placeholder={"Message\u2026"}
        />
        <button className="w-10 h-10 rounded-xl border-none bg-accent-green text-white text-lg font-bold cursor-pointer flex items-center justify-center shrink-0 hover:brightness-90 transition">{"\u2191"}</button>
      </div>
    </div>
  );
}
