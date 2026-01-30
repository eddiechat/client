import type { Message } from "../types";

interface MessageFullViewProps {
  message: Message;
  onClose: () => void;
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
    <div className="flex-1 flex flex-col bg-bg-secondary overflow-hidden">
      <div className="flex items-start justify-between gap-4 p-4 border-b border-divider">
        <div className="flex-1 min-w-0">
          <h3 className="text-lg font-semibold text-text-primary leading-tight mb-1">
            {message.envelope.subject || "(No subject)"}
          </h3>
          <div className="flex items-center gap-2 text-sm text-text-muted">
            <span className="font-medium text-text-secondary">{senderName}</span>
            <span>·</span>
            <span>{formatFullViewTime(message.envelope.date)}</span>
          </div>
        </div>
        <button
          className="w-8 h-8 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors shrink-0"
          onClick={onClose}
          title="Close"
        >
          <svg className="w-5 h-5 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-4 safe-bottom">
        {hasHtml ? (
          <div
            className="prose prose-invert prose-sm max-w-none [&_a]:text-accent-blue [&_img]:max-w-full [&_img]:h-auto"
            dangerouslySetInnerHTML={{ __html: message.html_body! }}
          />
        ) : (
          <div className="text-[15px] text-text-primary whitespace-pre-wrap leading-relaxed">
            {message.text_body || "(No content)"}
          </div>
        )}
      </div>
    </div>
  );
}
