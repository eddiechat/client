import type { Message } from "../types";

interface MessageViewProps {
  message: Message | null;
  loading?: boolean;
  error?: string | null;
  onClose: () => void;
  onDelete: () => void;
  onReply: () => void;
  onForward: () => void;
  onDownloadAttachments: () => void;
}

export function MessageView({
  message,
  loading,
  error,
  onClose,
  onDelete,
  onReply,
  onForward,
  onDownloadAttachments,
}: MessageViewProps) {
  if (loading) {
    return (
      <div className="message-view loading">
        <div className="loading-spinner">Loading message...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="message-view error">
        <p>Error loading message: {error}</p>
        <button onClick={onClose}>Close</button>
      </div>
    );
  }

  if (!message) {
    return (
      <div className="message-view empty">
        <p>Select an email to read</p>
      </div>
    );
  }

  const getHeader = (name: string) =>
    message.headers.find(([h]) => h.toLowerCase() === name.toLowerCase())?.[1];

  return (
    <div className="message-view">
      <div className="message-toolbar">
        <button onClick={onClose}>← Back</button>
        <div className="toolbar-actions">
          <button onClick={onReply}>Reply</button>
          <button onClick={onForward}>Forward</button>
          <button onClick={onDelete} className="delete-btn">
            Delete
          </button>
        </div>
      </div>

      <div className="message-header">
        <h2 className="message-subject">{message.envelope.subject || "(no subject)"}</h2>
        <div className="message-meta">
          <div className="message-from">
            <strong>From:</strong> {getHeader("From") || message.envelope.from}
          </div>
          <div className="message-to">
            <strong>To:</strong> {getHeader("To") || message.envelope.to.join(", ")}
          </div>
          <div className="message-date">
            <strong>Date:</strong> {getHeader("Date") || message.envelope.date}
          </div>
        </div>
      </div>

      {message.attachments.length > 0 && (
        <div className="message-attachments">
          <strong>Attachments ({message.attachments.length}):</strong>
          <ul>
            {message.attachments.map((att, i) => (
              <li key={i}>
                📎 {att.filename || "Unnamed"} ({(att.size / 1024).toFixed(1)} KB)
              </li>
            ))}
          </ul>
          <button onClick={onDownloadAttachments}>Download All</button>
        </div>
      )}

      <div className="message-body">
        {message.html_body ? (
          <iframe
            srcDoc={message.html_body}
            title="Email content"
            sandbox="allow-same-origin"
          />
        ) : (
          <pre>{message.text_body}</pre>
        )}
      </div>
    </div>
  );
}
