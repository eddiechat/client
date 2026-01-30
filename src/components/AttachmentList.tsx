import { useState } from "react";
import * as api from "../lib/api";

interface AttachmentListProps {
  messageId: string;
  hasAttachment: boolean;
  account?: string;
}

function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

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

  if (!hasAttachment) return null;

  const [folder, uid] = messageId.split(":");

  const handleToggle = async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
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
      const filePath = await api.downloadAttachment(folder, uid, attachment.index, undefined, account);
      console.log("Downloaded to:", filePath);
    } catch (e) {
      console.error("Download failed:", e);
    } finally {
      setDownloading(null);
    }
  };

  return (
    <div className="mt-2 pt-2 border-t border-white/10">
      <button
        className="flex items-center gap-1.5 text-xs text-white/70 hover:text-white transition-colors"
        onClick={handleToggle}
        title={expanded ? "Hide attachments" : "Show attachments"}
      >
        <svg className="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
        </svg>
        <span>{expanded ? "Hide" : "Attachments"}</span>
        {!expanded && attachments.length > 0 && (
          <span className="bg-white/20 px-1.5 py-0.5 rounded-full text-[10px]">{attachments.length}</span>
        )}
      </button>

      {expanded && (
        <div className="mt-2 flex flex-col gap-1">
          {loading ? (
            <div className="text-xs text-white/50 py-1">Loading...</div>
          ) : error ? (
            <div className="text-xs text-accent-red py-1">{error}</div>
          ) : attachments.length === 0 ? (
            <div className="text-xs text-white/50 py-1">No attachments found</div>
          ) : (
            attachments.map((attachment) => (
              <button
                key={attachment.index}
                className="flex items-center gap-2 px-2 py-1.5 bg-white/5 rounded-lg hover:bg-white/10 transition-colors text-left disabled:opacity-50"
                onClick={() => handleDownload(attachment)}
                disabled={downloading === attachment.index}
                title={`Download ${attachment.filename}`}
              >
                <div className="w-5 h-5 shrink-0 flex items-center justify-center text-white/60">
                  {getFileIcon(attachment.mime_type) === "image" ? (
                    <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                      <circle cx="8.5" cy="8.5" r="1.5" />
                      <polyline points="21,15 16,10 5,21" />
                    </svg>
                  ) : (
                    <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                      <polyline points="14,2 14,8 20,8" />
                    </svg>
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-xs text-white truncate">{attachment.filename}</div>
                  <div className="text-[10px] text-white/50">{formatFileSize(attachment.size)}</div>
                </div>
                {downloading === attachment.index ? (
                  <div className="spinner w-4 h-4" />
                ) : (
                  <svg className="w-4 h-4 text-white/40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
                    <polyline points="7,10 12,15 17,10" />
                    <line x1="12" y1="15" x2="12" y2="3" />
                  </svg>
                )}
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}
