import { useState, useRef, useEffect, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Conversation, ChatMessage, ComposeAttachment, ReplyTarget } from "../../../tauri";
import { searchEntities, type EntitySuggestion } from "../../../tauri/commands";
import {
  extractEmail,
  parseEmailContent,
  hasExpandableContent,
  formatMessageTime,
  formatDateSeparator,
  isDifferentDay,
} from "../../../shared";
import { Avatar } from "../../../shared/components";
import { searchEmojis } from "../../../lib/emojiData";
import { GravatarModal } from "./GravatarModal";
import { AttachmentList } from "./AttachmentList";
import { ChatMessageAsEmail } from "./ChatMessageAsEmail";
import { EmojiPicker, EmojiSuggestions } from "./EmojiPicker";
import {
  getHeaderAvatarTooltip,
  getSenderName,
  getAvatarTooltip,
  isOutgoing,
} from "../utils";

interface ConversationViewProps {
  conversation: Conversation | null;
  messages: ChatMessage[];
  loading?: boolean;
  error?: string | null;
  currentAccountEmail?: string;
  onSendMessage: (text: string, attachments?: ComposeAttachment[], replyTarget?: ReplyTarget) => void;
  onBack?: () => void;
  isComposing?: boolean;
  composeParticipants?: string[];
  onComposeParticipantsConfirm?: (participants: string[]) => void;
  onSendNewMessage?: (
    text: string,
    participants: string[],
    attachments?: ComposeAttachment[]
  ) => void;
}

export function ConversationView({
  conversation,
  messages,
  loading,
  error,
  currentAccountEmail,
  onSendMessage,
  onBack,
  isComposing,
  composeParticipants = [],
  onComposeParticipantsConfirm,
  onSendNewMessage,
}: ConversationViewProps) {
  const [inputValue, setInputValue] = useState("");
  const [toInputValue, setToInputValue] = useState("");
  const [participantsConfirmed, setParticipantsConfirmed] = useState(false);
  const [gravatarModalData, setGravatarModalData] = useState<{
    email: string;
    name: string;
  } | null>(null);
  const [fullViewMessage, setFullViewMessage] = useState<ChatMessage | null>(null);
  const [attachments, setAttachments] = useState<ComposeAttachment[]>([]);
  const [replyTarget, setReplyTarget] = useState<ReplyTarget | null>(null);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const [emojiSuggestion, setEmojiSuggestion] = useState<{
    query: string;
    startPos: number;
  } | null>(null);
  const [suggestionIndex, setSuggestionIndex] = useState(0);
  // Entity autocomplete state
  const [entitySuggestions, setEntitySuggestions] = useState<EntitySuggestion[]>([]);
  const [entitySuggestionIndex, setEntitySuggestionIndex] = useState(0);
  const entitySearchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const toInputRef = useRef<HTMLInputElement>(null);
  const prevConversationRef = useRef<Conversation | null>(null);
  const wasComposingRef = useRef(false);

  // Reset modals and reply target when conversation changes
  useEffect(() => {
    setGravatarModalData(null);
    setFullViewMessage(null);
    setReplyTarget(null);
  }, [conversation?.id]);

  // Scroll to bottom when messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Focus management for compose mode
  useEffect(() => {
    if (isComposing && !participantsConfirmed && composeParticipants.length === 0) {
      setTimeout(() => toInputRef.current?.focus(), 50);
      setToInputValue("");
      setInputValue("");
      setParticipantsConfirmed(false);
    } else if (isComposing && (participantsConfirmed || composeParticipants.length > 0)) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isComposing, participantsConfirmed, composeParticipants.length]);

  // Focus input when returning from compose mode
  useEffect(() => {
    if (wasComposingRef.current && !isComposing && conversation) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
    wasComposingRef.current = isComposing || false;
    prevConversationRef.current = conversation;
  }, [isComposing, conversation]);

  // Clear compose state when not composing
  useEffect(() => {
    if (!isComposing) {
      setToInputValue("");
      setParticipantsConfirmed(false);
      setAttachments([]);
    }
  }, [isComposing]);

  // Clear attachments when conversation changes
  useEffect(() => {
    setAttachments([]);
  }, [conversation?.id]);

  // Cleanup entity search timer on unmount
  useEffect(() => {
    return () => {
      if (entitySearchTimerRef.current) {
        clearTimeout(entitySearchTimerRef.current);
      }
    };
  }, []);

  // Get the current word being typed in To field (after the last comma)
  const getCurrentToWord = (value: string): string => {
    const parts = value.split(",");
    return parts[parts.length - 1].trim();
  };

  // Search entities with debounce
  const searchEntitySuggestions = useCallback(async (query: string) => {
    if (query.length < 1) {
      setEntitySuggestions([]);
      return;
    }
    try {
      const results = await searchEntities(query, 5);
      setEntitySuggestions(results);
      setEntitySuggestionIndex(0);
    } catch (error) {
      console.error("Failed to search entities:", error);
      setEntitySuggestions([]);
    }
  }, []);

  // Handle To input change with debounced entity search
  const handleToInputChange = (value: string) => {
    setToInputValue(value);

    // Debounce entity search
    if (entitySearchTimerRef.current) {
      clearTimeout(entitySearchTimerRef.current);
    }

    const currentWord = getCurrentToWord(value);
    entitySearchTimerRef.current = setTimeout(() => {
      searchEntitySuggestions(currentWord);
    }, 150);
  };

  // Handle selecting an entity suggestion
  const handleSelectEntitySuggestion = (suggestion: EntitySuggestion) => {
    const parts = toInputValue.split(",");
    parts[parts.length - 1] = " " + suggestion.email;
    const newValue = parts.join(",").trim() + ", ";
    setToInputValue(newValue);
    setEntitySuggestions([]);
    toInputRef.current?.focus();
  };

  const handleAddAttachment = async () => {
    try {
      const selected = await open({
        multiple: true,
        title: "Select files to attach",
      });
      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
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
        const newAttachments: ComposeAttachment[] = files.map((filePath) => {
          const fileName = filePath.split(/[/\\]/).pop() || "attachment";
          const extension = fileName.split(".").pop()?.toLowerCase() || "";
          return {
            path: filePath,
            name: fileName,
            mime_type: mimeTypes[extension] || "application/octet-stream",
            size: 0,
          };
        });
        setAttachments((prev) => [...prev, ...newAttachments]);
      }
    } catch (err) {
      console.error("Failed to select files:", err);
    }
  };

  const handleRemoveAttachment = (index: number) =>
    setAttachments((prev) => prev.filter((_, i) => i !== index));

  const handleEmojiSelect = (emoji: string) => {
    if (inputRef.current) {
      const start = inputRef.current.selectionStart || 0;
      const end = inputRef.current.selectionEnd || 0;
      const newValue =
        inputValue.slice(0, start) + emoji + inputValue.slice(end);
      setInputValue(newValue);
      setTimeout(() => {
        if (inputRef.current) {
          const newPos = start + emoji.length;
          inputRef.current.setSelectionRange(newPos, newPos);
          inputRef.current.focus();
        }
      }, 0);
    } else {
      setInputValue(inputValue + emoji);
    }
    setShowEmojiPicker(false);
  };

  const handleEmojiSuggestionSelect = (emoji: string, _name: string) => {
    if (emojiSuggestion && inputRef.current) {
      const beforeColon = inputValue.slice(0, emojiSuggestion.startPos);
      const afterQuery = inputValue.slice(
        inputRef.current.selectionStart ||
          emojiSuggestion.startPos + emojiSuggestion.query.length + 1
      );
      const newValue = beforeColon + emoji + afterQuery;
      setInputValue(newValue);
      setEmojiSuggestion(null);
      setSuggestionIndex(0);
      setTimeout(() => {
        if (inputRef.current) {
          const newPos = beforeColon.length + emoji.length;
          inputRef.current.setSelectionRange(newPos, newPos);
          inputRef.current.focus();
        }
      }, 0);
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    setInputValue(newValue);
    const cursorPos = e.target.selectionStart || 0;
    const textBeforeCursor = newValue.slice(0, cursorPos);
    const colonMatch = textBeforeCursor.match(/:([a-zA-Z0-9_]*)$/);
    if (colonMatch && colonMatch[1].length >= 1) {
      setEmojiSuggestion({
        query: colonMatch[1],
        startPos: cursorPos - colonMatch[0].length,
      });
      setSuggestionIndex(0);
    } else {
      setEmojiSuggestion(null);
    }
  };

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (emojiSuggestion) {
      const suggestions = searchEmojis(emojiSuggestion.query, 8);
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSuggestionIndex((prev) => (prev + 1) % suggestions.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSuggestionIndex(
          (prev) => (prev - 1 + suggestions.length) % suggestions.length
        );
      } else if (e.key === "Enter" && suggestions.length > 0) {
        e.preventDefault();
        handleEmojiSuggestionSelect(
          suggestions[suggestionIndex].emoji,
          suggestions[suggestionIndex].name
        );
      } else if (e.key === "Escape") {
        e.preventDefault();
        setEmojiSuggestion(null);
      } else if (e.key === "Tab" && suggestions.length > 0) {
        e.preventDefault();
        handleEmojiSuggestionSelect(
          suggestions[suggestionIndex].emoji,
          suggestions[suggestionIndex].name
        );
      }
    }
  };

  const handleToKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    // Handle entity suggestion navigation
    if (entitySuggestions.length > 0) {
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setEntitySuggestionIndex((prev) =>
            prev < entitySuggestions.length - 1 ? prev + 1 : prev
          );
          return;
        case "ArrowUp":
          e.preventDefault();
          setEntitySuggestionIndex((prev) => (prev > 0 ? prev - 1 : prev));
          return;
        case "Tab":
          e.preventDefault();
          handleSelectEntitySuggestion(entitySuggestions[entitySuggestionIndex]);
          return;
        case "Escape":
          e.preventDefault();
          setEntitySuggestions([]);
          return;
      }
    }

    if (e.key === "Enter") {
      e.preventDefault();
      // If there are entity suggestions, select the current one
      if (entitySuggestions.length > 0) {
        handleSelectEntitySuggestion(entitySuggestions[entitySuggestionIndex]);
        return;
      }
      // Otherwise, confirm participants
      const participants = toInputValue
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      if (participants.length > 0 && onComposeParticipantsConfirm) {
        setParticipantsConfirmed(true);
        setEntitySuggestions([]);
        onComposeParticipantsConfirm(participants);
      }
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (inputValue.trim() || attachments.length > 0) {
      if (
        isComposing &&
        !conversation &&
        composeParticipants.length > 0 &&
        onSendNewMessage
      ) {
        onSendNewMessage(
          inputValue.trim(),
          composeParticipants,
          attachments.length > 0 ? attachments : undefined
        );
        setInputValue("");
        setAttachments([]);
        setParticipantsConfirmed(false);
      } else if (conversation) {
        onSendMessage(
          inputValue.trim(),
          attachments.length > 0 ? attachments : undefined,
          replyTarget || undefined
        );
        setInputValue("");
        setAttachments([]);
        setReplyTarget(null);
      }
      inputRef.current?.focus();
    }
  };

  // Compose mode
  if (isComposing && !conversation) {
    const hasParticipants =
      composeParticipants.length > 0 || participantsConfirmed;
    return (
      <div className="flex flex-col h-full">
        <ComposeHeader
          onBack={onBack}
          hasParticipants={hasParticipants}
          composeParticipants={composeParticipants}
          toInputValue={toInputValue}
          toInputRef={toInputRef}
          onToInputChange={handleToInputChange}
          onToKeyDown={handleToKeyDown}
          entitySuggestions={entitySuggestions}
          entitySuggestionIndex={entitySuggestionIndex}
          onSelectEntitySuggestion={handleSelectEntitySuggestion}
          onEntitySuggestionHover={setEntitySuggestionIndex}
        />

        {/* Empty area */}
        <div className="flex-1 overflow-y-auto p-4 flex flex-col items-center justify-center">
          <p className="text-text-muted text-sm text-center">
            {!hasParticipants
              ? "Enter recipients and press Enter to start a new conversation"
              : "Start your conversation"}
          </p>
        </div>

        {/* Input */}
        {hasParticipants && (
          <MessageInput
            inputValue={inputValue}
            inputRef={inputRef}
            attachments={attachments}
            showEmojiPicker={showEmojiPicker}
            emojiSuggestion={emojiSuggestion}
            suggestionIndex={suggestionIndex}
            placeholder="Type your message (first line becomes subject)"
            onInputChange={handleInputChange}
            onInputKeyDown={handleInputKeyDown}
            onSubmit={handleSubmit}
            onAddAttachment={handleAddAttachment}
            onRemoveAttachment={handleRemoveAttachment}
            onEmojiSelect={handleEmojiSelect}
            onEmojiSuggestionSelect={handleEmojiSuggestionSelect}
            onToggleEmojiPicker={() => setShowEmojiPicker(!showEmojiPicker)}
            onCloseEmojiSuggestion={() => setEmojiSuggestion(null)}
          />
        )}
      </div>
    );
  }

  // Empty state
  if (!conversation) {
    return (
      <div className="flex flex-col h-full items-center justify-center">
        <div className="text-center p-10 max-w-xs">
          <div className="w-20 h-20 mx-auto mb-6 bg-bg-tertiary rounded-full flex items-center justify-center">
            <svg
              className="w-10 h-10 text-text-muted"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
            >
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
          </div>
          <h3 className="text-xl font-semibold text-text-primary mb-2">
            Select a conversation
          </h3>
          <p className="text-sm text-text-muted leading-relaxed">
            Choose a conversation from the list to start messaging
          </p>
        </div>
      </div>
    );
  }

  const headerTooltip = getHeaderAvatarTooltip(conversation);
  const participantData = conversation.participants.map((p, idx) => ({
    email: extractEmail(p),
    name: conversation.participant_names[idx] || extractEmail(p),
  }));
  // Show all participants in header (including user)
  const headerAvatarsToShow = participantData;

  // Header should show all participant names (including user), not filtered
  const conversationName = conversation.participant_names
    .map((name) => name.split(" ")[0]) // Get first names
    .join(", ");

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <ConversationHeader
        onBack={onBack}
        conversationName={conversationName}
        headerTooltip={headerTooltip}
        headerAvatarsToShow={headerAvatarsToShow}
        participantCount={conversation.participant_names.length}
      />

      {/* Content area */}
      {fullViewMessage ? (
        <ChatMessageAsEmail
          message={fullViewMessage}
          onClose={() => setFullViewMessage(null)}
        />
      ) : gravatarModalData ? (
        <GravatarModal
          email={gravatarModalData.email}
          name={gravatarModalData.name}
          isOpen={!!gravatarModalData}
          onClose={() => setGravatarModalData(null)}
        />
      ) : (
        <>
          {/* Messages */}
          <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-1">
            {loading ? (
              <div className="flex-1 flex flex-col items-center justify-center gap-3 text-text-muted text-sm">
                <div className="spinner" />
                <span>Loading messages...</span>
              </div>
            ) : error ? (
              <div className="flex-1 flex items-center justify-center text-text-muted text-sm">
                Error loading messages: {error}
              </div>
            ) : messages.length === 0 ? (
              <div className="flex-1 flex items-center justify-center text-text-muted text-sm">
                No messages yet. Start the conversation!
              </div>
            ) : (
              <>
                {messages.map((message, index) => {
                  const isOut = isOutgoing(
                    message.envelope.from,
                    currentAccountEmail
                  );
                  const showDateSeparator =
                    index === 0 ||
                    isDifferentDay(
                      messages[index - 1].envelope.date,
                      message.envelope.date
                    );
                  const showSender =
                    !isOut && conversation.participant_names.length > 2;

                  return (
                    <MessageBubble
                      key={message.id}
                      message={message}
                      isOutgoing={isOut}
                      showDateSeparator={showDateSeparator}
                      showSender={showSender}
                      onAvatarClick={(email, name) =>
                        setGravatarModalData({ email, name })
                      }
                      onExpandClick={() => setFullViewMessage(message)}
                    />
                  );
                })}
                <div ref={messagesEndRef} />
              </>
            )}
          </div>

          {/* Input */}
          <MessageInput
            inputValue={inputValue}
            inputRef={inputRef}
            attachments={attachments}
            showEmojiPicker={showEmojiPicker}
            emojiSuggestion={emojiSuggestion}
            suggestionIndex={suggestionIndex}
            placeholder="Message"
            onInputChange={handleInputChange}
            onInputKeyDown={handleInputKeyDown}
            onSubmit={handleSubmit}
            onAddAttachment={handleAddAttachment}
            onRemoveAttachment={handleRemoveAttachment}
            onEmojiSelect={handleEmojiSelect}
            onEmojiSuggestionSelect={handleEmojiSuggestionSelect}
            onToggleEmojiPicker={() => setShowEmojiPicker(!showEmojiPicker)}
            onCloseEmojiSuggestion={() => setEmojiSuggestion(null)}
          />
        </>
      )}
    </div>
  );
}

// Sub-components for better organization

interface ComposeHeaderProps {
  onBack?: () => void;
  hasParticipants: boolean;
  composeParticipants: string[];
  toInputValue: string;
  toInputRef: React.RefObject<HTMLInputElement | null>;
  onToInputChange: (value: string) => void;
  onToKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  entitySuggestions: EntitySuggestion[];
  entitySuggestionIndex: number;
  onSelectEntitySuggestion: (suggestion: EntitySuggestion) => void;
  onEntitySuggestionHover: (index: number) => void;
}

function ComposeHeader({
  onBack,
  hasParticipants,
  composeParticipants,
  toInputValue,
  toInputRef,
  onToInputChange,
  onToKeyDown,
  entitySuggestions,
  entitySuggestionIndex,
  onSelectEntitySuggestion,
  onEntitySuggestionHover,
}: ComposeHeaderProps) {
  return (
    <div
      className="flex items-center gap-3 px-4 h-16 safe-x"
      style={{
        paddingTop: "calc(0.75rem + env(safe-area-inset-top))",
        paddingBottom: "0.75rem",
      }}
    >
      {onBack && (
        <button
          className="flex md:hidden w-9 h-9 rounded-full items-center justify-center hover:bg-bg-hover transition-colors"
          onClick={onBack}
        >
          <svg
            className="w-5 h-5 text-text-primary"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
      )}
      {!hasParticipants ? (
        <div className="flex-1 flex items-center gap-2 relative">
          <span className="text-[15px] font-medium text-text-muted whitespace-nowrap">
            To:
          </span>
          <div className="flex-1 relative">
            <input
              ref={toInputRef}
              type="text"
              className="w-full bg-transparent border-none text-text-primary text-[15px] outline-none py-2 placeholder:text-text-muted"
              placeholder="Enter email addresses (comma-separated)"
              value={toInputValue}
              onChange={(e) => onToInputChange(e.target.value)}
              onKeyDown={onToKeyDown}
              autoComplete="off"
            />
            {entitySuggestions.length > 0 && (
              <div className="suggestions-dropdown">
                {entitySuggestions.map((suggestion, index) => (
                  <div
                    key={suggestion.id}
                    className={`suggestion-item ${index === entitySuggestionIndex ? "selected" : ""} ${suggestion.is_connection ? "is-connection" : ""}`}
                    onMouseDown={() => onSelectEntitySuggestion(suggestion)}
                    onMouseEnter={() => onEntitySuggestionHover(index)}
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
      ) : (
        <>
          <div className="w-10 h-10 relative flex items-center">
            {composeParticipants.slice(0, 2).map((participant, index) => (
              <div
                key={index}
                className={composeParticipants.length > 1 ? "absolute" : ""}
                style={
                  composeParticipants.length > 1
                    ? {
                        left: `${index * 19}px`,
                        zIndex: 20 - index,
                      }
                    : undefined
                }
              >
                <Avatar
                  email={extractEmail(participant)}
                  name={participant}
                  size={composeParticipants.length > 1 ? 28 : 40}
                  className={
                    composeParticipants.length > 1
                      ? "border-2 border-bg-secondary"
                      : ""
                  }
                />
              </div>
            ))}
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-[17px] font-semibold text-text-primary truncate">
              {composeParticipants.join(", ")}
            </h2>
            <span className="text-[13px] text-text-muted">New conversation</span>
          </div>
        </>
      )}
    </div>
  );
}

interface ConversationHeaderProps {
  onBack?: () => void;
  conversationName: string;
  headerTooltip: string;
  headerAvatarsToShow: { email: string; name: string }[];
  participantCount: number;
}

function ConversationHeader({
  onBack,
  conversationName,
  headerTooltip,
  headerAvatarsToShow,
  participantCount,
}: ConversationHeaderProps) {
  // Calculate width needed for avatar container based on number of avatars
  // With 33% overlap: first avatar = 28px, each additional = 19px (28 * 0.67)
  const containerWidth =
    headerAvatarsToShow.length > 1
      ? 28 + (headerAvatarsToShow.length - 1) * 19
      : 40;

  return (
    <div
      className="flex items-center gap-3 px-4"
      style={{
        minHeight: "4rem",
        paddingTop: "calc(0.75rem + env(safe-area-inset-top))",
        paddingBottom: "0.75rem",
      }}
    >
      {onBack && (
        <button
          className="flex md:hidden w-9 h-9 rounded-full items-center justify-center hover:bg-bg-hover transition-colors"
          onClick={onBack}
        >
          <svg
            className="w-5 h-5 text-text-primary"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
      )}
      <div
        className="h-10 relative flex items-center"
        style={{ width: `${containerWidth}px` }}
        title={headerTooltip}
      >
        {headerAvatarsToShow.map((pd, index) => (
          <div
            key={index}
            className={headerAvatarsToShow.length > 1 ? "absolute" : ""}
            style={
              headerAvatarsToShow.length > 1
                ? {
                    left: `${index * 19}px`,
                    zIndex: 20 - index,
                  }
                : undefined
            }
          >
            <Avatar
              email={pd.email}
              name={pd.name}
              size={headerAvatarsToShow.length > 1 ? 28 : 40}
              className={
                headerAvatarsToShow.length > 1
                  ? "border-2 border-bg-secondary"
                  : ""
              }
            />
          </div>
        ))}
      </div>
      <div className="flex-1 min-w-0">
        <h2 className="text-[17px] font-semibold text-text-primary truncate">
          {conversationName}
        </h2>
        <span className="text-[13px] text-text-muted">
          {participantCount > 1 ? `${participantCount} participants` : ""}
        </span>
      </div>
      <div className="flex gap-1">
        <button className="w-9 h-9 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors">
          <svg
            className="w-5 h-5 text-text-secondary"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.35-4.35" />
          </svg>
        </button>
        <button className="w-9 h-9 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors">
          <svg
            className="w-5 h-5 text-text-secondary"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <circle cx="12" cy="5" r="1.5" />
            <circle cx="12" cy="12" r="1.5" />
            <circle cx="12" cy="19" r="1.5" />
          </svg>
        </button>
      </div>
    </div>
  );
}

interface MessageBubbleProps {
  message: ChatMessage;
  isOutgoing: boolean;
  showDateSeparator: boolean;
  showSender: boolean;
  onAvatarClick: (email: string, name: string) => void;
  onExpandClick: () => void;
}

function MessageBubble({
  message,
  isOutgoing,
  showDateSeparator,
  showSender,
  onAvatarClick,
  onExpandClick,
}: MessageBubbleProps) {
  const isExpandable = hasExpandableContent(
    message.text_body,
    message.html_body
  );

  return (
    <div>
      {showDateSeparator && (
        <div className="flex items-center justify-center py-4">
          <span className="px-3 py-1.5 bg-bg-tertiary rounded-2xl text-xs font-medium text-text-muted">
            {formatDateSeparator(message.envelope.date)}
          </span>
        </div>
      )}
      <div
        className={`flex gap-2 max-w-[85%] md:max-w-[75%] mb-1 ${
          isOutgoing ? "self-end flex-row-reverse ml-auto" : "self-start"
        }`}
      >
        {!isOutgoing && (
          <Avatar
            email={extractEmail(message.envelope.from)}
            name={getSenderName(message.envelope.from)}
            size={32}
            className="self-end cursor-pointer"
            title={getAvatarTooltip(message.envelope.from, message.id)}
            onClick={() => {
              const email = extractEmail(message.envelope.from);
              const name = getSenderName(message.envelope.from);
              if (email) onAvatarClick(email, name);
            }}
          />
        )}
        <div className="flex flex-col gap-0.5 min-w-0 overflow-hidden">
          {showSender && (
            <span className="text-xs font-medium text-accent-blue pl-3">
              {getSenderName(message.envelope.from)}
            </span>
          )}
          <div
            className={`px-3.5 py-2.5 rounded-2xl max-w-full break-words relative ${
              isOutgoing
                ? "bg-bubble-sent text-white rounded-br-sm"
                : "bg-bubble-received text-white rounded-bl-sm"
            } ${
              isExpandable
                ? "border border-white/15 cursor-pointer hover:border-white/30"
                : ""
            }`}
            onClick={isExpandable ? onExpandClick : undefined}
            title={isExpandable ? "Click to view full message" : undefined}
          >
            <div className="text-[15px] leading-snug whitespace-pre-wrap break-words">
              {parseEmailContent(message.text_body) ||
                message.envelope.subject ||
                "(No content)"}
            </div>
            <AttachmentList
              messageId={message.id}
              hasAttachment={message.envelope.has_attachment}
            />
            <span className="flex items-center gap-1 text-[11px] opacity-70 mt-1 justify-end">
              {formatMessageTime(message.envelope.date)}
              {isOutgoing && (
                <svg className="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M18 7l-8.5 8.5-4-4L4 13l5.5 5.5L19.5 8.5z" />
                </svg>
              )}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

interface MessageInputProps {
  inputValue: string;
  inputRef: React.RefObject<HTMLInputElement | null>;
  attachments: ComposeAttachment[];
  showEmojiPicker: boolean;
  emojiSuggestion: { query: string; startPos: number } | null;
  suggestionIndex: number;
  placeholder: string;
  onInputChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onInputKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  onSubmit: (e: React.FormEvent) => void;
  onAddAttachment: () => void;
  onRemoveAttachment: (index: number) => void;
  onEmojiSelect: (emoji: string) => void;
  onEmojiSuggestionSelect: (emoji: string, name: string) => void;
  onToggleEmojiPicker: () => void;
  onCloseEmojiSuggestion: () => void;
}

function MessageInput({
  inputValue,
  inputRef,
  attachments,
  showEmojiPicker,
  emojiSuggestion,
  suggestionIndex,
  placeholder,
  onInputChange,
  onInputKeyDown,
  onSubmit,
  onAddAttachment,
  onRemoveAttachment,
  onEmojiSelect,
  onEmojiSuggestionSelect,
  onToggleEmojiPicker,
  onCloseEmojiSuggestion,
}: MessageInputProps) {
  return (
    <form
      className="flex flex-col gap-2 px-4 safe-x"
      style={{
        paddingBottom: "calc(0.75rem + env(safe-area-inset-bottom))",
        paddingTop: "0.75rem",
      }}
      onSubmit={onSubmit}
    >
      {attachments.length > 0 && (
        <div className="flex flex-wrap gap-2 p-3 bg-accent-green/10 border border-dashed border-accent-green/40 rounded-xl mb-2">
          <span className="w-full text-[11px] font-medium text-accent-green uppercase tracking-wider mb-1">
            Ready to send
          </span>
          {attachments.map((attachment, index) => (
            <div
              key={index}
              className="inline-flex items-center gap-1 bg-white/[0.08] rounded-md px-2 py-1 max-w-[200px]"
            >
              <span
                className="text-[13px] text-text-primary truncate"
                title={attachment.name}
              >
                {attachment.name}
              </span>
              <button
                type="button"
                className="w-4 h-4 min-w-4 rounded-full flex items-center justify-center opacity-60 hover:opacity-100"
                onClick={() => onRemoveAttachment(index)}
              >
                <svg
                  className="w-3 h-3 text-text-muted"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="flex items-center gap-2">
        <button
          type="button"
          className="w-10 h-10 min-w-10 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors"
          onClick={onAddAttachment}
        >
          <svg
            className="w-5 h-5 text-text-muted"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
          </svg>
        </button>
        <div className="flex-1 relative flex items-center">
          {emojiSuggestion && (
            <EmojiSuggestions
              query={emojiSuggestion.query}
              onSelect={onEmojiSuggestionSelect}
              onClose={onCloseEmojiSuggestion}
              selectedIndex={suggestionIndex}
            />
          )}
          {showEmojiPicker && (
            <EmojiPicker
              onSelect={onEmojiSelect}
              onClose={onToggleEmojiPicker}
            />
          )}
          <input
            ref={inputRef}
            type="text"
            className="w-full py-2.5 pl-4 pr-11 bg-bg-tertiary border-none rounded-3xl text-text-primary text-[15px] outline-none focus:bg-bg-hover transition-colors placeholder:text-text-muted"
            placeholder={placeholder}
            value={inputValue}
            onChange={onInputChange}
            onKeyDown={onInputKeyDown}
          />
          <button
            type="button"
            className="absolute right-1 w-8 h-8 min-w-8 rounded-full flex items-center justify-center hover:bg-bg-hover"
            onClick={onToggleEmojiPicker}
          >
            <svg
              className="w-4 h-4 text-text-muted"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <circle cx="12" cy="12" r="10" />
              <path d="M8 14s1.5 2 4 2 4-2 4-2" />
              <circle cx="9" cy="9" r="1" fill="currentColor" stroke="none" />
              <circle cx="15" cy="9" r="1" fill="currentColor" stroke="none" />
            </svg>
          </button>
        </div>
      </div>
    </form>
  );
}
