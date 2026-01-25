import { useState } from "react";
import type { ComposeMessageData } from "../types";

interface ComposeModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSend: (data: ComposeMessageData) => Promise<void>;
  onSaveDraft: (data: ComposeMessageData) => Promise<void>;
  initialData?: Partial<ComposeMessageData>;
  mode?: "new" | "reply" | "forward";
}

export function ComposeModal({
  isOpen,
  onClose,
  onSend,
  onSaveDraft,
  initialData,
  mode = "new",
}: ComposeModalProps) {
  const [to, setTo] = useState(initialData?.to?.join(", ") || "");
  const [cc, setCc] = useState(initialData?.cc?.join(", ") || "");
  const [subject, setSubject] = useState(initialData?.subject || "");
  const [body, setBody] = useState(initialData?.body || "");
  const [sending, setSending] = useState(false);

  if (!isOpen) return null;

  const parseAddresses = (input: string): string[] =>
    input
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);

  const handleSend = async () => {
    setSending(true);
    try {
      await onSend({
        to: parseAddresses(to),
        cc: parseAddresses(cc),
        subject,
        body,
        in_reply_to: initialData?.in_reply_to,
      });
      onClose();
    } finally {
      setSending(false);
    }
  };

  const handleSaveDraft = async () => {
    await onSaveDraft({
      to: parseAddresses(to),
      cc: parseAddresses(cc),
      subject,
      body,
      in_reply_to: initialData?.in_reply_to,
    });
  };

  const getTitle = () => {
    switch (mode) {
      case "reply":
        return "Reply";
      case "forward":
        return "Forward";
      default:
        return "New Message";
    }
  };

  return (
    <div className="compose-modal-overlay">
      <div className="compose-modal">
        <div className="compose-header">
          <h2>{getTitle()}</h2>
          <button className="close-btn" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="compose-form">
          <div className="form-row">
            <label htmlFor="to">To:</label>
            <input
              id="to"
              type="text"
              value={to}
              onChange={(e) => setTo(e.target.value)}
              placeholder="recipient@example.com"
            />
          </div>

          <div className="form-row">
            <label htmlFor="cc">Cc:</label>
            <input
              id="cc"
              type="text"
              value={cc}
              onChange={(e) => setCc(e.target.value)}
              placeholder="cc@example.com"
            />
          </div>

          <div className="form-row">
            <label htmlFor="subject">Subject:</label>
            <input
              id="subject"
              type="text"
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
            />
          </div>

          <div className="form-row body-row">
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder="Write your message..."
            />
          </div>
        </div>

        <div className="compose-footer">
          <button type="button" onClick={handleSaveDraft} disabled={sending} className="cancel-btn">
            Save Draft
          </button>
          <button type="button" onClick={handleSend} disabled={sending} className="send-btn">
            {sending ? "Sending..." : "Send"}
          </button>
        </div>
      </div>
    </div>
  );
}
