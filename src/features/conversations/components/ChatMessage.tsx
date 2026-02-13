import type { Conversation } from "../../../tauri";
import { extractEmail } from "../../../shared";
import { Avatar } from "../../../shared/components";
import { getConversationNameParts } from "../utils";

interface ChatMessageProps {
  conversation: Conversation;
  isSelected: boolean;
  onSelect: (conversation: Conversation) => void;
  currentAccountEmail?: string;
}

function formatTime(dateMs: number): string {
  const date = new Date(dateMs);
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

function getAvatarTooltip(conversation: Conversation): string {
  return conversation.participants
    .map((email, index) => {
      const name = conversation.participant_display_names[index];
      if (name && name !== email && !name.includes("@")) {
        return `${name} <${email}>`;
      }
      return email;
    })
    .join("\n");
}

export function ChatMessage({
  conversation,
  isSelected,
  onSelect,
  currentAccountEmail,
}: ChatMessageProps) {
  const nameParts = getConversationNameParts(conversation);
  const avatarTooltip = getAvatarTooltip(conversation);

  const participantData = conversation.participants.map((p, idx) => ({
    participant: p,
    email: extractEmail(p),
    name: conversation.participant_display_names[idx] || extractEmail(p),
  }));

  // Filter out the current user from sidebar avatars (only by email, not by name)
  const externalParticipants = currentAccountEmail
    ? participantData.filter(
        (pd) => pd.email.toLowerCase() !== currentAccountEmail.toLowerCase()
      )
    : participantData;

  // Show up to 3 avatars like the header does
  const avatarsToShow = externalParticipants.slice(0, 3);

  // Calculate width for avatar container
  // Single avatar: 48px, Multiple avatars: overlap at 21px each
  // Formula: first avatar (32px) + (n-1) * overlap (21px) + final avatar width minus overlap (11px)
  const avatarContainerWidth =
    avatarsToShow.length === 1 ? 48 : 32 + (avatarsToShow.length - 1) * 21 + 11;

  return (
    <div
      className={`flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors safe-x ${
        isSelected ? "bg-bg-active" : "hover:bg-bg-hover"
      }`}
      onClick={() => onSelect(conversation)}
    >
      {/* Avatar group */}
      <div
        className="h-12 min-w-12 relative flex items-center"
        style={{ width: `${avatarContainerWidth}px` }}
        title={avatarTooltip}
      >
        {avatarsToShow.map((pd, index) => (
          <div
            key={index}
            className={avatarsToShow.length > 1 ? "absolute" : ""}
            style={
              avatarsToShow.length > 1
                ? {
                    left: `${index * 21}px`,
                    zIndex: 20 - index,
                  }
                : undefined
            }
          >
            <Avatar
              email={pd.email}
              name={pd.name}
              size={avatarsToShow.length > 1 ? 32 : 48}
              className={
                avatarsToShow.length > 1 ? "border-2 border-bg-secondary" : ""
              }
            />
          </div>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 flex flex-col gap-1">
        <div className="flex justify-between items-center gap-2">
          <span
            className={`text-base text-text-primary truncate ${
              conversation.unread_count > 0 ? "font-semibold" : "font-medium"
            }`}
          >
            {nameParts.map((part, index) => (
              <span key={index}>
                {index > 0 && ", "}
                <span className={part.isUser ? "opacity-50" : ""}>{part.name}</span>
              </span>
            ))}
          </span>
          <span className="text-[13px] text-text-muted whitespace-nowrap shrink-0">
            {formatTime(conversation.last_message_date)}
          </span>
        </div>

        <div className="flex justify-between items-center gap-2">
          <span
            className={`text-sm truncate ${
              conversation.unread_count > 0
                ? "text-text-primary font-medium"
                : "text-text-secondary"
            }`}
          >
            {conversation.last_message_preview || ""}
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
}
