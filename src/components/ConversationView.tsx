import { useState, useRef, useEffect } from "react";
import type { Conversation, Message } from "../types";
import md5 from "md5";

interface ConversationViewProps {
  conversation: Conversation | null;
  messages: Message[];
  loading?: boolean;
  error?: string | null;
  currentAccountEmail?: string;
  onSendMessage: (text: string) => void;
  onBack?: () => void;
}

// Generate a consistent color from a string
function getAvatarColor(name: string): string {
  const colors = [
    "#e91e63",
    "#9c27b0",
    "#673ab7",
    "#3f51b5",
    "#2196f3",
    "#03a9f4",
    "#00bcd4",
    "#009688",
    "#4caf50",
    "#8bc34a",
    "#ff9800",
    "#ff5722",
  ];

  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

// Get initials from a name
function getInitials(name: string): string {
  const cleanName = name.replace(/<[^>]+>/g, "").trim();
  if (!cleanName) return "?";

  if (cleanName.includes("@")) {
    return cleanName.split("@")[0].charAt(0).toUpperCase();
  }

  const parts = cleanName.split(/\s+/).filter(Boolean);
  if (parts.length === 1) {
    return parts[0].charAt(0).toUpperCase();
  }

  return (parts[0].charAt(0) + parts[parts.length - 1].charAt(0)).toUpperCase();
}

// Extract email from a participant string (handles both "Name <email>" and "email" formats)
function extractEmail(participant: string): string {
  const match = participant.match(/<([^>]+)>/);
  if (match) {
    return match[1].trim().toLowerCase();
  }
  return participant.trim().toLowerCase();
}

// Generate Gravatar URL from email
function getGravatarUrl(email: string, size: number = 40): string {
  const hash = md5(email.trim().toLowerCase());
  return `https://www.gravatar.com/avatar/${hash}?s=${size}&d=404`;
}

// Get the primary email for a conversation (first non-user participant)
function getPrimaryEmail(conversation: Conversation): string | null {
  if (conversation.participants.length === 0) {
    return null;
  }

  // If user is in conversation and there are other participants, use the second one
  if (conversation.user_in_conversation && conversation.participants.length > 1) {
    return extractEmail(conversation.participants[1]);
  }

  // Otherwise use the first participant
  return extractEmail(conversation.participants[0]);
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

// Extract first name from a full name
function getFirstName(name: string): string {
  // Remove any email parts first
  const cleanName = name.replace(/<[^>]+>/g, "").trim();
  if (!cleanName || cleanName.includes("@")) {
    // It's an email address, use username part
    const email = cleanName || name;
    return email.split("@")[0];
  }
  // Return the first word (first name)
  return cleanName.split(/\s+/)[0];
}

// Get tooltip text for avatar (name and email)
function getAvatarTooltip(from: string): string {
  const name = getSenderName(from);
  const emailMatch = from.match(/<([^>]+)>/);
  const email = emailMatch ? emailMatch[1] : from.replace(/^[^<]*/, "").trim();

  if (name && email && name !== email && !name.includes("@")) {
    return `${name} <${email}>`;
  }
  return email || from;
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

// Get display name parts for conversation (first names, with user marked)
function getConversationNameParts(conversation: Conversation): { name: string; isUser: boolean }[] {
  if (conversation.participant_names.length === 0) {
    return [{ name: "Unknown", isUser: false }];
  }

  const userFirstName = getFirstName(conversation.user_name).toLowerCase();
  const parts: { name: string; isUser: boolean }[] = [];

  if (conversation.user_in_conversation && conversation.participant_names.length > 1) {
    // User is in the conversation - add them first (faded), then others
    parts.push({ name: userFirstName, isUser: true });

    // Add other participants (skip index 0 which is the user)
    for (let i = 1; i < conversation.participant_names.length && parts.length < 3; i++) {
      const firstName = getFirstName(conversation.participant_names[i]);
      parts.push({ name: firstName, isUser: false });
    }

    // Handle more than 3 participants
    if (conversation.participant_names.length > 3) {
      const remaining = conversation.participant_names.length - 3;
      parts.push({ name: `+${remaining}`, isUser: false });
    }
  } else {
    // User is not in this conversation - just show the participants
    for (let i = 0; i < conversation.participant_names.length && parts.length < 2; i++) {
      const firstName = getFirstName(conversation.participant_names[i]);
      parts.push({ name: firstName, isUser: false });
    }

    // Handle more than 2 participants
    if (conversation.participant_names.length > 2) {
      const remaining = conversation.participant_names.length - 2;
      parts.push({ name: `+${remaining}`, isUser: false });
    }
  }

  return parts;
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
}: ConversationViewProps) {
  const [inputValue, setInputValue] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Scroll to bottom when messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (inputValue.trim()) {
      onSendMessage(inputValue.trim());
      setInputValue("");
      inputRef.current?.focus();
    }
  };

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
  const nameParts = getConversationNameParts(conversation);
  const primaryEmail = getPrimaryEmail(conversation);

  // Use same logic as sidebar: color/initials based on other participants (not user)
  const otherPartsName = nameParts.filter(p => !p.isUser).map(p => p.name).join(", ") || conversationName;
  const avatarColor = getAvatarColor(otherPartsName);
  const initials = getInitials(otherPartsName);
  const headerTooltip = getHeaderAvatarTooltip(conversation);
  const headerGravatarUrl = primaryEmail ? getGravatarUrl(primaryEmail, 40) : null;

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

        <div className="header-avatar" style={{ backgroundColor: avatarColor }} title={headerTooltip}>
          {headerGravatarUrl ? (
            <img
              src={headerGravatarUrl}
              alt={conversationName}
              className="chat-avatar-img"
              onError={(e) => {
                // On error, hide image and show initials
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
                // On success, hide initials
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
          <span className="chat-avatar-initials">{initials}</span>
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
          <button className="header-action-btn" title="Video call">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="m22 8-6 4 6 4V8Z" />
              <rect x="2" y="6" width="14" height="12" rx="2" ry="2" />
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
                    className={`message-bubble-container ${
                      isOut ? "outgoing" : "incoming"
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
                        title={getAvatarTooltip(message.envelope.from)}
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
                                    // On error, hide image and show initials
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
                                    // On success, hide initials
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
                        {message.envelope.subject && (
                          <div className="message-subject">
                            {message.envelope.subject}
                          </div>
                        )}
                        <div className="message-text">
                          {message.text_body || "(No content)"}
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

        {inputValue.trim() ? (
          <button type="submit" className="send-btn" title="Send">
            <svg viewBox="0 0 24 24" fill="currentColor">
              <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
            </svg>
          </button>
        ) : (
          <button type="button" className="mic-btn" title="Voice message">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 1a3 3 0 00-3 3v8a3 3 0 006 0V4a3 3 0 00-3-3z" />
              <path d="M19 10v2a7 7 0 01-14 0v-2" />
              <line x1="12" y1="19" x2="12" y2="23" />
              <line x1="8" y1="23" x2="16" y2="23" />
            </svg>
          </button>
        )}
      </form>
    </div>
  );
}
