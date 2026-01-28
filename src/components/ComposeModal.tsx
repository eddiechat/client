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
    input.split(",").map((s) => s.trim()).filter(Boolean);

  const handleAddAttachment = async () => {
    try {
      const selected = await open({ multiple: true, title: "Select files to attach" });
      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
        const mimeTypes: Record<string, string> = {
          pdf: "application/pdf", doc: "application/msword", docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
          xls: "application/vnd.ms-excel", xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
          txt: "text/plain", csv: "text/csv", html: "text/html", json: "application/json", zip: "application/zip",
          png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp",
          mp3: "audio/mpeg", mp4: "video/mp4",
        };
        const newAttachments: ComposeAttachment[] = files.map((filePath) => {
          const fileName = filePath.split(/[/\\]/).pop() || "attachment";
          const extension = fileName.split(".").pop()?.toLowerCase() || "";
          return { path: filePath, name: fileName, mime_type: mimeTypes[extension] || "application/octet-stream", size: 0 };
        });
        setAttachments((prev) => [...prev, ...newAttachments]);
      }
    } catch (error) {
      console.error("Failed to select files:", error);
    }
  };

  const handleRemoveAttachment = (index: number) => setAttachments((prev) => prev.filter((_, i) => i !== index));

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
      case "reply": return "Reply";
      case "forward": return "Forward";
      default: return "New Message";
    }
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4 safe-y">
      <div className="w-full max-w-lg bg-bg-secondary rounded-2xl flex flex-col max-h-[90vh] overflow-hidden shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-divider">
          <h2 className="text-lg font-semibold text-text-primary">{getTitle()}</h2>
          <button
            className="w-8 h-8 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors text-xl text-text-muted"
            onClick={onClose}
          >
            ×
          </button>
        </div>

        {/* Form */}
        <div className="flex-1 overflow-y-auto p-5 flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label htmlFor="to" className="text-sm font-medium text-text-muted">To:</label>
            <input
              id="to"
              type="text"
              className="w-full px-3.5 py-2.5 bg-bg-tertiary border border-divider rounded-lg text-text-primary text-[15px] outline-none focus:border-accent-blue transition-colors placeholder:text-text-muted"
              value={to}
              onChange={(e) => setTo(e.target.value)}
              placeholder="recipient@example.com"
            />
          </div>

          <div className="flex flex-col gap-1.5">
            <label htmlFor="cc" className="text-sm font-medium text-text-muted">Cc:</label>
            <input
              id="cc"
              type="text"
              className="w-full px-3.5 py-2.5 bg-bg-tertiary border border-divider rounded-lg text-text-primary text-[15px] outline-none focus:border-accent-blue transition-colors placeholder:text-text-muted"
              value={cc}
              onChange={(e) => setCc(e.target.value)}
              placeholder="cc@example.com"
            />
          </div>

          <div className="flex flex-col gap-1.5">
            <label htmlFor="subject" className="text-sm font-medium text-text-muted">Subject:</label>
            <input
              id="subject"
              type="text"
              className="w-full px-3.5 py-2.5 bg-bg-tertiary border border-divider rounded-lg text-text-primary text-[15px] outline-none focus:border-accent-blue transition-colors"
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
            />
          </div>

          <div className="flex flex-col gap-1.5 flex-1">
            <textarea
              className="w-full flex-1 min-h-[200px] px-3.5 py-2.5 bg-bg-tertiary border border-divider rounded-lg text-text-primary text-[15px] outline-none focus:border-accent-blue transition-colors resize-none placeholder:text-text-muted"
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder="Write your message..."
            />
          </div>

          {attachments.length > 0 && (
            <div className="flex flex-wrap gap-2">
              {attachments.map((attachment, index) => (
                <div key={index} className="inline-flex items-center gap-1.5 bg-bg-tertiary rounded-lg px-2.5 py-1.5 max-w-[200px]">
                  <span className="text-sm text-text-primary truncate" title={attachment.name}>{attachment.name}</span>
                  <button
                    type="button"
                    className="w-4 h-4 rounded-full flex items-center justify-center opacity-60 hover:opacity-100"
                    onClick={() => handleRemoveAttachment(index)}
                    title="Remove attachment"
                  >
                    <svg className="w-3 h-3 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M18 6L6 18M6 6l12 12" />
                    </svg>
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-4 border-t border-divider">
          <button
            type="button"
            className="w-9 h-9 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors"
            onClick={handleAddAttachment}
            title="Add attachment"
          >
            <svg className="w-5 h-5 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
            </svg>
          </button>
          <div className="flex gap-2">
            <button
              type="button"
              className="px-4 py-2 rounded-lg text-sm font-medium bg-bg-tertiary text-text-primary hover:bg-bg-hover transition-colors"
              onClick={handleSaveDraft}
              disabled={sending}
            >
              Save Draft
            </button>
            <button
              type="button"
              className="px-5 py-2 rounded-lg text-sm font-medium bg-bubble-sent text-white hover:brightness-110 transition-all disabled:opacity-50"
              onClick={handleSend}
              disabled={sending}
            >
              {sending ? "Sending..." : "Send"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
