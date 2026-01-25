import type { Envelope } from "../types";
import { FLAGS } from "../types";

interface EnvelopeListProps {
  envelopes: Envelope[];
  selectedId: string | null;
  onSelect: (envelope: Envelope) => void;
  onToggleFlag: (id: string, isFlagged: boolean) => void;
  loading?: boolean;
}

export function EnvelopeList({
  envelopes,
  selectedId,
  onSelect,
  onToggleFlag,
  loading,
}: EnvelopeListProps) {
  if (loading) {
    return <div className="envelope-list loading">Loading emails...</div>;
  }

  if (envelopes.length === 0) {
    return <div className="envelope-list empty">No emails in this folder</div>;
  }

  const formatDate = (dateStr: string) => {
    try {
      const date = new Date(dateStr);
      const now = new Date();
      const isToday = date.toDateString() === now.toDateString();

      if (isToday) {
        return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      }

      const isThisYear = date.getFullYear() === now.getFullYear();
      if (isThisYear) {
        return date.toLocaleDateString([], { month: "short", day: "numeric" });
      }

      return date.toLocaleDateString([], {
        year: "numeric",
        month: "short",
        day: "numeric",
      });
    } catch {
      return dateStr;
    }
  };

  const isRead = (envelope: Envelope) =>
    envelope.flags.some((f) => f.toLowerCase() === FLAGS.SEEN);

  const isFlagged = (envelope: Envelope) =>
    envelope.flags.some((f) => f.toLowerCase() === FLAGS.FLAGGED);

  return (
    <div className="envelope-list">
      {envelopes.map((envelope) => (
        <div
          key={envelope.id}
          className={`envelope-item ${selectedId === envelope.id ? "selected" : ""} ${
            !isRead(envelope) ? "unread" : ""
          }`}
          onClick={() => onSelect(envelope)}
        >
          <button
            className={`flag-btn ${isFlagged(envelope) ? "flagged" : ""}`}
            onClick={(e) => {
              e.stopPropagation();
              onToggleFlag(envelope.id, isFlagged(envelope));
            }}
          >
            {isFlagged(envelope) ? "⭐" : "☆"}
          </button>
          <div className="envelope-content">
            <div className="envelope-header">
              <span className="envelope-from">{envelope.from}</span>
              <span className="envelope-date">{formatDate(envelope.date)}</span>
            </div>
            <div className="envelope-subject">
              {envelope.has_attachment && <span className="attachment-icon">📎</span>}
              {envelope.subject || "(no subject)"}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
