import { useState, useRef, useEffect } from "react";
import type { Conversation, Message } from "../types";
import {
  getAvatarColor,
  getInitials,
  extractEmail,
  getGravatarUrl,
  getConversationNameParts,
  parseEmailContent,
} from "../lib/utils";
import { Avatar } from "./Avatar";

interface ConversationViewProps {
  conversation: Conversation | null;
  messages: Message[];
  loading?: boolean;
  error?: string | null;
  currentAccountEmail?: string;
  onSendMessage: (text: string) => void;
  onBack?: () => void;
  // Compose mode props
  isComposing?: boolean;
  composeParticipants?: string[];
  onComposeParticipantsConfirm?: (participants: string[]) => void;
  onSendNewMessage?: (text: string, participants: string[]) => void;
}

// Format time for message bubbles
function formatMessageTime(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

// Format date for date separators
function formatDateSeparator(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) {
    return "Today";
  } else if (diffDays === 1) {
    return "Yesterday";
  } else if (diffDays < 7) {
    return date.toLocaleDateString([], { weekday: "long" });
  } else {
    return date.toLocaleDateString([], {
      weekday: "long",
      month: "long",
      day: "numeric",
      year: now.getFullYear() !== date.getFullYear() ? "numeric" : undefined,
    });
  }
}

// Check if two dates are on different days
function isDifferentDay(date1: string, date2: string): boolean {
  const d1 = new Date(date1);
  const d2 = new Date(date2);
  return (
    d1.getFullYear() !== d2.getFullYear() ||
    d1.getMonth() !== d2.getMonth() ||
    d1.getDate() !== d2.getDate()
  );
}

// Get sender name from email
function getSenderName(from: string): string {
  const cleanName = from.replace(/<[^>]+>/g, "").trim();
  if (!cleanName || cleanName.includes("@")) {
    // Extract from email address
    const match = from.match(/<([^>]+)>/);
    const email = match ? match[1] : from;
    return email.split("@")[0];
  }
  return cleanName;
}

// Get tooltip text for avatar (name and email, plus message ID in dev mode)
function getAvatarTooltip(from: string, messageId?: string): string {
  const name = getSenderName(from);
  const emailMatch = from.match(/<([^>]+)>/);
  const email = emailMatch ? emailMatch[1] : from.replace(/^[^<]*/, "").trim();

  let tooltip: string;
  if (name && email && name !== email && !name.includes("@")) {
    tooltip = `${name} <${email}>`;
  } else {
    tooltip = email || from;
  }

  if (import.meta.env.DEV && messageId) {
    tooltip += `\nID: ${messageId}`;
  }

  return tooltip;
}

// Check if message is from current user
function isOutgoing(message: Message, currentAccountEmail?: string): boolean {
  if (!currentAccountEmail) return false;

  const fromEmail = message.envelope.from.toLowerCase();
  const accountEmail = currentAccountEmail.toLowerCase();

  return (
    fromEmail.includes(accountEmail) ||
    accountEmail.includes(fromEmail.replace(/<|>/g, "").split("@")[0])
  );
}

// Get conversation display name (first names, comma-separated)
function getConversationName(conversation: Conversation): string {
  const parts = getConversationNameParts(conversation);
  return parts.map(p => p.name).join(", ");
}

// Get tooltip text for header avatar showing all participants
function getHeaderAvatarTooltip(conversation: Conversation): string {
  return conversation.participants.map((email, index) => {
    const name = conversation.participant_names[index];
    if (name && name !== email && !name.includes("@")) {
      return `${name} <${email}>`;
    }
    return email;
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
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const toInputRef = useRef<HTMLInputElement>(null);

  // Track previous conversation to detect switches from compose mode
  const prevConversationRef = useRef<Conversation | null>(null);
  const wasComposingRef = useRef(false);

  // Scroll to bottom when messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Focus management for compose mode
  useEffect(() => {
    if (isComposing && !participantsConfirmed && composeParticipants.length === 0) {
      // Focus on to input when starting compose
      setTimeout(() => toInputRef.current?.focus(), 50);
      setToInputValue("");
      setInputValue("");
      setParticipantsConfirmed(false);
    } else if (isComposing && (participantsConfirmed || composeParticipants.length > 0)) {
      // Focus on message input when participants confirmed (new conversation)
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isComposing, participantsConfirmed, composeParticipants.length]);

  // Focus on message input when switching from compose mode to existing conversation
  useEffect(() => {
    if (wasComposingRef.current && !isComposing && conversation) {
      // We just switched from compose mode to an existing conversation
      setTimeout(() => inputRef.current?.focus(), 50);
    }
    wasComposingRef.current = isComposing || false;
    prevConversationRef.current = conversation;
  }, [isComposing, conversation]);

  // Reset compose state when exiting compose mode
  useEffect(() => {
    if (!isComposing) {
      setToInputValue("");
      setParticipantsConfirmed(false);
    }
  }, [isComposing]);

  // Handle Enter key in To field
  const handleToKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      const participants = toInputValue
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);

      if (participants.length > 0 && onComposeParticipantsConfirm) {
        setParticipantsConfirmed(true);
        onComposeParticipantsConfirm(participants);
      }
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (inputValue.trim()) {
      // Check if we're in compose mode with no existing conversation
      if (isComposing && !conversation && composeParticipants.length > 0 && onSendNewMessage) {
        onSendNewMessage(inputValue.trim(), composeParticipants);
        setInputValue("");
        setParticipantsConfirmed(false);
      } else if (conversation) {
        onSendMessage(inputValue.trim());
        setInputValue("");
      }
      inputRef.current?.focus();
    }
  };

  // Compose mode - show compose UI
  if (isComposing && !conversation) {
    const hasParticipants = composeParticipants.length > 0 || participantsConfirmed;

    return (
      <div className="conversation-view">
        {/* Compose Header */}
        <div className="conversation-header compose-header-mode">
          {onBack && (
            <button className="back-button" onClick={onBack}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M19 12H5M12 19l-7-7 7-7" />
              </svg>
            </button>
          )}

          {!hasParticipants ? (
            <div className="compose-to-field">
              <span className="compose-to-label">To:</span>
              <input
                ref={toInputRef}
                type="text"
                className="compose-to-input"
                placeholder="Enter email addresses (comma-separated)"
                value={toInputValue}
                onChange={(e) => setToInputValue(e.target.value)}
                onKeyDown={handleToKeyDown}
              />
            </div>
          ) : (
            <>
              <div className="header-avatar-group">
                {composeParticipants.slice(0, 2).map((participant, index) => (
                  <Avatar
                    key={index}
                    email={extractEmail(participant)}
                    name={participant}
                    size={40}
                    className={`header-avatar ${composeParticipants.length > 1 ? `header-avatar-stacked header-avatar-pos-${index}` : ''}`}
                  />
                ))}
              </div>
              <div className="header-info">
                <h2 className="header-name">{composeParticipants.join(", ")}</h2>
                <span className="header-status">New conversation</span>
              </div>
            </>
          )}
        </div>

        {/* Empty Messages Area */}
        <div className="conversation-messages">
          <div className="messages-empty compose-empty">
            {!hasParticipants ? (
              <p>Enter recipients and press Enter to start a new conversation</p>
            ) : (
              <p>Start your conversation</p>
            )}
          </div>
        </div>

        {/* Input - only show when participants are confirmed */}
        {hasParticipants && (
          <form className="conversation-input" onSubmit={handleSubmit}>
            <button type="button" className="input-action-btn" title="Add attachment">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
              </svg>
            </button>

            <div className="input-wrapper">
              <input
                ref={inputRef}
                type="text"
                className="message-input"
                placeholder="Type your message (first line becomes subject)"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
              />
              <button type="button" className="emoji-btn" title="Emoji">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <circle cx="12" cy="12" r="10" />
                  <path d="M8 14s1.5 2 4 2 4-2 4-2" />
                  <line x1="9" y1="9" x2="9.01" y2="9" />
                  <line x1="15" y1="9" x2="15.01" y2="9" />
                </svg>
              </button>
            </div>
          </form>
        )}
      </div>
    );
  }

  // Empty state - no conversation selected
  if (!conversation) {
    return (
      <div className="conversation-view conversation-empty">
        <div className="empty-state">
          <div className="empty-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
          </div>
          <h3>Select a conversation</h3>
          <p>Choose a conversation from the list to start messaging</p>
        </div>
      </div>
    );
  }

  const conversationName = getConversationName(conversation);
  const headerTooltip = getHeaderAvatarTooltip(conversation);

  // Get participants excluding the user for avatars
  const userEmail = currentAccountEmail?.toLowerCase() || extractEmail(conversation.user_name);

  // Map participants with their metadata, then filter
  const participantData = conversation.participants.map((p, idx) => ({
    participant: p,
    email: extractEmail(p),
    name: conversation.participant_names[idx] || extractEmail(p),
  }));

  const otherParticipantData = participantData.filter(pd => pd.email !== userEmail);

  // Limit to 2 avatars for header
  const headerAvatarsToShow = otherParticipantData.slice(0, 2);

  return (
    <div className="conversation-view">
      {/* Header */}
      <div className="conversation-header">
        {onBack && (
          <button className="back-button" onClick={onBack}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M19 12H5M12 19l-7-7 7-7" />
            </svg>
          </button>
        )}

        <div className="header-avatar-group" title={headerTooltip}>
          {headerAvatarsToShow.map((participantData, index) => (
            <Avatar
              key={index}
              email={participantData.email}
              name={participantData.name}
              size={40}
              className={`header-avatar ${headerAvatarsToShow.length > 1 ? `header-avatar-stacked header-avatar-pos-${index}` : ''}`}
            />
          ))}
        </div>

        <div className="header-info">
          <h2 className="header-name">{conversationName}</h2>
          <span className="header-status">
            {conversation.participant_names.length > 1
              ? `${conversation.participant_names.length} participants`
              : ""}
          </span>
        </div>

        <div className="header-actions">
          <button className="header-action-btn" title="Search">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="11" cy="11" r="8" />
              <path d="m21 21-4.35-4.35" />
            </svg>
          </button>
          <button className="header-action-btn" title="More options">
            <svg viewBox="0 0 24 24" fill="currentColor">
              <circle cx="12" cy="5" r="1.5" />
              <circle cx="12" cy="12" r="1.5" />
              <circle cx="12" cy="19" r="1.5" />
            </svg>
          </button>
        </div>
      </div>

      {/* Messages */}
      <div className="conversation-messages">
        {loading ? (
          <div className="messages-loading">
            <div className="loading-spinner" />
            <span>Loading messages...</span>
          </div>
        ) : error ? (
          <div className="messages-error">
            <span>Error loading messages: {error}</span>
          </div>
        ) : messages.length === 0 ? (
          <div className="messages-empty">
            <p>No messages yet. Start the conversation!</p>
          </div>
        ) : (
          <>
            {messages.map((message, index) => {
              const isOut = isOutgoing(message, currentAccountEmail);
              const showDateSeparator =
                index === 0 ||
                isDifferentDay(
                  messages[index - 1].envelope.date,
                  message.envelope.date
                );

              // Show sender name for incoming group messages
              const showSender =
                !isOut && conversation.participant_names.length > 2;

              return (
                <div key={message.id}>
                  {showDateSeparator && (
                    <div className="date-separator">
                      <span>{formatDateSeparator(message.envelope.date)}</span>
                    </div>
                  )}

                  <div
                    className={`message-bubble-container ${isOut ? "outgoing" : "incoming"
                      }`}
                  >
                    {!isOut && (
                      <div
                        className="message-avatar"
                        style={{
                          backgroundColor: getAvatarColor(
                            message.envelope.from
                          ),
                        }}
                        title={getAvatarTooltip(message.envelope.from, message.id)}
                      >
                        {(() => {
                          const messageEmail = extractEmail(message.envelope.from);
                          const messageGravatarUrl = messageEmail ? getGravatarUrl(messageEmail, 32) : null;
                          return (
                            <>
                              {messageGravatarUrl ? (
                                <img
                                  src={messageGravatarUrl}
                                  alt={getSenderName(message.envelope.from)}
                                  className="chat-avatar-img"
                                  onError={(e) => {
                                    const avatar = e.currentTarget.parentElement;
                                    if (avatar) {
                                      e.currentTarget.style.display = 'none';
                                      const initials = avatar.querySelector('.chat-avatar-initials');
                                      if (initials) {
                                        (initials as HTMLElement).style.display = 'block';
                                      }
                                    }
                                  }}
                                  onLoad={(e) => {
                                    const avatar = e.currentTarget.parentElement;
                                    if (avatar) {
                                      const initials = avatar.querySelector('.chat-avatar-initials');
                                      if (initials) {
                                        (initials as HTMLElement).style.display = 'none';
                                      }
                                    }
                                  }}
                                />
                              ) : null}
                              <span className="chat-avatar-initials">{getInitials(message.envelope.from)}</span>
                            </>
                          );
                        })()}
                      </div>
                    )}

                    <div className="message-bubble-wrapper">
                      {showSender && (
                        <span className="message-sender">
                          {getSenderName(message.envelope.from)}
                        </span>
                      )}

                      <div className={`message-bubble ${isOut ? "sent" : "received"}`}>
                        <div className="message-text">
                          {parseEmailContent(message.text_body) || message.envelope.subject || "(No content)"}
                        </div>
                        <span className="message-time">
                          {formatMessageTime(message.envelope.date)}
                          {isOut && (
                            <span className="message-status">
                              <svg viewBox="0 0 24 24" fill="currentColor">
                                <path d="M18 7l-8.5 8.5-4-4L4 13l5.5 5.5L19.5 8.5z" />
                              </svg>
                            </span>
                          )}
                        </span>
                      </div>
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
      <form className="conversation-input" onSubmit={handleSubmit}>
        <button type="button" className="input-action-btn" title="Add attachment">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
          </svg>
        </button>

        <div className="input-wrapper">
          <input
            ref={inputRef}
            type="text"
            className="message-input"
            placeholder="Message"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
          />
          <button type="button" className="emoji-btn" title="Emoji">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" />
              <path d="M8 14s1.5 2 4 2 4-2 4-2" />
              <line x1="9" y1="9" x2="9.01" y2="9" />
              <line x1="15" y1="9" x2="15.01" y2="9" />
            </svg>
          </button>
        </div>
      </form>
    </div>
  );
}
