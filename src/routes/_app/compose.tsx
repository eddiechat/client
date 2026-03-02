import { useState, useEffect, useRef, useCallback } from "react";
import { createFileRoute, useRouter, useNavigate } from "@tanstack/react-router";
import { useAuth, useData } from "../../shared/context";
import { searchEntities, getUserAliases } from "../../tauri";
import type { EntityResult, AliasInfo } from "../../tauri";
import { participantEmails } from "../../shared/lib";
import { Avatar } from "../../shared/components";

export const Route = createFileRoute("/_app/compose")({
  component: ComposeScreen,
});

function ComposeScreen() {
  const router = useRouter();
  const navigate = useNavigate();
  const { accountId, email } = useAuth();

  const [recipients, setRecipients] = useState<EntityResult[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [suggestions, setSuggestions] = useState<EntityResult[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [aliases, setAliases] = useState<AliasInfo[]>([]);
  const [fromEmail, setFromEmail] = useState(email || "");
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    if (!accountId) return;
    getUserAliases(accountId).then((a) => {
      setAliases(a);
      const primary = a.find((x) => x.is_primary);
      if (primary) setFromEmail(primary.email);
    });
  }, [accountId]);

  const handleSearch = useCallback((query: string) => {
    if (!accountId || query.length < 2) {
      setSuggestions([]);
      setShowSuggestions(false);
      return;
    }
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      searchEntities(accountId, query).then((results) => {
        // Filter out already-added recipients
        const addedEmails = new Set(recipients.map((r) => r.email.toLowerCase()));
        setSuggestions(results.filter((r) => !addedEmails.has(r.email.toLowerCase())));
        setShowSuggestions(true);
      });
    }, 200);
  }, [accountId, recipients]);

  const addRecipient = (entity: EntityResult) => {
    setRecipients((prev) => [...prev, entity]);
    setInputValue("");
    setSuggestions([]);
    setShowSuggestions(false);
    inputRef.current?.focus();
  };

  const addRawEmail = (emailAddr: string) => {
    const trimmed = emailAddr.trim();
    if (!trimmed || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmed)) return;
    if (recipients.some((r) => r.email.toLowerCase() === trimmed.toLowerCase())) return;
    addRecipient({ email: trimmed, display_name: null, trust_level: "unknown" });
  };

  const removeRecipient = (index: number) => {
    setRecipients((prev) => prev.filter((_, i) => i !== index));
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.key === "Enter" || e.key === "," || e.key === " ") && inputValue.trim()) {
      e.preventDefault();
      // If suggestions are showing, pick first one
      if (showSuggestions && suggestions.length > 0) {
        addRecipient(suggestions[0]);
      } else {
        addRawEmail(inputValue);
      }
    }
    if (e.key === "Backspace" && !inputValue && recipients.length > 0) {
      removeRecipient(recipients.length - 1);
    }
    if (e.key === "Enter" && !inputValue && recipients.length > 0) {
      e.preventDefault();
      handleStartConversation();
    }
  };

  const { conversations } = useData();

  const handleStartConversation = async () => {
    if (recipients.length === 0 || !accountId) return;

    const toAddrs = recipients.map((r) => r.email);
    const toSet = new Set(toAddrs.map((e) => e.toLowerCase()));

    // Check if a conversation already exists with these participants
    const existing = conversations.find((c) => {
      const emails = participantEmails(c);
      if (emails.length !== toSet.size) return false;
      return emails.every((e) => toSet.has(e.toLowerCase()));
    });

    if (existing) {
      navigate({
        to: "/conversation/$id",
        params: { id: existing.id },
        search: { compose: "1" } as Record<string, string>,
        replace: true,
      });
      return;
    }

    const subject = `${fromEmail.split("@")[0]} via Eddie`;

    navigate({
      to: "/conversation/$id",
      params: { id: "__new__" },
      search: {
        to: toAddrs.join(","),
        subject,
        from: fromEmail,
      } as Record<string, string>,
      replace: true,
    });
  };

  const cycleFrom = () => {
    if (aliases.length <= 1) return;
    const currentIdx = aliases.findIndex((a) => a.email === fromEmail);
    const nextIdx = (currentIdx + 1) % aliases.length;
    setFromEmail(aliases[nextIdx].email);
  };

  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-gradient)" }}>
      {/* Header */}
      <div
        className="flex items-center gap-3 px-4 pb-3 shrink-0 bg-bg-secondary"
        style={{
          paddingTop: 'calc(0.75rem + env(safe-area-inset-top, 0px))',
          boxShadow: '0 2px 12px rgba(0,0,0,0.06)',
        }}
      >
        <button
          className="border-none bg-transparent text-[26px] cursor-pointer text-text-secondary w-10 h-10 flex items-center justify-center rounded-[10px] hover:bg-bg-tertiary active:scale-95 transition-all font-bold -ml-1"
          onClick={() => router.history.back()}
        >
          &#8249;
        </button>
        <span className="font-extrabold text-[16px] text-text-primary" style={{ letterSpacing: "-0.2px" }}>
          New Message
        </span>
      </div>

      <div className="flex-1 overflow-y-auto flex flex-col px-4 py-4 gap-4">
        {/* From selector (if aliases exist) */}
        {aliases.length > 1 && (
          <div className="flex items-center gap-2">
            <span className="text-[12px] font-bold text-text-dim tracking-wide">FROM</span>
            <button
              className="px-3 py-1.5 rounded-full bg-bg-tertiary border border-divider text-[13px] font-medium text-text-primary cursor-pointer hover:bg-bg-secondary transition"
              onClick={cycleFrom}
            >
              {fromEmail}
            </button>
          </div>
        )}

        {/* Recipients */}
        <div>
          <span className="text-[12px] font-bold text-text-dim tracking-wide mb-2 block">TO</span>
          <div
            className="flex flex-wrap items-center gap-1.5 p-2.5 rounded-xl bg-bg-secondary border border-divider min-h-[44px] cursor-text"
            onClick={() => inputRef.current?.focus()}
          >
            {recipients.map((r, i) => (
              <span
                key={i}
                className="flex items-center gap-1 px-2.5 py-1 rounded-full bg-accent-green/15 text-[13px] font-medium text-text-primary border border-accent-green/30"
              >
                {r.display_name || r.email}
                <button
                  className="bg-transparent border-none text-text-dim cursor-pointer text-[16px] leading-none p-0 hover:text-accent-red"
                  onClick={(e) => { e.stopPropagation(); removeRecipient(i); }}
                >
                  &times;
                </button>
              </span>
            ))}
            <input
              ref={inputRef}
              className="flex-1 min-w-[120px] border-none outline-none bg-transparent text-[14px] text-text-primary placeholder:text-text-dim"
              placeholder={recipients.length === 0 ? "Type a name or email..." : ""}
              value={inputValue}
              onChange={(e) => { setInputValue(e.target.value); handleSearch(e.target.value); }}
              onKeyDown={handleKeyDown}
              onBlur={() => { setTimeout(() => setShowSuggestions(false), 200); }}
              autoFocus
            />
          </div>

          {/* Suggestions dropdown */}
          {showSuggestions && suggestions.length > 0 && (
            <div className="mt-1 rounded-xl bg-bg-secondary border border-divider overflow-hidden" style={{ boxShadow: '0 4px 16px rgba(0,0,0,0.1)' }}>
              {suggestions.map((s, i) => (
                <button
                  key={i}
                  className="w-full flex items-center gap-3 px-3.5 py-2.5 bg-transparent border-none cursor-pointer text-left hover:bg-bg-tertiary transition"
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => addRecipient(s)}
                >
                  <Avatar name={s.display_name || s.email} email={s.email} size={9} fontSize="text-[12px]" className="shrink-0" />
                  <div className="min-w-0">
                    {s.display_name && (
                      <div className="text-[14px] font-medium text-text-primary truncate">{s.display_name}</div>
                    )}
                    <div className="text-[12px] text-text-muted truncate">{s.email}</div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Instructions */}
        <div className="flex-1 flex items-center justify-center">
          <p className="text-[14px] text-text-dim text-center leading-relaxed">
            Add recipients and press Enter to start a conversation.
          </p>
        </div>
      </div>

      {/* Bottom action */}
      <div
        className="flex items-center gap-2 px-4 pt-3 shrink-0 bg-bg-secondary"
        style={{
          paddingBottom: 'calc(0.75rem + env(safe-area-inset-bottom, 0px))',
          boxShadow: '0 -2px 12px rgba(0,0,0,0.05)',
        }}
      >
        <button
          className="flex-1 py-3.5 border-none rounded-[12px] bg-accent-green text-white text-[15px] font-extrabold cursor-pointer hover:brightness-95 disabled:opacity-40 disabled:cursor-not-allowed transition"
          disabled={recipients.length === 0}
          onClick={handleStartConversation}
        >
          Start Conversation
        </button>
      </div>
    </div>
  );
}
