import { useState, useEffect, useRef, useCallback } from "react";
import { createFileRoute, useRouter, useNavigate } from "@tanstack/react-router";
import { useAuth, useTheme } from "../../shared/context";
import { useData } from "../../shared/context";
import { fetchConversationMessages, queueAction, sendMessage } from "../../tauri";
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
import { Avatar, PartitionedAvatar, MessageDetail, LinkifiedText } from "../../shared/components";

export const Route = createFileRoute("/_app/conversation/$id")({
  component: ConversationView,
});

function ConversationView() {
  useTheme(); // subscribe to theme changes for avatar colors
  const { id } = Route.useParams();
  const router = useRouter();
  const navigate = useNavigate();
  const { myAddrs, email: myEmail, accountId } = useAuth();
  const { conversations } = useData();

  const [messages, setMessages] = useState<Message[]>([]);
  const [messagesLoading, setMessagesLoading] = useState(false);
  const [selectedMessage, setSelectedMessage] = useState<Message | null>(null);
  const [composeText, setComposeText] = useState("");
  const [sending, setSending] = useState(false);
  const [replyingTo, setReplyingTo] = useState<Message | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const savedScrollRef = useRef<number>(0);
  const markedReadRef = useRef<Set<string>>(new Set());
  const visibilityTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const composeRef = useRef<HTMLInputElement>(null);

  // Parse search params for new-conversation compose flow
  const searchParams = new URLSearchParams(window.location.hash.split("?")[1] || "");
  const newTo = searchParams.get("to")?.split(",").filter(Boolean) || [];
  const newSubject = searchParams.get("subject") || "";
  const newFrom = searchParams.get("from") || myEmail || "";
  const wantsFocus = searchParams.has("compose") || id === "__new__";

  // Find conversation by ID, or by matching participants if __new__
  let conversation = conversations.find((c) => c.id === id);
  if (!conversation && newTo.length > 0) {
    const toSet = new Set(newTo.map((e) => e.toLowerCase()));
    conversation = conversations.find((c) => {
      const emails = participantEmails(c);
      if (emails.length !== toSet.size) return false;
      return emails.every((e) => toSet.has(e.toLowerCase()));
    });
  }
  const isNewConversation = !conversation && (id === "__new__" || searchParams.has("to"));

  // Redirect to real conversation ID if we found a match for __new__
  useEffect(() => {
    if (id === "__new__" && conversation) {
      navigate({ to: "/conversation/$id", params: { id: conversation.id }, replace: true });
    }
  }, [id, conversation, navigate]);

  // Re-fetch messages when conversation changes or when conversations list updates
  // (e.g. after send, the ConversationsUpdated event triggers a list refresh which
  // changes the conversation object reference, causing this effect to re-run).
  const convLastMessage = conversation?.last_message_date;
  useEffect(() => {
    if (isNewConversation || !conversation) return;
    if (messages.length === 0) setMessagesLoading(true);
    setSelectedMessage(null);
    fetchConversationMessages(conversation.account_id, conversation.id)
      .then(setMessages)
      .catch(() => {})
      .finally(() => setMessagesLoading(false));
  }, [conversation?.id, convLastMessage, isNewConversation]);

  useEffect(() => {
    if (!messagesLoading && messages.length > 0) {
      bottomRef.current?.scrollIntoView();
    }
  }, [messagesLoading, messages]);

  // Auto-focus compose input when arriving from compose flow
  useEffect(() => {
    if (wantsFocus && !messagesLoading && composeRef.current) {
      composeRef.current.focus();
    }
  }, [wantsFocus, messagesLoading]);

  // Restore scroll position when returning from detail view
  useEffect(() => {
    if (!selectedMessage && scrollRef.current && savedScrollRef.current > 0) {
      scrollRef.current.scrollTop = savedScrollRef.current;
    }
  }, [selectedMessage]);

  // Helper: check if a message is unread (missing \\Seen flag, and not sent by us)
  const isUnread = useCallback((m: Message): boolean => {
    if (m.is_sent) return false;
    try {
      const flags: string[] = JSON.parse(m.imap_flags || "[]");
      return !flags.some(f => f.toLowerCase().includes("seen"));
    } catch { return false; }
  }, []);

  // Mark-as-read: IntersectionObserver with 1s delay
  const observerRef = useRef<IntersectionObserver | null>(null);
  const msgElementsRef = useRef<Map<string, HTMLDivElement>>(new Map());

  const markAsRead = useCallback((toMark: Message[]) => {
    if (!conversation || toMark.length === 0) return;

    // Group by folder
    const byFolder = new Map<string, { uids: number[]; ids: string[] }>();
    for (const m of toMark) {
      const folder = m.imap_folder || "INBOX";
      if (!byFolder.has(folder)) byFolder.set(folder, { uids: [], ids: [] });
      const group = byFolder.get(folder)!;
      group.uids.push(m.imap_uid);
      group.ids.push(m.id);
    }

    // Queue actions per folder
    for (const [folder, { uids }] of byFolder) {
      queueAction(conversation.account_id, "mark_read", { folder, uids }).catch(() => {});
    }

    // Optimistic update: add \\Seen to local flags
    const markedIds = new Set(toMark.map(m => m.id));
    setMessages(prev => prev.map(m => {
      if (!markedIds.has(m.id)) return m;
      try {
        const flags: string[] = JSON.parse(m.imap_flags || "[]");
        if (!flags.some(f => f.toLowerCase().includes("seen"))) {
          return { ...m, imap_flags: JSON.stringify([...flags, "\\Seen"]) };
        }
      } catch { /* keep original */ }
      return m;
    }));
  }, [conversation]);

  useEffect(() => {
    // Clean up previous observer
    observerRef.current?.disconnect();
    visibilityTimers.current.forEach(t => clearTimeout(t));
    visibilityTimers.current.clear();

    observerRef.current = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const msgId = (entry.target as HTMLElement).dataset.msgId;
        if (!msgId) continue;

        if (entry.isIntersecting) {
          // Start 1s timer
          if (!visibilityTimers.current.has(msgId)) {
            const timer = setTimeout(() => {
              visibilityTimers.current.delete(msgId);
              markedReadRef.current.add(msgId);
              // Find the message and mark it
              const msg = messages.find(m => m.id === msgId);
              if (msg) markAsRead([msg]);
              // Stop observing this element
              const el = msgElementsRef.current.get(msgId);
              if (el) observerRef.current?.unobserve(el);
            }, 1000);
            visibilityTimers.current.set(msgId, timer);
          }
        } else {
          // Cancel timer if scrolled away before 1s
          const timer = visibilityTimers.current.get(msgId);
          if (timer) {
            clearTimeout(timer);
            visibilityTimers.current.delete(msgId);
          }
        }
      }
    }, { threshold: 0.5 });

    // Re-register elements with the new observer. React calls ref callbacks
    // during the commit phase (before effects), so elements were registered
    // with the now-disconnected old observer and must be re-added here.
    for (const [msgId, el] of msgElementsRef.current) {
      if (!markedReadRef.current.has(msgId)) {
        observerRef.current.observe(el);
      }
    }

    return () => {
      observerRef.current?.disconnect();
      visibilityTimers.current.forEach(t => clearTimeout(t));
      visibilityTimers.current.clear();
    };
  }, [messages, markAsRead]);

  // Ref callback for unread message elements
  const setMsgRef = useCallback((el: HTMLDivElement | null, m: Message) => {
    if (!el || !isUnread(m) || markedReadRef.current.has(m.id)) return;
    msgElementsRef.current.set(m.id, el);
    observerRef.current?.observe(el);
  }, [isUnread]);

  const handleSend = useCallback(async () => {
    const text = composeText.trim();
    if (!text || !accountId || !myEmail || sending) return;

    const toAddrs = isNewConversation ? newTo : (conversation ? participantEmails(conversation) : []);
    if (toAddrs.length === 0) return;

    // Compute subject, In-Reply-To, and References based on send mode:
    // - Reply button: thread against replied-to message, subject = "Re: <original>"
    // - Normal send: subject = "<name> via Eddie", thread against latest message with same subject
    let subject: string;
    let inReplyTo: string | undefined;
    let refs: string[] = [];

    if (replyingTo) {
      // Reply button: thread against the specific message, keep subject stable
      subject = replyingTo.subject?.match(/^re:/i) ? replyingTo.subject : `Re: ${replyingTo.subject || ""}`;
      inReplyTo = replyingTo.message_id || undefined;
      try {
        const existingRefs: string[] = JSON.parse(replyingTo.references_ids || "[]");
        refs = [...existingRefs, replyingTo.message_id].filter(Boolean);
      } catch { refs = [replyingTo.message_id].filter(Boolean); }
    } else if (isNewConversation) {
      // Brand-new conversation with no prior messages
      subject = newSubject || `${myEmail.split("@")[0]} via Eddie`;
    } else {
      // Normal send (no reply button): always use "via Eddie" subject.
      // Thread against the most recent "via Eddie" message if one exists.
      const viaSubject = `${myEmail.split("@")[0]} via Eddie`;
      subject = viaSubject;
      const lastViaMsg = [...messages].reverse().find(
        (m) => m.subject?.toLowerCase() === viaSubject.toLowerCase()
      );
      if (lastViaMsg) {
        inReplyTo = lastViaMsg.message_id || undefined;
        try {
          const existingRefs: string[] = JSON.parse(lastViaMsg.references_ids || "[]");
          refs = [...existingRefs, lastViaMsg.message_id].filter(Boolean);
        } catch { refs = [lastViaMsg.message_id].filter(Boolean); }
      }
    }

    setSending(true);
    try {
      const result = await sendMessage({
        accountId,
        fromEmail: isNewConversation ? newFrom : myEmail,
        to: toAddrs,
        cc: [],
        subject,
        body: text,
        inReplyTo,
        references: refs,
      });
      setComposeText("");
      setReplyingTo(null);

      if (isNewConversation) {
        navigate({
          to: "/conversation/$id",
          params: { id: result.conversation_id },
          replace: true,
        });
      } else if (conversation) {
        fetchConversationMessages(conversation.account_id, conversation.id)
          .then(setMessages)
          .catch(() => {});
      }
    } catch (e) {
      console.error("Send failed:", e);
    } finally {
      setSending(false);
    }
  }, [composeText, accountId, myEmail, sending, isNewConversation, newTo, newSubject, newFrom, conversation, replyingTo, messages, navigate]);

  const handleReply = useCallback((m: Message) => {
    setReplyingTo(m);
    composeRef.current?.focus();
  }, []);

  if (!conversation && !isNewConversation) {
    return (
      <div className="flex flex-col h-screen items-center justify-center text-text-muted font-semibold" style={{ background: "var(--color-bg-gradient)" }}>
        Conversation not found
      </div>
    );
  }

  // New conversation compose view
  if (isNewConversation) {
    const recipientNames = newTo.map(e => e.split("@")[0]).join(", ");
    return (
      <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-gradient)" }}>
        <div
          className="flex items-center gap-3 px-4 pb-3 shrink-0 bg-bg-secondary"
          style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))', boxShadow: '0 2px 12px rgba(0,0,0,0.06)' }}
        >
          <button className="border-none bg-transparent text-[26px] cursor-pointer text-text-secondary w-10 h-10 flex items-center justify-center rounded-[10px] hover:bg-bg-tertiary active:scale-95 transition-all font-bold -ml-1" onClick={() => router.history.back()}>&#8249;</button>
          <Avatar name={recipientNames} email={newTo[0]} size={12} fontSize="text-[16px]" className="shrink-0" />
          <div className="flex flex-col min-w-0">
            <span className="font-extrabold text-[16px] text-text-primary leading-tight truncate" style={{ letterSpacing: "-0.2px" }}>{recipientNames}</span>
            <span className="text-[12px] text-text-muted leading-tight font-medium truncate">{newTo.join(", ")}</span>
            <span className="text-[11px] text-text-dim leading-tight font-semibold">New conversation</span>
          </div>
        </div>
        <div className="flex-1 flex items-center justify-center px-6">
          <p className="text-[15px] text-text-dim text-center leading-relaxed">Type your first message below to start this conversation.</p>
        </div>
        <div className="flex items-center gap-2 px-3 pt-2.5 shrink-0 bg-bg-secondary" style={{ paddingBottom: 'calc(0.625rem + env(safe-area-inset-bottom, 0px))', boxShadow: '0 -2px 12px rgba(0,0,0,0.05)' }}>
          <input
            ref={composeRef}
            className="compose-input flex-1 py-2.5 px-3.5 rounded-[12px] text-[16px] font-medium outline-none bg-bg-primary text-text-primary placeholder:text-text-dim"
            style={{ border: '1px solid var(--color-divider)', transition: 'border-color 0.15s, box-shadow 0.15s' }}
            placeholder="Message…"
            value={composeText}
            onChange={(e) => setComposeText(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
            disabled={sending}
            autoFocus
          />
          <button
            className="w-10 h-10 rounded-[12px] border-none bg-accent-green text-white text-base font-extrabold cursor-pointer flex items-center justify-center shrink-0 hover:brightness-90 active:scale-95 transition disabled:opacity-40 disabled:cursor-not-allowed"
            style={{ boxShadow: '0 2px 8px color-mix(in srgb, var(--color-accent-green) 35%, transparent)' }}
            disabled={!composeText.trim() || sending}
            onClick={handleSend}
          >
            {sending ? "\u2026" : "\u2191"}
          </button>
        </div>
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

  // At this point conversation is guaranteed to exist (early returns handled all other cases)
  const conv = conversation!;
  const name = displayName(conv);
  const totalCount = conv.total_count;
  const emails = participantEmails(conv);
  // Build email→color map locally using same logic as PartitionedAvatar
  const entries = participantEntries(conv);
  const gHash = entries.reduce((acc, [email, n]) => acc + (n || email).split("").reduce((a, c) => a + c.charCodeAt(0), 0), 0);
  const palette = avatarGroupPalette(gHash);
  const emailColorMap = new Map<string, string>();
  entries.forEach(([email], i) => {
    emailColorMap.set(email.toLowerCase(), palette[i % palette.length]);
  });
  const colorOf = (email: string): string | undefined =>
    emailColorMap.get(email.toLowerCase());
  const isMultiParticipant = participantCount(conv) > 1;
  const participantMap: Record<string, string> = (() => {
    if (!conv.participant_names) return {};
    try {
      return JSON.parse(conv.participant_names) as Record<string, string>;
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
          <PartitionedAvatar participants={participantEntries(conv)} sizePx={50} conversationId={conv.id} />
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
            const showSubject = !!m.subject && !m.in_reply_to;
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
            // Compute quoted reply data for discontinuous replies
            const quoteData = (() => {
              if (!m.in_reply_to || !prevMsg) return null;
              const replyId = m.in_reply_to;
              if (prevMsg.message_id === replyId) return null;
              const quoted = messages.find(q => q.message_id === replyId);
              if (!quoted) return null;
              const isSelf = quoted.is_sent || myAddrs.has(quoted.from_address.toLowerCase());
              return {
                body: (quoted.distilled_text || quoted.body_text || "").slice(0, 60),
                sender: isSelf ? "Me" : firstName(quoted.from_name || quoted.from_address),
                color: colorOf(quoted.from_address) || "var(--color-text-muted)",
                email: quoted.from_address,
              };
            })();
            return (
              <div
                key={m.id}
                ref={(el) => setMsgRef(el, m)}
                data-msg-id={m.id}
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
                  <span className="text-[12px] font-bold mb-0.5 px-1 ml-11" style={{ letterSpacing: '0.1px' }}>
                    <span className="text-text-secondary">{sender}</span>
                    {missing.length > 0 && (
                      <span className="text-text-muted line-through ml-1">{missing.join(", ")}</span>
                    )}
                  </span>
                )}
                {isSent && isMultiParticipant && missing.length > 0 && (
                  <span className="text-[11px] text-text-muted line-through mb-0.5 px-1 font-semibold">{missing.join(", ")}</span>
                )}
                {quoteData && (
                  <div className={`flex items-center gap-1 text-[12px] text-text-muted mb-0.5 max-w-[85%] overflow-hidden ${isSent ? "self-end" : "self-start pl-1"}`}>
                    <span className="font-bold shrink-0 text-text-secondary">{quoteData.sender}:</span>
                    <span className="truncate opacity-70">{quoteData.body}</span>
                  </div>
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
                    {showSubject && (
                      <span className={`text-[13px] font-bold ${isSent ? "opacity-85" : "text-text-muted"}`}>{m.subject}{/[.!?:;]$/.test(m.subject!) ? " " : ". "}</span>
                    )}
                    <LinkifiedText text={body} />
                  </div>
                </div>
                <div className="flex items-center gap-2 px-1 pt-0.5">
                  {showTime && (
                    <span className="text-[11px] text-text-dim font-medium">{fmtTime(m.date)}</span>
                  )}
                  {!isSent && (
                    <button
                      className="text-[11px] text-text-dim font-medium bg-transparent border-none cursor-pointer p-0 hover:text-accent-green transition-colors"
                      onClick={(e) => { e.stopPropagation(); handleReply(m); }}
                    >
                      ↩ Reply
                    </button>
                  )}
                </div>
              </div>
            );
          })
        )}
        <div ref={bottomRef} />
      </div>

      {/* Compose */}
      <div
        className="flex flex-col shrink-0 bg-bg-secondary"
        style={{
          paddingBottom: 'calc(0.625rem + env(safe-area-inset-bottom, 0px))',
          boxShadow: '0 -2px 12px rgba(0,0,0,0.05)',
        }}
      >
        {/* Reply preview */}
        {replyingTo && (
          <div className="flex items-center gap-2 px-3 pt-2 pb-1">
            <div
              className="flex-1 text-[12px] text-text-muted pl-2 py-1 rounded truncate"
              style={{ borderLeft: '2px solid var(--color-accent-green)' }}
            >
              <span className="font-semibold text-text-primary">{firstName(replyingTo.from_name || replyingTo.from_address)}</span>
              {" "}{(replyingTo.distilled_text || replyingTo.body_text || "").slice(0, 60)}
            </div>
            <button
              className="text-text-dim text-[16px] bg-transparent border-none cursor-pointer p-1 hover:text-accent-red"
              onClick={() => setReplyingTo(null)}
            >
              &times;
            </button>
          </div>
        )}
        <div className="flex items-center gap-2 px-3 pt-2.5">
          <input
            ref={composeRef}
            className="compose-input flex-1 py-2.5 px-3.5 rounded-[12px] text-[16px] font-medium outline-none bg-bg-primary text-text-primary placeholder:text-text-dim"
            style={{ border: '1px solid var(--color-divider)', transition: 'border-color 0.15s, box-shadow 0.15s' }}
            placeholder={"Message\u2026"}
            value={composeText}
            onChange={(e) => setComposeText(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
            disabled={sending}
          />
          <button
            className="w-10 h-10 rounded-[12px] border-none bg-accent-green text-white text-base font-extrabold cursor-pointer flex items-center justify-center shrink-0 hover:brightness-90 active:scale-95 transition disabled:opacity-40 disabled:cursor-not-allowed"
            style={{ boxShadow: '0 2px 8px color-mix(in srgb, var(--color-accent-green) 35%, transparent)' }}
            disabled={!composeText.trim() || sending}
            onClick={handleSend}
          >
            {sending ? "\u2026" : "\u2191"}
          </button>
        </div>
      </div>
    </div>
  );
}
