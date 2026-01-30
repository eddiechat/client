import { useState } from "react";
import type { Conversation } from "../../../tauri";
import { ChatMessage } from "./ChatMessage";

type FilterType = "chats" | "important" | "requests" | "all";

interface ChatMessagesProps {
  conversations: Conversation[];
  selectedId: string | null;
  onSelect: (conversation: Conversation) => void;
  loading?: boolean;
  searchQuery: string;
  onSearchChange: (query: string) => void;
  currentAccountEmail?: string;
}

export function ChatMessages({
  conversations,
  selectedId,
  onSelect,
  loading,
  searchQuery,
  onSearchChange,
  currentAccountEmail,
}: ChatMessagesProps) {
  const [activeFilter, setActiveFilter] = useState<FilterType>("chats");

  // Filter conversations by search query
  const filteredConversations = searchQuery
    ? conversations.filter((conv) => {
        const searchLower = searchQuery.toLowerCase();
        return (
          conv.participant_names.some((name) =>
            name.toLowerCase().includes(searchLower)
          ) || conv.last_message_preview.toLowerCase().includes(searchLower)
        );
      })
    : conversations;

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Search */}
      <div className="px-3 pb-3 safe-x">
        <div className="relative flex items-center">
          <svg
            className="absolute left-3 w-4 h-4 text-text-muted pointer-events-none"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.35-4.35" />
          </svg>
          <input
            type="text"
            className="w-full py-2.5 pl-10 pr-3 bg-bg-tertiary border-none rounded-xl text-text-primary text-[15px] outline-none focus:bg-bg-hover transition-colors placeholder:text-text-muted"
            placeholder="Search"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
          />
        </div>
        <div className="flex gap-2 mt-3">
          <button
            className={`px-3.5 py-2 rounded-full text-sm font-medium transition-all ${
              activeFilter === "chats"
                ? "bg-white text-bg-primary"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover hover:text-text-primary"
            }`}
            onClick={() => setActiveFilter("chats")}
          >
            Connections
          </button>
          <button
            className={`px-3.5 py-2 rounded-full text-sm font-medium transition-all ${
              activeFilter === "requests"
                ? "bg-white text-bg-primary"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover hover:text-text-primary"
            }`}
            onClick={() => setActiveFilter("requests")}
          >
            Strangers
          </button>
          <button
            className={`px-3.5 py-2 rounded-full text-sm font-medium transition-all ${
              activeFilter === "all"
                ? "bg-white text-bg-primary"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover hover:text-text-primary"
            }`}
            onClick={() => setActiveFilter("all")}
          >
            All
          </button>
        </div>
      </div>

      {/* Chat list content */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden safe-bottom">
        {loading ? (
          <div className="flex flex-col items-center justify-center py-10 gap-3 text-text-muted text-sm">
            <div className="spinner" />
            <span>Loading conversations...</span>
          </div>
        ) : filteredConversations.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 text-text-muted text-sm">
            {searchQuery ? "No conversations found" : "No conversations yet"}
          </div>
        ) : (
          filteredConversations.map((conversation) => (
            <ChatMessage
              key={conversation.id}
              conversation={conversation}
              isSelected={selectedId === conversation.id}
              onSelect={onSelect}
              currentAccountEmail={currentAccountEmail}
            />
          ))
        )}
      </div>
    </div>
  );
}
