import type { Conversation } from "../types";

interface ChatListProps {
  conversations: Conversation[];
  selectedId: string | null;
  onSelect: (conversation: Conversation) => void;
  loading?: boolean;
  searchQuery: string;
  onSearchChange: (query: string) => void;
}

// Generate a consistent color from a string (name/email)
function getAvatarColor(name: string): string {
  const colors = [
    "#e91e63", // pink
    "#9c27b0", // purple
    "#673ab7", // deep purple
    "#3f51b5", // indigo
    "#2196f3", // blue
    "#03a9f4", // light blue
    "#00bcd4", // cyan
    "#009688", // teal
    "#4caf50", // green
    "#8bc34a", // light green
    "#ff9800", // orange
    "#ff5722", // deep orange
  ];

  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

// Get initials from a name or email
function getInitials(name: string): string {
  // Clean up the name (remove email parts if present)
  const cleanName = name.replace(/<[^>]+>/g, "").trim();

  if (!cleanName) return "?";

  // If it's an email address, use first letter of username
  if (cleanName.includes("@")) {
    return cleanName.split("@")[0].charAt(0).toUpperCase();
  }

  // Get initials from name parts
  const parts = cleanName.split(/\s+/).filter(Boolean);
  if (parts.length === 1) {
    return parts[0].charAt(0).toUpperCase();
  }

  return (parts[0].charAt(0) + parts[parts.length - 1].charAt(0)).toUpperCase();
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

// Get plain display name for conversation (for avatar)
function getConversationName(conversation: Conversation): string {
  const parts = getConversationNameParts(conversation);
  return parts.map(p => p.name).join(", ");
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
}: ChatListProps) {
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
            const displayName = getConversationName(conversation);
            const nameParts = getConversationNameParts(conversation);
            // Use display name without user for avatar color/initials (skip user part)
            const otherPartsName = nameParts.filter(p => !p.isUser).map(p => p.name).join(", ") || displayName;
            const avatarColor = getAvatarColor(otherPartsName);
            const initials = getInitials(otherPartsName);
            const isSelected = selectedId === conversation.id;
            const avatarTooltip = getAvatarTooltip(conversation);

            return (
              <div
                key={conversation.id}
                className={`chat-item ${isSelected ? "selected" : ""} ${
                  conversation.unread_count > 0 ? "unread" : ""
                }`}
                onClick={() => onSelect(conversation)}
              >
                <div
                  className="chat-avatar"
                  style={{ backgroundColor: avatarColor }}
                  title={avatarTooltip}
                >
                  {initials}
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
