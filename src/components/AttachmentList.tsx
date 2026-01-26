import { useState } from "react";
import * as api from "../lib/api";

interface AttachmentListProps {
  messageId: string; // Format: "folder:uid"
  hasAttachment: boolean;
  account?: string;
}

// Format file size to human readable string
function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

// Get file type icon based on mime type
function getFileIcon(mimeType: string): string {
  if (mimeType.startsWith("image/")) return "image";
  if (mimeType.startsWith("video/")) return "video";
  if (mimeType.startsWith("audio/")) return "audio";
  if (mimeType.includes("pdf")) return "pdf";
  if (mimeType.includes("zip") || mimeType.includes("compressed")) return "archive";
  if (mimeType.includes("word") || mimeType.includes("document")) return "document";
  if (mimeType.includes("sheet") || mimeType.includes("excel")) return "spreadsheet";
  return "file";
}

export function AttachmentList({ messageId, hasAttachment, account }: AttachmentListProps) {
  const [expanded, setExpanded] = useState(false);
  const [attachments, setAttachments] = useState<api.AttachmentInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [downloading, setDownloading] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!hasAttachment) {
    return null;
  }

  // Parse folder and uid from messageId
  const [folder, uid] = messageId.split(":");

  const handleToggle = async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }

    // Fetch attachments if not already loaded
    if (attachments.length === 0) {
      setLoading(true);
      setError(null);
      try {
        const result = await api.getMessageAttachments(folder, uid, account);
        setAttachments(result);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to load attachments");
      } finally {
        setLoading(false);
      }
    }
    setExpanded(true);
  };

  const handleDownload = async (attachment: api.AttachmentInfo) => {
    setDownloading(attachment.index);
    try {
      const filePath = await api.downloadAttachment(
        folder,
        uid,
        attachment.index,
        undefined,
        account
      );
      console.log("Downloaded to:", filePath);
    } catch (e) {
      console.error("Download failed:", e);
    } finally {
      setDownloading(null);
    }
  };

  return (
    <div className="attachment-container">
      <button
        className="attachment-toggle"
        onClick={handleToggle}
        title={expanded ? "Hide attachments" : "Show attachments"}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
        </svg>
        {!expanded && <span className="attachment-count">{attachments.length || ""}</span>}
      </button>

      {expanded && (
        <div className="attachment-list">
          {loading ? (
            <div className="attachment-loading">Loading...</div>
          ) : error ? (
            <div className="attachment-error">{error}</div>
          ) : attachments.length === 0 ? (
            <div className="attachment-empty">No attachments found</div>
          ) : (
            attachments.map((attachment) => (
              <button
                key={attachment.index}
                className="attachment-item"
                onClick={() => handleDownload(attachment)}
                disabled={downloading === attachment.index}
                title={`Download ${attachment.filename}`}
              >
                <span className={`attachment-icon attachment-icon-${getFileIcon(attachment.mime_type)}`}>
                  {getFileIcon(attachment.mime_type) === "image" && (
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                      <circle cx="8.5" cy="8.5" r="1.5" />
                      <polyline points="21,15 16,10 5,21" />
                    </svg>
                  )}
                  {getFileIcon(attachment.mime_type) === "pdf" && (
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                      <polyline points="14,2 14,8 20,8" />
                      <line x1="9" y1="15" x2="15" y2="15" />
                    </svg>
                  )}
                  {getFileIcon(attachment.mime_type) === "file" && (
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                      <polyline points="14,2 14,8 20,8" />
                    </svg>
                  )}
                  {["video", "audio", "archive", "document", "spreadsheet"].includes(getFileIcon(attachment.mime_type)) && (
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                      <polyline points="14,2 14,8 20,8" />
                    </svg>
                  )}
                </span>
                <span className="attachment-info">
                  <span className="attachment-name">{attachment.filename}</span>
                  <span className="attachment-size">{formatFileSize(attachment.size)}</span>
                </span>
                {downloading === attachment.index ? (
                  <span className="attachment-downloading">
                    <svg className="spinner" viewBox="0 0 24 24">
                      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" strokeWidth="3" strokeDasharray="31.4 31.4" />
                    </svg>
                  </span>
                ) : (
                  <span className="attachment-download-icon">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
                      <polyline points="7,10 12,15 17,10" />
                      <line x1="12" y1="15" x2="12" y2="3" />
                    </svg>
                  </span>
                )}
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}
