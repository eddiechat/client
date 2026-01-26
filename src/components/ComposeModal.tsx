import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { ComposeMessageData, ComposeAttachment } from "../types";

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
  const [attachments, setAttachments] = useState<ComposeAttachment[]>([]);
  const [sending, setSending] = useState(false);

  if (!isOpen) return null;

  const parseAddresses = (input: string): string[] =>
    input
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);

  const handleAddAttachment = async () => {
    try {
      const selected = await open({
        multiple: true,
        title: "Select files to attach",
      });

      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
        const newAttachments: ComposeAttachment[] = files.map((filePath) => {
          const fileName = filePath.split(/[/\\]/).pop() || "attachment";
          const extension = fileName.split(".").pop()?.toLowerCase() || "";

          const mimeTypes: Record<string, string> = {
            pdf: "application/pdf",
            doc: "application/msword",
            docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            xls: "application/vnd.ms-excel",
            xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            txt: "text/plain",
            csv: "text/csv",
            html: "text/html",
            json: "application/json",
            zip: "application/zip",
            png: "image/png",
            jpg: "image/jpeg",
            jpeg: "image/jpeg",
            gif: "image/gif",
            webp: "image/webp",
            mp3: "audio/mpeg",
            mp4: "video/mp4",
          };

          return {
            path: filePath,
            name: fileName,
            mime_type: mimeTypes[extension] || "application/octet-stream",
            size: 0,
          };
        });

        setAttachments((prev) => [...prev, ...newAttachments]);
      }
    } catch (error) {
      console.error("Failed to select files:", error);
    }
  };

  const handleRemoveAttachment = (index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index));
  };

  const handleSend = async () => {
    setSending(true);
    try {
      await onSend({
        to: parseAddresses(to),
        cc: parseAddresses(cc),
        subject,
        body,
        in_reply_to: initialData?.in_reply_to,
        attachments: attachments.length > 0 ? attachments : undefined,
      });
      setAttachments([]);
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
      attachments: attachments.length > 0 ? attachments : undefined,
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

          {/* Attachments section */}
          <div className="form-row attachments-row">
            <button type="button" onClick={handleAddAttachment} className="add-attachment-btn">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
              </svg>
              Add Attachment
            </button>
            {attachments.length > 0 && (
              <div className="compose-attachments-list">
                {attachments.map((attachment, index) => (
                  <div key={index} className="attachment-chip">
                    <span className="attachment-name" title={attachment.name}>
                      {attachment.name}
                    </span>
                    <button
                      type="button"
                      className="attachment-remove"
                      onClick={() => handleRemoveAttachment(index)}
                      title="Remove attachment"
                    >
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M18 6L6 18M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                ))}
              </div>
            )}
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
