import { useState, useEffect, useRef, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { ComposeMessageData, ComposeAttachment } from "../types";
import { searchEntities, type EntitySuggestion } from "../lib/api";

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

  // Suggestion state
  const [suggestions, setSuggestions] = useState<EntitySuggestion[]>([]);
  const [activeField, setActiveField] = useState<"to" | "cc" | null>(null);
  const [selectedSuggestionIndex, setSelectedSuggestionIndex] = useState(0);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const toInputRef = useRef<HTMLInputElement>(null);
  const ccInputRef = useRef<HTMLInputElement>(null);

  // Get the current word being typed (after the last comma)
  const getCurrentWord = (value: string): string => {
    const parts = value.split(",");
    return parts[parts.length - 1].trim();
  };

  // Search for suggestions with debounce
  const searchSuggestions = useCallback(async (query: string) => {
    if (query.length < 1) {
      setSuggestions([]);
      return;
    }
    try {
      const results = await searchEntities(query, 5);
      setSuggestions(results);
      setSelectedSuggestionIndex(0);
    } catch (error) {
      console.error("Failed to search entities:", error);
      setSuggestions([]);
    }
  }, []);

  // Handle input change with debounced search
  const handleInputChange = (
    field: "to" | "cc",
    value: string,
    setter: React.Dispatch<React.SetStateAction<string>>
  ) => {
    setter(value);
    setActiveField(field);

    // Debounce the search
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
    }

    const currentWord = getCurrentWord(value);
    debounceTimerRef.current = setTimeout(() => {
      searchSuggestions(currentWord);
    }, 150);
  };

  // Handle selecting a suggestion
  const handleSelectSuggestion = (suggestion: EntitySuggestion) => {
    const setter = activeField === "to" ? setTo : setCc;
    const currentValue = activeField === "to" ? to : cc;

    // Replace the current word with the selected suggestion
    const parts = currentValue.split(",");
    parts[parts.length - 1] = " " + suggestion.email;
    const newValue = parts.join(",").trim() + ", ";

    setter(newValue);
    setSuggestions([]);
    setActiveField(null);

    // Focus back on the input
    const inputRef = activeField === "to" ? toInputRef : ccInputRef;
    inputRef.current?.focus();
  };

  // Handle keyboard navigation in suggestions
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (suggestions.length === 0) return;

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedSuggestionIndex((prev) =>
          prev < suggestions.length - 1 ? prev + 1 : prev
        );
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedSuggestionIndex((prev) => (prev > 0 ? prev - 1 : prev));
        break;
      case "Enter":
        if (suggestions[selectedSuggestionIndex]) {
          e.preventDefault();
          handleSelectSuggestion(suggestions[selectedSuggestionIndex]);
        }
        break;
      case "Escape":
        setSuggestions([]);
        setActiveField(null);
        break;
      case "Tab":
        if (suggestions[selectedSuggestionIndex]) {
          e.preventDefault();
          handleSelectSuggestion(suggestions[selectedSuggestionIndex]);
        }
        break;
    }
  };

  // Clear suggestions when clicking outside
  const handleInputBlur = () => {
    // Delay to allow click on suggestion to register
    setTimeout(() => {
      setSuggestions([]);
      setActiveField(null);
    }, 200);
  };

  // Cleanup debounce timer on unmount
  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, []);

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
          <div className="form-row form-row-with-suggestions">
            <label htmlFor="to">To:</label>
            <div className="input-with-suggestions">
              <input
                ref={toInputRef}
                id="to"
                type="text"
                value={to}
                onChange={(e) => handleInputChange("to", e.target.value, setTo)}
                onKeyDown={handleKeyDown}
                onBlur={handleInputBlur}
                onFocus={() => setActiveField("to")}
                placeholder="recipient@example.com"
                autoComplete="off"
              />
              {activeField === "to" && suggestions.length > 0 && (
                <div className="suggestions-dropdown">
                  {suggestions.map((suggestion, index) => (
                    <div
                      key={suggestion.id}
                      className={`suggestion-item ${index === selectedSuggestionIndex ? "selected" : ""} ${suggestion.is_connection ? "is-connection" : ""}`}
                      onMouseDown={() => handleSelectSuggestion(suggestion)}
                      onMouseEnter={() => setSelectedSuggestionIndex(index)}
                    >
                      <span className="suggestion-email">{suggestion.email}</span>
                      {suggestion.name && (
                        <span className="suggestion-name">{suggestion.name}</span>
                      )}
                      {suggestion.is_connection && (
                        <span className="suggestion-badge" title="You've emailed this person">
                          <svg viewBox="0 0 24 24" fill="currentColor" width="12" height="12">
                            <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                          </svg>
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="form-row form-row-with-suggestions">
            <label htmlFor="cc">Cc:</label>
            <div className="input-with-suggestions">
              <input
                ref={ccInputRef}
                id="cc"
                type="text"
                value={cc}
                onChange={(e) => handleInputChange("cc", e.target.value, setCc)}
                onKeyDown={handleKeyDown}
                onBlur={handleInputBlur}
                onFocus={() => setActiveField("cc")}
                placeholder="cc@example.com"
                autoComplete="off"
              />
              {activeField === "cc" && suggestions.length > 0 && (
                <div className="suggestions-dropdown">
                  {suggestions.map((suggestion, index) => (
                    <div
                      key={suggestion.id}
                      className={`suggestion-item ${index === selectedSuggestionIndex ? "selected" : ""} ${suggestion.is_connection ? "is-connection" : ""}`}
                      onMouseDown={() => handleSelectSuggestion(suggestion)}
                      onMouseEnter={() => setSelectedSuggestionIndex(index)}
                    >
                      <span className="suggestion-email">{suggestion.email}</span>
                      {suggestion.name && (
                        <span className="suggestion-name">{suggestion.name}</span>
                      )}
                      {suggestion.is_connection && (
                        <span className="suggestion-badge" title="You've emailed this person">
                          <svg viewBox="0 0 24 24" fill="currentColor" width="12" height="12">
                            <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                          </svg>
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
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

        <div className="compose-footer">
          <button
            type="button"
            onClick={handleAddAttachment}
            className="attachment-icon-btn"
            title="Add attachment"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="20" height="20">
              <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
            </svg>
          </button>
          <div className="footer-spacer"></div>
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
