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
    return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  } else if (diffDays === 1) {
    return "Yesterday";
  } else if (diffDays < 7) {
    return date.toLocaleDateString([], { weekday: "short" });
  } else {
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
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Search */}
      <div className="px-3 pb-3 safe-x">
        <div className="relative flex items-center">
          <svg className="absolute left-3 w-4 h-4 text-text-muted pointer-events-none" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.35-4.35" />
          </svg>
          <input
            type="text"
            className="w-full py-2.5 pl-10 pr-3 bg-bg-tertiary border-none rounded-xl text-text-primary text-[15px] outline-none focus:bg-bg-hover transition-colors placeholder:text-text-muted"
            placeholder="Search"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
          />
        </div>
        <div className="flex gap-2 mt-3">
          <button
            className={`px-3.5 py-2 rounded-full text-sm font-medium transition-all ${
              activeFilter === "chats"
                ? "bg-white text-bg-primary"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover hover:text-text-primary"
            }`}
            onClick={() => setActiveFilter("chats")}
          >
            Connections
          </button>
          <button
            className={`px-3.5 py-2 rounded-full text-sm font-medium transition-all ${
              activeFilter === "requests"
                ? "bg-white text-bg-primary"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover hover:text-text-primary"
            }`}
            onClick={() => setActiveFilter("requests")}
          >
            Strangers
          </button>
          <button
            className={`px-3.5 py-2 rounded-full text-sm font-medium transition-all ${
              activeFilter === "all"
                ? "bg-white text-bg-primary"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover hover:text-text-primary"
            }`}
            onClick={() => setActiveFilter("all")}
          >
            All
          </button>
        </div>
      </div>

      {/* Chat list content */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden">
        {loading ? (
          <div className="flex flex-col items-center justify-center py-10 gap-3 text-text-muted text-sm">
            <div className="spinner" />
            <span>Loading conversations...</span>
          </div>
        ) : filteredConversations.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 text-text-muted text-sm">
            {searchQuery ? "No conversations found" : "No conversations yet"}
          </div>
        ) : (
          filteredConversations.map((conversation) => {
            const nameParts = getConversationNameParts(conversation);
            const isSelected = selectedId === conversation.id;
            const avatarTooltip = getAvatarTooltip(conversation);

            const userEmail = currentAccountEmail?.toLowerCase() || extractEmail(conversation.user_name);

            const participantData = conversation.participants.map((p, idx) => ({
              participant: p,
              email: extractEmail(p),
              name: conversation.participant_names[idx] || extractEmail(p),
            }));

            const otherParticipantData = participantData.filter(pd => pd.email !== userEmail);
            const avatarsToShow = otherParticipantData.slice(0, 2);

            return (
              <div
                key={conversation.id}
                className={`flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors safe-x ${
                  isSelected ? "bg-bg-active" : "hover:bg-bg-hover"
                }`}
                onClick={() => onSelect(conversation)}
              >
                {/* Avatar group */}
                <div className="w-12 h-12 min-w-12 relative flex items-center" title={avatarTooltip}>
                  {avatarsToShow.map((pd, index) => {
                    const avatarColor = getAvatarColor(pd.email || pd.name);
                    const initials = getInitials(pd.name);
                    const gravatarUrl = pd.email ? getGravatarUrl(pd.email, 48) : null;

                    return (
                      <div
                        key={index}
                        className={`flex items-center justify-center rounded-full text-white font-semibold uppercase overflow-hidden relative ${
                          avatarsToShow.length > 1
                            ? `w-8 h-8 min-w-8 text-xs border-2 border-bg-secondary absolute ${
                                index === 0 ? "left-0 z-20" : "left-4 z-10"
                              }`
                            : "w-12 h-12 min-w-12 text-lg"
                        }`}
                        style={{ backgroundColor: avatarColor }}
                      >
                        {gravatarUrl && (
                          <img
                            src={gravatarUrl}
                            alt={pd.name}
                            className="absolute inset-0 w-full h-full object-cover rounded-full"
                            onError={(e) => {
                              e.currentTarget.style.display = 'none';
                              const initials = e.currentTarget.parentElement?.querySelector('.avatar-initials');
                              if (initials) (initials as HTMLElement).style.display = 'block';
                            }}
                            onLoad={(e) => {
                              const initials = e.currentTarget.parentElement?.querySelector('.avatar-initials');
                              if (initials) (initials as HTMLElement).style.display = 'none';
                            }}
                          />
                        )}
                        <span className="avatar-initials relative z-0">{initials}</span>
                      </div>
                    );
                  })}
                </div>

                {/* Content */}
                <div className="flex-1 min-w-0 flex flex-col gap-1">
                  <div className="flex justify-between items-center gap-2">
                    <span className={`text-base text-text-primary truncate ${
                      conversation.unread_count > 0 ? "font-semibold" : "font-medium"
                    }`}>
                      {nameParts.map((part, index) => (
                        <span key={index}>
                          {index > 0 && ", "}
                          <span className={part.isUser ? "opacity-50" : ""}>
                            {part.name}
                          </span>
                        </span>
                      ))}
                    </span>
                    <span className="text-[13px] text-text-muted whitespace-nowrap shrink-0">
                      {formatTime(conversation.last_message_date)}
                    </span>
                  </div>

                  <div className="flex justify-between items-center gap-2">
                    <span className={`text-sm truncate ${
                      conversation.unread_count > 0
                        ? "text-text-primary font-medium"
                        : "text-text-secondary"
                    }`}>
                      {conversation.last_message_preview}
                    </span>
                    {conversation.unread_count > 0 && (
                      <span className="min-w-5 h-5 px-1.5 rounded-full bg-accent-blue text-white text-xs font-semibold flex items-center justify-center shrink-0">
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
