import type { Message } from "../types";

interface MessageFullViewProps {
  message: Message;
  onClose: () => void;
}

// Get sender name from email
function getSenderName(from: string): string {
  const cleanName = from.replace(/<[^>]+>/g, "").trim();
  if (!cleanName || cleanName.includes("@")) {
    const match = from.match(/<([^>]+)>/);
    const email = match ? match[1] : from;
    return email.split("@")[0];
  }
  return cleanName;
}

// Format time for the header
function formatFullViewTime(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function MessageFullView({ message, onClose }: MessageFullViewProps) {
  const senderName = getSenderName(message.envelope.from);
  const hasHtml = message.html_body && message.html_body.trim().length > 0;

  return (
    <div className="message-full-view">
      <div className="message-full-view-header">
        <div className="message-full-view-info">
          <h3 className="message-full-view-subject">
            {message.envelope.subject || "(No subject)"}
          </h3>
          <div className="message-full-view-meta">
            <span className="message-full-view-sender">{senderName}</span>
            <span className="message-full-view-date">
              {formatFullViewTime(message.envelope.date)}
            </span>
          </div>
        </div>
        <button
          className="message-full-view-close"
          onClick={onClose}
          title="Close"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="message-full-view-content">
        {hasHtml ? (
          <div
            className="message-full-view-html"
            dangerouslySetInnerHTML={{ __html: message.html_body! }}
          />
        ) : (
          <div className="message-full-view-text">
            {message.text_body || "(No content)"}
          </div>
        )}
      </div>
    </div>
  );
}
