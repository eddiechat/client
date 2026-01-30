import { useState, useRef, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Conversation, Message, ComposeAttachment } from "../types";
import {
  getAvatarColor,
  getInitials,
  extractEmail,
  getGravatarUrl,
  getConversationNameParts,
  parseEmailContent,
  hasExpandableContent,
} from "../lib/utils";
import { searchEmojis } from "../lib/emojiData";
import { Avatar } from "./Avatar";
import { GravatarModal } from "./GravatarModal";
import { AttachmentList } from "./AttachmentList";
import { ChatMessageAsEmail } from "./ChatMessageAsEmail";
import { EmojiPicker, EmojiSuggestions } from "./EmojiPicker";

interface ConversationViewProps {
  conversation: Conversation | null;
  messages: Message[];
  loading?: boolean;
  error?: string | null;
  currentAccountEmail?: string;
  onSendMessage: (text: string, attachments?: ComposeAttachment[]) => void;
  onBack?: () => void;
  isComposing?: boolean;
  composeParticipants?: string[];
  onComposeParticipantsConfirm?: (participants: string[]) => void;
  onSendNewMessage?: (text: string, participants: string[], attachments?: ComposeAttachment[]) => void;
}

function formatMessageTime(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

function formatDateSeparator(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) return date.toLocaleDateString([], { weekday: "long" });
  return date.toLocaleDateString([], {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: now.getFullYear() !== date.getFullYear() ? "numeric" : undefined,
  });
}

function isDifferentDay(date1: string, date2: string): boolean {
  const d1 = new Date(date1);
  const d2 = new Date(date2);
  return d1.getFullYear() !== d2.getFullYear() || d1.getMonth() !== d2.getMonth() || d1.getDate() !== d2.getDate();
}

function getSenderName(from: string): string {
  const cleanName = from.replace(/<[^>]+>/g, "").trim();
  if (!cleanName || cleanName.includes("@")) {
    const match = from.match(/<([^>]+)>/);
    const email = match ? match[1] : from;
    return email.split("@")[0];
  }
  return cleanName;
}

function getAvatarTooltip(from: string, messageId?: string): string {
  const name = getSenderName(from);
  const emailMatch = from.match(/<([^>]+)>/);
  const email = emailMatch ? emailMatch[1] : from.replace(/^[^<]*/, "").trim();
  let tooltip = name && email && name !== email && !name.includes("@") ? `${name} <${email}>` : email || from;
  if (import.meta.env.DEV && messageId) tooltip += `\nID: ${messageId}`;
  return tooltip;
}

function isOutgoing(message: Message, currentAccountEmail?: string): boolean {
  if (!currentAccountEmail) return false;
  const fromEmail = message.envelope.from.toLowerCase();
  const accountEmail = currentAccountEmail.toLowerCase();
  return fromEmail.includes(accountEmail) || accountEmail.includes(fromEmail.replace(/<|>/g, "").split("@")[0]);
}

function getConversationName(conversation: Conversation): string {
  return getConversationNameParts(conversation).map(p => p.name).join(", ");
}

function getHeaderAvatarTooltip(conversation: Conversation): string {
  return conversation.participants.map((email, index) => {
    const name = conversation.participant_names[index];
    return name && name !== email && !name.includes("@") ? `${name} <${email}>` : email;
  }).join("\n");
}

export function ConversationView({
  conversation,
  messages,
  loading,
  error,
  currentAccountEmail,
  onSendMessage,
  onBack,
  isComposing,
  composeParticipants = [],
  onComposeParticipantsConfirm,
  onSendNewMessage,
}: ConversationViewProps) {
  const [inputValue, setInputValue] = useState("");
  const [toInputValue, setToInputValue] = useState("");
  const [participantsConfirmed, setParticipantsConfirmed] = useState(false);
  const [gravatarModalData, setGravatarModalData] = useState<{ email: string; name: string } | null>(null);
  const [fullViewMessage, setFullViewMessage] = useState<Message | null>(null);
  const [attachments, setAttachments] = useState<ComposeAttachment[]>([]);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const [emojiSuggestion, setEmojiSuggestion] = useState<{ query: string; startPos: number } | null>(null);
  const [suggestionIndex, setSuggestionIndex] = useState(0);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const toInputRef = useRef<HTMLInputElement>(null);
  const prevConversationRef = useRef<Conversation | null>(null);
  const wasComposingRef = useRef(false);

  useEffect(() => {
    setGravatarModalData(null);
    setFullViewMessage(null);
  }, [conversation?.id]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    if (isComposing && !participantsConfirmed && composeParticipants.length === 0) {
      setTimeout(() => toInputRef.current?.focus(), 50);
      setToInputValue("");
      setInputValue("");
      setParticipantsConfirmed(false);
    } else if (isComposing && (participantsConfirmed || composeParticipants.length > 0)) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isComposing, participantsConfirmed, composeParticipants.length]);

  useEffect(() => {
    if (wasComposingRef.current && !isComposing && conversation) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
    wasComposingRef.current = isComposing || false;
    prevConversationRef.current = conversation;
  }, [isComposing, conversation]);

  useEffect(() => {
    if (!isComposing) {
      setToInputValue("");
      setParticipantsConfirmed(false);
      setAttachments([]);
    }
  }, [isComposing]);

  useEffect(() => {
    setAttachments([]);
  }, [conversation?.id]);

  const handleAddAttachment = async () => {
    try {
      const selected = await open({ multiple: true, title: "Select files to attach" });
      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
        const mimeTypes: Record<string, string> = {
          pdf: "application/pdf", doc: "application/msword", docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
          xls: "application/vnd.ms-excel", xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
          txt: "text/plain", csv: "text/csv", html: "text/html", json: "application/json", zip: "application/zip",
          png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp",
          mp3: "audio/mpeg", mp4: "video/mp4",
        };
        const newAttachments: ComposeAttachment[] = files.map((filePath) => {
          const fileName = filePath.split(/[/\\]/).pop() || "attachment";
          const extension = fileName.split(".").pop()?.toLowerCase() || "";
          return { path: filePath, name: fileName, mime_type: mimeTypes[extension] || "application/octet-stream", size: 0 };
        });
        setAttachments((prev) => [...prev, ...newAttachments]);
      }
    } catch (error) {
      console.error("Failed to select files:", error);
    }
  };

  const handleRemoveAttachment = (index: number) => setAttachments((prev) => prev.filter((_, i) => i !== index));

  const handleEmojiSelect = (emoji: string) => {
    if (inputRef.current) {
      const start = inputRef.current.selectionStart || 0;
      const end = inputRef.current.selectionEnd || 0;
      const newValue = inputValue.slice(0, start) + emoji + inputValue.slice(end);
      setInputValue(newValue);
      setTimeout(() => {
        if (inputRef.current) {
          const newPos = start + emoji.length;
          inputRef.current.setSelectionRange(newPos, newPos);
          inputRef.current.focus();
        }
      }, 0);
    } else {
      setInputValue(inputValue + emoji);
    }
    setShowEmojiPicker(false);
  };

  const handleEmojiSuggestionSelect = (emoji: string, _name: string) => {
    if (emojiSuggestion && inputRef.current) {
      const beforeColon = inputValue.slice(0, emojiSuggestion.startPos);
      const afterQuery = inputValue.slice(inputRef.current.selectionStart || emojiSuggestion.startPos + emojiSuggestion.query.length + 1);
      const newValue = beforeColon + emoji + afterQuery;
      setInputValue(newValue);
      setEmojiSuggestion(null);
      setSuggestionIndex(0);
      setTimeout(() => {
        if (inputRef.current) {
          const newPos = beforeColon.length + emoji.length;
          inputRef.current.setSelectionRange(newPos, newPos);
          inputRef.current.focus();
        }
      }, 0);
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    setInputValue(newValue);
    const cursorPos = e.target.selectionStart || 0;
    const textBeforeCursor = newValue.slice(0, cursorPos);
    const colonMatch = textBeforeCursor.match(/:([a-zA-Z0-9_]*)$/);
    if (colonMatch && colonMatch[1].length >= 1) {
      setEmojiSuggestion({ query: colonMatch[1], startPos: cursorPos - colonMatch[0].length });
      setSuggestionIndex(0);
    } else {
      setEmojiSuggestion(null);
    }
  };

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (emojiSuggestion) {
      const suggestions = searchEmojis(emojiSuggestion.query, 8);
      if (e.key === "ArrowDown") { e.preventDefault(); setSuggestionIndex((prev) => (prev + 1) % suggestions.length); }
      else if (e.key === "ArrowUp") { e.preventDefault(); setSuggestionIndex((prev) => (prev - 1 + suggestions.length) % suggestions.length); }
      else if (e.key === "Enter" && suggestions.length > 0) { e.preventDefault(); handleEmojiSuggestionSelect(suggestions[suggestionIndex].emoji, suggestions[suggestionIndex].name); }
      else if (e.key === "Escape") { e.preventDefault(); setEmojiSuggestion(null); }
      else if (e.key === "Tab" && suggestions.length > 0) { e.preventDefault(); handleEmojiSuggestionSelect(suggestions[suggestionIndex].emoji, suggestions[suggestionIndex].name); }
    }
  };

  const handleToKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      const participants = toInputValue.split(",").map((s) => s.trim()).filter(Boolean);
      if (participants.length > 0 && onComposeParticipantsConfirm) {
        setParticipantsConfirmed(true);
        onComposeParticipantsConfirm(participants);
      }
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (inputValue.trim() || attachments.length > 0) {
      if (isComposing && !conversation && composeParticipants.length > 0 && onSendNewMessage) {
        onSendNewMessage(inputValue.trim(), composeParticipants, attachments.length > 0 ? attachments : undefined);
        setInputValue("");
        setAttachments([]);
        setParticipantsConfirmed(false);
      } else if (conversation) {
        onSendMessage(inputValue.trim(), attachments.length > 0 ? attachments : undefined);
        setInputValue("");
        setAttachments([]);
      }
      inputRef.current?.focus();
    }
  };

  // Compose mode
  if (isComposing && !conversation) {
    const hasParticipants = composeParticipants.length > 0 || participantsConfirmed;
    return (
      <div className="flex flex-col h-full">
        {/* Header */}
        <div className="flex items-center gap-3 px-4 h-16 safe-x" style={{ paddingTop: 'calc(0.75rem + env(safe-area-inset-top))', paddingBottom: '0.75rem' }}>
          {onBack && (
            <button className="flex md:hidden w-9 h-9 rounded-full items-center justify-center hover:bg-bg-hover transition-colors" onClick={onBack}>
              <svg className="w-5 h-5 text-text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M19 12H5M12 19l-7-7 7-7" />
              </svg>
            </button>
          )}
          {!hasParticipants ? (
            <div className="flex-1 flex items-center gap-2">
              <span className="text-[15px] font-medium text-text-muted whitespace-nowrap">To:</span>
              <input
                ref={toInputRef}
                type="text"
                className="flex-1 bg-transparent border-none text-text-primary text-[15px] outline-none py-2 placeholder:text-text-muted"
                placeholder="Enter email addresses (comma-separated)"
                value={toInputValue}
                onChange={(e) => setToInputValue(e.target.value)}
                onKeyDown={handleToKeyDown}
              />
            </div>
          ) : (
            <>
              <div className="w-10 h-10 relative flex items-center">
                {composeParticipants.slice(0, 2).map((participant, index) => (
                  <Avatar key={index} email={extractEmail(participant)} name={participant} size={40}
                    className={`${composeParticipants.length > 1 ? `w-7 h-7 min-w-7 text-xs border-2 border-bg-secondary absolute ${index === 0 ? "left-0 z-20" : "left-3 z-10"}` : ""}`}
                  />
                ))}
              </div>
              <div className="flex-1 min-w-0">
                <h2 className="text-[17px] font-semibold text-text-primary truncate">{composeParticipants.join(", ")}</h2>
                <span className="text-[13px] text-text-muted">New conversation</span>
              </div>
            </>
          )}
        </div>

        {/* Empty area */}
        <div className="flex-1 overflow-y-auto p-4 flex flex-col items-center justify-center">
          <p className="text-text-muted text-sm text-center">{!hasParticipants ? "Enter recipients and press Enter to start a new conversation" : "Start your conversation"}</p>
        </div>

        {/* Input */}
        {hasParticipants && (
          <form className="flex flex-col gap-2 px-4 safe-x" style={{ paddingBottom: 'calc(0.75rem + env(safe-area-inset-bottom))', paddingTop: '0.75rem' }} onSubmit={handleSubmit}>
            {attachments.length > 0 && (
              <div className="flex flex-wrap gap-2 p-3 bg-accent-green/10 border border-dashed border-accent-green/40 rounded-xl mb-2">
                <span className="w-full text-[11px] font-medium text-accent-green uppercase tracking-wider mb-1">Ready to send</span>
                {attachments.map((attachment, index) => (
                  <div key={index} className="inline-flex items-center gap-1 bg-white/[0.08] rounded-md px-2 py-1 max-w-[200px]">
                    <span className="text-[13px] text-text-primary truncate" title={attachment.name}>{attachment.name}</span>
                    <button type="button" className="w-4 h-4 min-w-4 rounded-full flex items-center justify-center opacity-60 hover:opacity-100" onClick={() => handleRemoveAttachment(index)}>
                      <svg className="w-3 h-3 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M18 6L6 18M6 6l12 12" /></svg>
                    </button>
                  </div>
                ))}
              </div>
            )}
            <div className="flex items-center gap-2">
              <button type="button" className="w-10 h-10 min-w-10 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors" onClick={handleAddAttachment}>
                <svg className="w-5 h-5 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" /></svg>
              </button>
              <div className="flex-1 relative flex items-center">
                {emojiSuggestion && <EmojiSuggestions query={emojiSuggestion.query} onSelect={handleEmojiSuggestionSelect} onClose={() => setEmojiSuggestion(null)} selectedIndex={suggestionIndex} />}
                {showEmojiPicker && <EmojiPicker onSelect={handleEmojiSelect} onClose={() => setShowEmojiPicker(false)} />}
                <input ref={inputRef} type="text" className="w-full py-2.5 pl-4 pr-11 bg-bg-tertiary border-none rounded-3xl text-text-primary text-[15px] outline-none focus:bg-bg-hover transition-colors placeholder:text-text-muted" placeholder="Type your message (first line becomes subject)" value={inputValue} onChange={handleInputChange} onKeyDown={handleInputKeyDown} />
                <button type="button" className="absolute right-1 w-8 h-8 min-w-8 rounded-full flex items-center justify-center hover:bg-bg-hover" onClick={() => setShowEmojiPicker(!showEmojiPicker)}>
                  <svg className="w-4 h-4 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10" /><path d="M8 14s1.5 2 4 2 4-2 4-2" /><circle cx="9" cy="9" r="1" fill="currentColor" stroke="none" /><circle cx="15" cy="9" r="1" fill="currentColor" stroke="none" /></svg>
                </button>
              </div>
            </div>
          </form>
        )}
      </div>
    );
  }

  // Empty state
  if (!conversation) {
    return (
      <div className="flex flex-col h-full items-center justify-center">
        <div className="text-center p-10 max-w-xs">
          <div className="w-20 h-20 mx-auto mb-6 bg-bg-tertiary rounded-full flex items-center justify-center">
            <svg className="w-10 h-10 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" /></svg>
          </div>
          <h3 className="text-xl font-semibold text-text-primary mb-2">Select a conversation</h3>
          <p className="text-sm text-text-muted leading-relaxed">Choose a conversation from the list to start messaging</p>
        </div>
      </div>
    );
  }

  const conversationName = getConversationName(conversation);
  const headerTooltip = getHeaderAvatarTooltip(conversation);
  const userEmail = currentAccountEmail?.toLowerCase() || extractEmail(conversation.user_name);
  const participantData = conversation.participants.map((p, idx) => ({ email: extractEmail(p), name: conversation.participant_names[idx] || extractEmail(p) }));
  const headerAvatarsToShow = participantData.filter(pd => pd.email !== userEmail).slice(0, 2);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 px-4" style={{ minHeight: '4rem', paddingTop: 'calc(0.75rem + env(safe-area-inset-top))', paddingBottom: '0.75rem' }}>
        {onBack && (
          <button className="flex md:hidden w-9 h-9 rounded-full items-center justify-center hover:bg-bg-hover transition-colors" onClick={onBack}>
            <svg className="w-5 h-5 text-text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M19 12H5M12 19l-7-7 7-7" /></svg>
          </button>
        )}
        <div className="w-10 h-10 relative flex items-center" title={headerTooltip}>
          {headerAvatarsToShow.map((pd, index) => (
            <Avatar key={index} email={pd.email} name={pd.name} size={40}
              className={`${headerAvatarsToShow.length > 1 ? `w-7 h-7 min-w-7 text-xs border-2 border-bg-secondary absolute ${index === 0 ? "left-0 z-20" : "left-3 z-10"}` : ""}`}
            />
          ))}
        </div>
        <div className="flex-1 min-w-0">
          <h2 className="text-[17px] font-semibold text-text-primary truncate">{conversationName}</h2>
          <span className="text-[13px] text-text-muted">{conversation.participant_names.length > 1 ? `${conversation.participant_names.length} participants` : ""}</span>
        </div>
        <div className="flex gap-1">
          <button className="w-9 h-9 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors"><svg className="w-5 h-5 text-text-secondary" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="11" cy="11" r="8" /><path d="m21 21-4.35-4.35" /></svg></button>
          <button className="w-9 h-9 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors"><svg className="w-5 h-5 text-text-secondary" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="5" r="1.5" /><circle cx="12" cy="12" r="1.5" /><circle cx="12" cy="19" r="1.5" /></svg></button>
        </div>
      </div>

      {/* Content area */}
      {fullViewMessage ? (
        <ChatMessageAsEmail message={fullViewMessage} onClose={() => setFullViewMessage(null)} />
      ) : gravatarModalData ? (
        <GravatarModal email={gravatarModalData.email} name={gravatarModalData.name} isOpen={!!gravatarModalData} onClose={() => setGravatarModalData(null)} />
      ) : (
        <>
          {/* Messages */}
          <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-1">
            {loading ? (
              <div className="flex-1 flex flex-col items-center justify-center gap-3 text-text-muted text-sm"><div className="spinner" /><span>Loading messages...</span></div>
            ) : error ? (
              <div className="flex-1 flex items-center justify-center text-text-muted text-sm">Error loading messages: {error}</div>
            ) : messages.length === 0 ? (
              <div className="flex-1 flex items-center justify-center text-text-muted text-sm">No messages yet. Start the conversation!</div>
            ) : (
              <>
                {messages.map((message, index) => {
                  const isOut = isOutgoing(message, currentAccountEmail);
                  const showDateSeparator = index === 0 || isDifferentDay(messages[index - 1].envelope.date, message.envelope.date);
                  const showSender = !isOut && conversation.participant_names.length > 2;

                  return (
                    <div key={message.id}>
                      {showDateSeparator && (
                        <div className="flex items-center justify-center py-4">
                          <span className="px-3 py-1.5 bg-bg-tertiary rounded-2xl text-xs font-medium text-text-muted">{formatDateSeparator(message.envelope.date)}</span>
                        </div>
                      )}
                      <div className={`flex gap-2 max-w-[85%] md:max-w-[75%] mb-1 ${isOut ? "self-end flex-row-reverse ml-auto" : "self-start"}`}>
                        {!isOut && (
                          <div
                            className="w-8 h-8 min-w-8 rounded-full flex items-center justify-center text-xs font-semibold text-white uppercase self-end cursor-pointer overflow-hidden relative"
                            style={{ backgroundColor: getAvatarColor(message.envelope.from) }}
                            title={getAvatarTooltip(message.envelope.from, message.id)}
                            onClick={() => { const email = extractEmail(message.envelope.from); const name = getSenderName(message.envelope.from); if (email) setGravatarModalData({ email, name }); }}
                          >
                            {(() => {
                              const messageEmail = extractEmail(message.envelope.from);
                              const gravatarUrl = messageEmail ? getGravatarUrl(messageEmail, 32) : null;
                              return (
                                <>
                                  {gravatarUrl && <img src={gravatarUrl} alt={getSenderName(message.envelope.from)} className="absolute inset-0 w-full h-full object-cover rounded-full" onError={(e) => { e.currentTarget.style.display = 'none'; }} />}
                                  <span className="avatar-initials">{getInitials(message.envelope.from)}</span>
                                </>
                              );
                            })()}
                          </div>
                        )}
                        <div className="flex flex-col gap-0.5 min-w-0 overflow-hidden">
                          {showSender && <span className="text-xs font-medium text-accent-blue pl-3">{getSenderName(message.envelope.from)}</span>}
                          {(() => {
                            const isExpandable = hasExpandableContent(message.text_body, message.html_body);
                            return (
                              <div
                                className={`px-3.5 py-2.5 rounded-2xl max-w-full break-words relative ${isOut ? "bg-bubble-sent text-white rounded-br-sm" : "bg-bubble-received text-white rounded-bl-sm"
                                  } ${isExpandable ? "border border-white/15 cursor-pointer hover:border-white/30" : ""}`}
                                onClick={isExpandable ? () => setFullViewMessage(message) : undefined}
                                title={isExpandable ? "Click to view full message" : undefined}
                              >
                                <div className="text-[15px] leading-snug whitespace-pre-wrap break-words">{parseEmailContent(message.text_body) || message.envelope.subject || "(No content)"}</div>
                                <AttachmentList messageId={message.id} hasAttachment={message.envelope.has_attachment} />
                                <span className="flex items-center gap-1 text-[11px] opacity-70 mt-1 justify-end">
                                  {formatMessageTime(message.envelope.date)}
                                  {isOut && <svg className="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M18 7l-8.5 8.5-4-4L4 13l5.5 5.5L19.5 8.5z" /></svg>}
                                </span>
                              </div>
                            );
                          })()}
                        </div>
                      </div>
                    </div>
                  );
                })}
                <div ref={messagesEndRef} />
              </>
            )}
          </div>

          {/* Input */}
          <form className="flex flex-col gap-2 px-4 safe-x" style={{ paddingBottom: 'calc(0.75rem + env(safe-area-inset-bottom))', paddingTop: '0.75rem' }} onSubmit={handleSubmit}>
            {attachments.length > 0 && (
              <div className="flex flex-wrap gap-2 p-3 bg-accent-green/10 border border-dashed border-accent-green/40 rounded-xl mb-2">
                <span className="w-full text-[11px] font-medium text-accent-green uppercase tracking-wider mb-1">Ready to send</span>
                {attachments.map((attachment, index) => (
                  <div key={index} className="inline-flex items-center gap-1 bg-white/[0.08] rounded-md px-2 py-1 max-w-[200px]">
                    <span className="text-[13px] text-text-primary truncate" title={attachment.name}>{attachment.name}</span>
                    <button type="button" className="w-4 h-4 min-w-4 rounded-full flex items-center justify-center opacity-60 hover:opacity-100" onClick={() => handleRemoveAttachment(index)}>
                      <svg className="w-3 h-3 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M18 6L6 18M6 6l12 12" /></svg>
                    </button>
                  </div>
                ))}
              </div>
            )}
            <div className="flex items-center gap-2">
              <button type="button" className="w-10 h-10 min-w-10 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors" onClick={handleAddAttachment}>
                <svg className="w-5 h-5 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" /></svg>
              </button>
              <div className="flex-1 relative flex items-center">
                {emojiSuggestion && <EmojiSuggestions query={emojiSuggestion.query} onSelect={handleEmojiSuggestionSelect} onClose={() => setEmojiSuggestion(null)} selectedIndex={suggestionIndex} />}
                {showEmojiPicker && <EmojiPicker onSelect={handleEmojiSelect} onClose={() => setShowEmojiPicker(false)} />}
                <input ref={inputRef} type="text" className="w-full py-2.5 pl-4 pr-11 bg-bg-tertiary border-none rounded-3xl text-text-primary text-[15px] outline-none focus:bg-bg-hover transition-colors placeholder:text-text-muted" placeholder="Message" value={inputValue} onChange={handleInputChange} onKeyDown={handleInputKeyDown} />
                <button type="button" className="absolute right-1 w-8 h-8 min-w-8 rounded-full flex items-center justify-center hover:bg-bg-hover" onClick={() => setShowEmojiPicker(!showEmojiPicker)}>
                  <svg className="w-4 h-4 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10" /><path d="M8 14s1.5 2 4 2 4-2 4-2" /><circle cx="9" cy="9" r="1" fill="currentColor" stroke="none" /><circle cx="15" cy="9" r="1" fill="currentColor" stroke="none" /></svg>
                </button>
              </div>
            </div>
          </form>
        </>
      )}
    </div>
  );
}
