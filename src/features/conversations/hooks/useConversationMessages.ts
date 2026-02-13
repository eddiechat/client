import { useState, useEffect, useCallback } from "react";
import {
  getCachedConversationMessages,
  fetchMessageBody,
} from "../../../tauri";
import type { Conversation, ChatMessage, Message } from "../../../tauri";

interface UseConversationMessagesResult {
  messages: ChatMessage[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Convert a backend Message to the frontend ChatMessage display format.
 */
function formatMessage(msg: Message): ChatMessage {
  const fromField = msg.from_name
    ? `${msg.from_name} <${msg.from_address}>`
    : msg.from_address;

  // Parse JSON string arrays
  let toAddresses: string[] = [];
  let ccAddresses: string[] = [];
  let flags: string[] = [];
  try { toAddresses = JSON.parse(msg.to_addresses); } catch { /* empty */ }
  try { ccAddresses = JSON.parse(msg.cc_addresses); } catch { /* empty */ }
  try { flags = JSON.parse(msg.imap_flags); } catch { /* empty */ }

  return {
    id: msg.id,
    envelope: {
      id: msg.id,
      from: fromField,
      to: toAddresses,
      cc: ccAddresses,
      subject: msg.subject || "",
      date: new Date(msg.date).toISOString(),
      flags,
      has_attachment: msg.has_attachments,
    },
    headers: [],
    text_body: msg.body_text || undefined,
    html_body: msg.body_html || undefined,
    attachments: [],
  };
}

/**
 * Hook for getting messages in a conversation.
 *
 * Uses the sync engine's cache - messages are fetched from SQLite.
 * If message bodies are not cached, they will be fetched on demand.
 */
export function useConversationMessages(
  conversation: Conversation | null,
  account?: string
): UseConversationMessagesResult {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchMessages = useCallback(async () => {
    if (!conversation) {
      setMessages([]);
      return;
    }

    try {
      setLoading(true);
      setError(null);

      // Fetch from cache using conversation ID (string hash)
      const backendMessages = await getCachedConversationMessages(
        conversation.id,
        account
      );

      // Fetch bodies for messages that don't have them cached
      const messagesWithBodies = await Promise.all(
        backendMessages.map(async (msg) => {
          if (!msg.body_text && !msg.body_html) {
            try {
              const withBody = await fetchMessageBody(msg.id, account);
              return withBody || msg;
            } catch (e) {
              console.warn("Failed to fetch body for message:", msg.id, e);
              return msg;
            }
          }
          return msg;
        })
      );

      setMessages(messagesWithBodies.map(formatMessage));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [conversation, account]);

  useEffect(() => {
    fetchMessages();
  }, [fetchMessages]);

  return {
    messages,
    loading,
    error,
    refresh: fetchMessages,
  };
}
