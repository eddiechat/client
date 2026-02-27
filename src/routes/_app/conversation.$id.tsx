import { useState, useEffect, useRef } from "react";
import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useAuth, useTheme } from "../../shared/context";
import { useData } from "../../shared/context";
import { fetchConversationMessages } from "../../tauri";
import type { Message } from "../../tauri";
import {
  displayName,
  firstName,
  participantCount,
  participantEmails,
  dedup,
  fmtTime,
  avatarBg,
} from "../../shared/lib";
import { Avatar, MessageDetail } from "../../shared/components";

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
  const [selectedMessage, setSelectedMessage] = useState<Message | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const savedScrollRef = useRef<number>(0);

  const conversation = conversations.find((c) => c.id === id);

  useEffect(() => {
    if (!conversation) return;
    setMessagesLoading(true);
    setSelectedMessage(null);
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

  // Restore scroll position when returning from detail view
  useEffect(() => {
    if (!selectedMessage && scrollRef.current && savedScrollRef.current > 0) {
      scrollRef.current.scrollTop = savedScrollRef.current;
    }
  }, [selectedMessage]);

  if (!conversation) {
    return (
      <div className="flex flex-col h-screen bg-bg-primary items-center justify-center text-text-muted font-semibold">
        Conversation not found
      </div>
    );
  }

  if (selectedMessage) {
    return (
      <MessageDetail
        message={selectedMessage}
        onBack={() => setSelectedMessage(null)}
      />
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

  const handleSelectMessage = (m: Message) => {
    if (scrollRef.current) {
      savedScrollRef.current = scrollRef.current.scrollTop;
    }
    setSelectedMessage(m);
  };

  return (
    <div className="flex flex-col h-screen bg-bg-primary">
      {/* Header */}
      <div className="flex items-center gap-3 px-4 pb-3 border-b border-divider shrink-0 bg-bg-primary" style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))' }}>
        <button className="border-none bg-transparent text-[28px] cursor-pointer text-text-muted min-w-10 min-h-10 flex items-center justify-center -ml-1 font-bold" onClick={() => router.history.back()}>
          &#8249;
        </button>
        <Avatar name={name} email={participantEmails(conversation)[0]} size={10} fontSize="text-[13px]" className="shrink-0" />
        <div className="flex flex-col min-w-0">
          <span className="font-extrabold text-[13px] text-text-primary leading-tight truncate" style={{ letterSpacing: "-0.2px" }}>{name}</span>
          <span className="text-[10px] text-text-muted leading-tight font-medium truncate">{participantEmails(conversation).join(", ")}</span>
        </div>
      </div>

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-3 py-4 flex flex-col gap-1">
        <div className="self-center bg-bg-tertiary text-text-muted text-[11px] font-semibold px-3 py-1 rounded-[10px] mb-4 border border-divider">
          Derived from {totalCount} emails{oldestYear ? ` since ${oldestYear}` : ""}
        </div>

        {messagesLoading ? (
          <div className="text-center py-10 text-text-muted font-semibold text-[13px]">Loading messages&hellip;</div>
        ) : (
          dedup(messages).sort((a, b) => a.date - b.date).map((m, i, arr) => {
            const isSent = m.is_sent;
            const body = m.distilled_text || m.body_text || m.subject || "";
            const sender = firstName(m.from_name || m.from_address);
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
              <div
                key={m.id}
                className={`flex flex-col mb-0.5 ${isSent ? "items-end" : "items-start"} cursor-pointer`}
                onClick={() => handleSelectMessage(m)}
              >
                {i > 0 && year !== prevYear && (
                  <div className="self-center text-text-dim text-[11px] font-semibold px-3 py-1 rounded-[10px] my-3 border border-divider">
                    {year}
                  </div>
                )}
                {!isSent && isMultiParticipant && (
                  <span className="text-[9px] font-bold mb-0.5 px-1 ml-8">
                    <span style={{ color: avatarBg(sender) }}>{sender}</span>
                    {missing.length > 0 && (
                      <span className="text-text-muted line-through ml-1">{missing.join(", ")}</span>
                    )}
                  </span>
                )}
                {isSent && isMultiParticipant && missing.length > 0 && (
                  <span className="text-[9px] text-text-muted line-through mb-0.5 px-1 font-semibold">{missing.join(", ")}</span>
                )}
                <div className={`flex items-end gap-2 ${isSent ? "flex-row-reverse" : ""} max-w-[85%]`}>
                  {!isSent && isMultiParticipant && (
                    <Avatar name={sender} email={m.from_address} size={7} fontSize="text-[10px]" className="shrink-0" />
                  )}
                  <div className={`min-w-0 px-3 py-2 text-[11px] font-medium leading-snug break-words ${isSent
                    ? "bg-accent-green text-white rounded-[12px_12px_4px_12px]"
                    : "bg-bg-secondary text-text-primary rounded-[12px_12px_12px_4px]"
                    }`}
                    style={isSent ? undefined : { boxShadow: "0 1px 4px rgba(0,0,0,0.07)" }}
                  >
                    {body}
                  </div>
                </div>
                <span className="text-[9px] text-text-dim px-1 pt-0.5 font-medium">{fmtTime(m.date)}</span>
              </div>
            );
          })
        )}
        <div ref={bottomRef} />
      </div>

      {/* Compose */}
      <div className="flex items-center gap-2 px-3 pt-2 border-t border-divider shrink-0 bg-bg-secondary" style={{ paddingBottom: 'calc(0.625rem + env(safe-area-inset-bottom, 0px))' }}>
        <button className="w-8 h-8 rounded-[10px] border border-divider bg-bg-primary text-lg text-text-dim cursor-pointer flex items-center justify-center shrink-0 leading-none font-light hover:border-accent-green hover:text-accent-green transition-colors">+</button>
        <input
          className="flex-1 py-2 px-3 border border-divider rounded-[10px] text-[13px] font-medium outline-none bg-bg-primary text-text-primary placeholder:text-text-dim focus:border-accent-green"
          placeholder={"Message\u2026"}
        />
        <button className="w-8 h-8 rounded-[10px] border-none bg-accent-green text-white text-sm font-extrabold cursor-pointer flex items-center justify-center shrink-0 hover:brightness-90 transition">{"\u2191"}</button>
      </div>
    </div>
  );
}
