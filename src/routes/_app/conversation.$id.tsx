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
  participantEntries,
  participantEmails,
  dedup,
  fmtTime,
  avatarGroupPalette,
} from "../../shared/lib";
import { Avatar, PartitionedAvatar, MessageDetail } from "../../shared/components";

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
      <div className="flex flex-col h-screen items-center justify-center text-text-muted font-semibold" style={{ background: "var(--color-bg-gradient)" }}>
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
  const emails = participantEmails(conversation);
  // Build email→color map locally using same logic as PartitionedAvatar
  const entries = participantEntries(conversation);
  const gHash = entries.reduce((acc, [email, n]) => acc + (n || email).split("").reduce((a, c) => a + c.charCodeAt(0), 0), 0);
  const palette = avatarGroupPalette(gHash);
  const emailColorMap = new Map<string, string>();
  entries.forEach(([email], i) => {
    emailColorMap.set(email.toLowerCase(), palette[i % palette.length]);
  });
  const colorOf = (email: string): string | undefined =>
    emailColorMap.get(email.toLowerCase());
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
    <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-gradient)" }}>
      {/* Header */}
      <div
        className="flex items-center gap-3 px-4 pb-3 shrink-0 bg-bg-secondary"
        style={{
          paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))',
          boxShadow: '0 2px 12px rgba(0,0,0,0.06)',
        }}
      >
        <button
          className="border-none bg-transparent text-[26px] cursor-pointer text-text-secondary w-10 h-10 flex items-center justify-center rounded-[10px] hover:bg-bg-tertiary active:scale-95 transition-all font-bold -ml-1"
          onClick={() => router.history.back()}
        >
          &#8249;
        </button>
        {isMultiParticipant ? (
          <PartitionedAvatar participants={participantEntries(conversation)} sizePx={50} conversationId={conversation.id} />
        ) : (
          <Avatar name={name} email={emails[0]} size={12} fontSize="text-[16px]" className="shrink-0" color={colorOf(emails[0])} />
        )}
        <div className="flex flex-col min-w-0">
          <span className="font-extrabold text-[16px] text-text-primary leading-tight truncate" style={{ letterSpacing: "-0.2px" }}>{name}</span>
          <span className="text-[12px] text-text-muted leading-tight font-medium truncate">{emails.join(", ")}</span>
          <span className="text-[11px] text-text-dim leading-tight font-semibold">{totalCount} messages</span>
        </div>
      </div>

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-3 py-4 flex flex-col gap-2">
        <div
          className="self-center text-text-muted text-[12px] font-semibold px-3 py-1 rounded-full mb-4"
          style={{
            background: 'color-mix(in srgb, var(--color-accent-purple) 8%, var(--color-bg-secondary))',
            border: '1px solid color-mix(in srgb, var(--color-accent-purple) 15%, transparent)',
          }}
        >
          Derived from {totalCount} emails{oldestYear ? ` since ${oldestYear}` : ""}
        </div>

        {messagesLoading ? (
          <div className="text-center py-10 text-text-muted font-semibold text-[16px]">Loading messages&hellip;</div>
        ) : (
          dedup(messages).sort((a, b) => a.date - b.date).map((m, i, arr) => {
            const isSent = m.is_sent;
            const body = m.distilled_text || m.body_text || m.subject || "";
            const sender = firstName(m.from_name || m.from_address);
            const year = new Date(m.date).getFullYear();
            const prevYear = i > 0 ? new Date(arr[i - 1].date).getFullYear() : year;
            const prevMsg = i > 0 ? arr[i - 1] : null;
            const timeDelta = prevMsg ? m.date - prevMsg.date : Infinity;
            const senderChanged = prevMsg ? prevMsg.is_sent !== m.is_sent || prevMsg.from_address !== m.from_address : true;
            const showTime = senderChanged || timeDelta > 30 * 60 * 1000;
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
                className={`flex flex-col ${isSent ? "items-end" : "items-start"} cursor-pointer`}
                onClick={() => handleSelectMessage(m)}
              >
                {i > 0 && year !== prevYear && (
                  <div
                    className="self-center text-text-dim text-[12px] font-semibold px-3 py-1 rounded-full my-4"
                    style={{
                      background: 'color-mix(in srgb, var(--color-accent-purple) 8%, var(--color-bg-secondary))',
                      border: '1px solid color-mix(in srgb, var(--color-accent-purple) 15%, transparent)',
                    }}
                  >
                    {year}
                  </div>
                )}
                {!isSent && isMultiParticipant && (
                  <span className="text-[12px] font-bold mb-0.5 px-1 ml-8" style={{ letterSpacing: '0.1px' }}>
                    <span style={{ color: colorOf(m.from_address) }}>{sender}</span>
                    {missing.length > 0 && (
                      <span className="text-text-muted line-through ml-1">{missing.join(", ")}</span>
                    )}
                  </span>
                )}
                {isSent && isMultiParticipant && missing.length > 0 && (
                  <span className="text-[11px] text-text-muted line-through mb-0.5 px-1 font-semibold">{missing.join(", ")}</span>
                )}
                <div className={`flex items-end gap-2 ${isSent ? "flex-row-reverse" : ""} max-w-[85%] transition-transform active:scale-[0.98]`}>
                  {!isSent && isMultiParticipant && (
                    <Avatar name={sender} email={m.from_address} size={9} fontSize="text-[12px]" className="shrink-0" color={colorOf(m.from_address)} />
                  )}
                  <div className={`min-w-0 px-3.5 py-2.5 text-[15px] font-medium leading-snug break-words ${isSent
                    ? "bg-accent-blue text-white rounded-[14px_14px_4px_14px]"
                    : "bg-bg-secondary text-text-primary rounded-[14px_14px_14px_4px]"
                    }`}
                    style={isSent
                      ? { boxShadow: '0 2px 8px color-mix(in srgb, var(--color-accent-blue) 30%, transparent)' }
                      : { boxShadow: '0 1px 8px rgba(0,0,0,0.08)', border: '1px solid rgba(0,0,0,0.05)' }
                    }
                  >
                    {body}
                  </div>
                </div>
                {showTime && (
                  <span className="text-[11px] text-text-dim px-1 pt-0.5 font-medium">{fmtTime(m.date)}</span>
                )}
              </div>
            );
          })
        )}
        <div ref={bottomRef} />
      </div>

      {/* Compose */}
      <div
        className="flex items-center gap-2 px-3 pt-2.5 shrink-0 bg-bg-secondary"
        style={{
          paddingBottom: 'calc(0.625rem + env(safe-area-inset-bottom, 0px))',
          boxShadow: '0 -2px 12px rgba(0,0,0,0.05)',
        }}
      >
        <button className="w-10 h-10 rounded-[12px] border border-divider bg-bg-primary text-xl text-text-dim cursor-pointer flex items-center justify-center shrink-0 leading-none font-light hover:border-accent-green hover:text-accent-green active:scale-95 transition-all">+</button>
        <input
          className="compose-input flex-1 py-2.5 px-3.5 rounded-[12px] text-[16px] font-medium outline-none bg-bg-primary text-text-primary placeholder:text-text-dim"
          style={{ border: '1px solid var(--color-divider)', transition: 'border-color 0.15s, box-shadow 0.15s' }}
          placeholder={"Message\u2026"}
        />
        <button
          className="w-10 h-10 rounded-[12px] border-none bg-accent-green text-white text-base font-extrabold cursor-pointer flex items-center justify-center shrink-0 hover:brightness-90 active:scale-95 transition"
          style={{ boxShadow: '0 2px 8px color-mix(in srgb, var(--color-accent-green) 35%, transparent)' }}
        >
          {"\u2191"}
        </button>
      </div>
    </div>
  );
}
