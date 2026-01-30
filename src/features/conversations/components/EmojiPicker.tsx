import { useState, useRef, useEffect } from "react";
import { emojiCategories, searchEmojis, type Emoji } from "../../../lib/emojiData";

interface EmojiPickerProps {
  onSelect: (emoji: string) => void;
  onClose: () => void;
}

export function EmojiPicker({ onSelect, onClose }: EmojiPickerProps) {
  const [search, setSearch] = useState("");
  const [activeCategory, setActiveCategory] = useState(0);
  const pickerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        pickerRef.current &&
        !pickerRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [onClose]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const handleEmojiClick = (emoji: Emoji) => {
    onSelect(emoji.emoji);
    onClose();
  };

  const searchResults = search ? searchEmojis(search, 50) : null;

  return (
    <div
      ref={pickerRef}
      className="absolute bottom-full left-0 mb-2 w-72 max-h-80 bg-bg-secondary rounded-xl shadow-xl border border-divider flex flex-col overflow-hidden z-50"
    >
      {/* Search bar */}
      <div className="flex items-center gap-2 p-2.5 border-b border-divider">
        <svg
          className="w-4 h-4 text-text-muted shrink-0"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.35-4.35" />
        </svg>
        <input
          ref={searchInputRef}
          type="text"
          className="flex-1 bg-transparent border-none text-text-primary text-sm outline-none placeholder:text-text-muted"
          placeholder="Search emoji..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {/* Category tabs */}
      {!search && (
        <div className="flex border-b border-divider">
          {emojiCategories.map((category, index) => (
            <button
              type="button"
              key={category.name}
              className={`flex-1 py-2 text-base transition-colors ${
                activeCategory === index ? "bg-bg-hover" : "hover:bg-bg-hover"
              }`}
              onClick={() => setActiveCategory(index)}
              title={category.name}
            >
              {category.icon}
            </button>
          ))}
        </div>
      )}

      {/* Emoji grid */}
      <div className="flex-1 overflow-y-auto p-2">
        {searchResults ? (
          searchResults.length > 0 ? (
            <div className="grid grid-cols-8 gap-0.5">
              {searchResults.map((emoji) => (
                <button
                  type="button"
                  key={emoji.emoji + emoji.name}
                  className="w-8 h-8 flex items-center justify-center text-xl rounded hover:bg-bg-hover transition-colors"
                  onClick={() => handleEmojiClick(emoji)}
                  title={`:${emoji.name}:`}
                >
                  {emoji.emoji}
                </button>
              ))}
            </div>
          ) : (
            <div className="py-6 text-center text-text-muted text-sm">
              No emojis found
            </div>
          )
        ) : (
          <>
            <div className="text-xs font-medium text-text-muted uppercase tracking-wider mb-2 px-1">
              {emojiCategories[activeCategory].name}
            </div>
            <div className="grid grid-cols-8 gap-0.5">
              {emojiCategories[activeCategory].emojis.map((emoji) => (
                <button
                  type="button"
                  key={emoji.emoji + emoji.name}
                  className="w-8 h-8 flex items-center justify-center text-xl rounded hover:bg-bg-hover transition-colors"
                  onClick={() => handleEmojiClick(emoji)}
                  title={`:${emoji.name}:`}
                >
                  {emoji.emoji}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

// Emoji suggestions popup for colon autocomplete
interface EmojiSuggestionsProps {
  query: string;
  onSelect: (emoji: string, name: string) => void;
  onClose: () => void;
  selectedIndex: number;
}

export function EmojiSuggestions({
  query,
  onSelect,
  onClose,
  selectedIndex,
}: EmojiSuggestionsProps) {
  const suggestions = searchEmojis(query, 8);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        suggestionsRef.current &&
        !suggestionsRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [onClose]);

  if (suggestions.length === 0) return null;

  return (
    <div
      ref={suggestionsRef}
      className="absolute bottom-full left-0 mb-2 w-56 bg-bg-secondary rounded-xl shadow-xl border border-divider overflow-hidden z-50"
    >
      {suggestions.map((emoji, index) => (
        <button
          type="button"
          key={emoji.emoji + emoji.name}
          className={`w-full flex items-center gap-2.5 px-3 py-2 text-left transition-colors ${
            index === selectedIndex ? "bg-bg-hover" : "hover:bg-bg-hover"
          }`}
          onClick={() => onSelect(emoji.emoji, emoji.name)}
        >
          <span className="text-lg">{emoji.emoji}</span>
          <span className="text-sm text-text-secondary truncate">
            :{emoji.name}:
          </span>
        </button>
      ))}
    </div>
  );
}
