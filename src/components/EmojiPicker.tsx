import { useState, useRef, useEffect } from "react";
import { emojiCategories, searchEmojis, type Emoji } from "../lib/emojiData";

interface EmojiPickerProps {
  onSelect: (emoji: string) => void;
  onClose: () => void;
}

export function EmojiPicker({ onSelect, onClose }: EmojiPickerProps) {
  const [search, setSearch] = useState("");
  const [activeCategory, setActiveCategory] = useState(0);
  const pickerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Focus search input on mount
  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  // Close on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [onClose]);

  // Close on escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
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
    <div className="emoji-picker" ref={pickerRef}>
      {/* Search bar */}
      <div className="emoji-picker-search">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.35-4.35" />
        </svg>
        <input
          ref={searchInputRef}
          type="text"
          placeholder="Search emoji..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {/* Category tabs */}
      {!search && (
        <div className="emoji-picker-categories">
          {emojiCategories.map((category, index) => (
            <button
              type="button"
              key={category.name}
              className={`emoji-category-tab ${activeCategory === index ? "active" : ""}`}
              onClick={() => setActiveCategory(index)}
              title={category.name}
            >
              {category.icon}
            </button>
          ))}
        </div>
      )}

      {/* Emoji grid */}
      <div className="emoji-picker-content">
        {searchResults ? (
          // Search results
          searchResults.length > 0 ? (
            <div className="emoji-grid">
              {searchResults.map((emoji) => (
                <button
                  type="button"
                  key={emoji.emoji + emoji.name}
                  className="emoji-item"
                  onClick={() => handleEmojiClick(emoji)}
                  title={`:${emoji.name}:`}
                >
                  {emoji.emoji}
                </button>
              ))}
            </div>
          ) : (
            <div className="emoji-no-results">No emojis found</div>
          )
        ) : (
          // Category view
          <>
            <div className="emoji-category-header">
              {emojiCategories[activeCategory].name}
            </div>
            <div className="emoji-grid">
              {emojiCategories[activeCategory].emojis.map((emoji) => (
                <button
                  type="button"
                  key={emoji.emoji + emoji.name}
                  className="emoji-item"
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

  // Close on click outside
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

  if (suggestions.length === 0) {
    return null;
  }

  return (
    <div className="emoji-suggestions" ref={suggestionsRef}>
      {suggestions.map((emoji, index) => (
        <button
          type="button"
          key={emoji.emoji + emoji.name}
          className={`emoji-suggestion-item ${index === selectedIndex ? "selected" : ""}`}
          onClick={() => onSelect(emoji.emoji, emoji.name)}
        >
          <span className="emoji-suggestion-emoji">{emoji.emoji}</span>
          <span className="emoji-suggestion-name">:{emoji.name}:</span>
        </button>
      ))}
    </div>
  );
}
