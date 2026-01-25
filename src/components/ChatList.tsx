import { useState } from "react";
import type { Conversation } from "../types";
import {
  getAvatarColor,
  getInitials,
  extractEmail,
  getGravatarUrl,
  getConversationNameParts,
} from "../lib/utils";

type FilterType = "chats" | "important" | "requests" | "all";

interface ChatListProps {
  conversations: Conversation[];
  selectedId: string | null;
  onSelect: (conversation: Conversation) => void;
  loading?: boolean;
  searchQuery: string;
  onSearchChange: (query: string) => void;
  currentAccountEmail?: string;
}

// Format date for display
function formatTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) {
    // Today - show time
    return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  } else if (diffDays === 1) {
    return "Yesterday";
  } else if (diffDays < 7) {
    // This week - show day name
    return date.toLocaleDateString([], { weekday: "short" });
  } else {
    // Older - show date
    return date.toLocaleDateString([], { month: "short", day: "numeric" });
  }
}

// Get tooltip text showing full names and emails
function getAvatarTooltip(conversation: Conversation): string {
  return conversation.participants.map((email, index) => {
    const name = conversation.participant_names[index];
    if (name && name !== email && !name.includes("@")) {
      return `${name} <${email}>`;
    }
    return email;
  }).join("\n");
}

export function ChatList({
  conversations,
  selectedId,
  onSelect,
  loading,
  searchQuery,
  onSearchChange,
  currentAccountEmail,
}: ChatListProps) {
  const [activeFilter, setActiveFilter] = useState<FilterType>("chats");

  // Filter conversations by search query
  const filteredConversations = searchQuery
    ? conversations.filter((conv) => {
      const searchLower = searchQuery.toLowerCase();
      return (
        conv.participant_names.some((name) =>
          name.toLowerCase().includes(searchLower)
        ) ||
        conv.last_message_preview.toLowerCase().includes(searchLower)
      );
    })
    : conversations;

  return (
    <div className="chat-list">
      <div className="chat-list-header">
        <h2>Messages</h2>
      </div>

      <div className="search-container">
        <div className="search-wrapper">
          <svg className="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.35-4.35" />
          </svg>
          <input
            type="text"
            className="search-input"
            placeholder="Search"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
          />
        </div>
        <div className="filter-badges">
          <button
            className={`filter-badge ${activeFilter === "chats" ? "active" : ""}`}
            onClick={() => setActiveFilter("chats")}
          >
            Connections
          </button>
          {/* <button
            className={`filter-badge ${activeFilter === "important" ? "active" : ""}`}
            onClick={() => setActiveFilter("important")}
          >
            Important
          </button> */}
          <button
            className={`filter-badge ${activeFilter === "requests" ? "active" : ""}`}
            onClick={() => setActiveFilter("requests")}
          >
            Strangers
          </button>
          <button
            className={`filter-badge ${activeFilter === "all" ? "active" : ""}`}
            onClick={() => setActiveFilter("all")}
          >
            All
          </button>
        </div>
      </div>

      <div className="chat-list-content">
        {loading ? (
          <div className="chat-list-loading">
            <div className="loading-spinner" />
            <span>Loading conversations...</span>
          </div>
        ) : filteredConversations.length === 0 ? (
          <div className="chat-list-empty">
            {searchQuery ? "No conversations found" : "No conversations yet"}
          </div>
        ) : (
          filteredConversations.map((conversation) => {
            const nameParts = getConversationNameParts(conversation);
            const isSelected = selectedId === conversation.id;
            const avatarTooltip = getAvatarTooltip(conversation);

            // Get participants excluding the user for avatars
            const userEmail = currentAccountEmail?.toLowerCase() || extractEmail(conversation.user_name);

            // Map participants with their metadata, then filter
            const participantData = conversation.participants.map((p, idx) => ({
              participant: p,
              email: extractEmail(p),
              name: conversation.participant_names[idx] || extractEmail(p),
            }));

            const otherParticipantData = participantData.filter(pd => pd.email !== userEmail);

            // Limit to 2 avatars for cleaner display
            const avatarsToShow = otherParticipantData.slice(0, 2);

            return (
              <div
                key={conversation.id}
                className={`chat-item ${isSelected ? "selected" : ""} ${conversation.unread_count > 0 ? "unread" : ""
                  }`}
                onClick={() => onSelect(conversation)}
              >
                <div className="chat-avatar-group" title={avatarTooltip}>
                  {avatarsToShow.map((participantData, index) => {
                    const { email, name } = participantData;
                    const avatarColor = getAvatarColor(email || name);
                    const initials = getInitials(name);
                    const gravatarUrl = email ? getGravatarUrl(email, 48) : null;

                    return (
                      <div
                        key={index}
                        className={`chat-avatar ${avatarsToShow.length > 1 ? `chat-avatar-stacked chat-avatar-pos-${index}` : ''}`}
                        style={{ backgroundColor: avatarColor }}
                      >
                        {gravatarUrl ? (
                          <img
                            src={gravatarUrl}
                            alt={name}
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
                        <span className="chat-avatar-initials">{initials}</span>
                      </div>
                    );
                  })}
                </div>

                <div className="chat-content">
                  <div className="chat-header-row">
                    <span className="chat-name">
                      {nameParts.map((part, index) => (
                        <span key={index}>
                          {index > 0 && ", "}
                          <span style={part.isUser ? { opacity: 0.5 } : undefined}>
                            {part.name}
                          </span>
                        </span>
                      ))}
                    </span>
                    <span className="chat-time">
                      {formatTime(conversation.last_message_date)}
                    </span>
                  </div>

                  <div className="chat-preview-row">
                    <span className="chat-preview">
                      {conversation.last_message_preview}
                    </span>
                    {conversation.unread_count > 0 && (
                      <span className="unread-badge">
                        {conversation.unread_count}
                      </span>
                    )}
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
