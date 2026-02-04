import { useState, useEffect, useCallback } from "react";
import {
  getCachedConversationMessages,
  fetchMessageBody,
  getConversationMessages,
} from "../../../tauri";
import type { Conversation, ChatMessage, CachedChatMessage } from "../../../tauri";

interface UseConversationMessagesResult {
  messages: ChatMessage[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Convert cached message to display format.
 */
function formatMessage(cached: CachedChatMessage): ChatMessage {
  const fromField = cached.from_name
    ? `${cached.from_name} <${cached.from_address}>`
    : cached.from_address;

  // Debug logging
  if (import.meta.env.DEV) {
    console.log('[formatMessage]', {
      from_name: cached.from_name,
      from_address: cached.from_address,
      formatted_from: fromField,
      uid: cached.uid
    });
  }

  return {
    id: `${cached.folder}:${cached.uid}`,
    envelope: {
      id: cached.uid.toString(),
      message_id: cached.message_id || undefined,
      in_reply_to: undefined,
      from: fromField,
      to: cached.to_addresses,
      cc: cached.cc_addresses,
      subject: cached.subject || "",
      date: cached.date || "",
      flags: cached.flags,
      has_attachment: cached.has_attachment,
    },
    headers: [],
    text_body: cached.text_body || undefined,
    html_body: cached.html_body || undefined,
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
    const cachedId = conversation?._cached_id;

    if (!conversation && !cachedId) {
      setMessages([]);
      return;
    }

    try {
      setLoading(true);
      setError(null);

      if (cachedId) {
        // Fetch from cache using conversation ID
        const cachedMessages = await getCachedConversationMessages(
          cachedId,
          account
        );

        // Fetch bodies for messages that don't have them cached
        const messagesWithBodies = await Promise.all(
          cachedMessages.map(async (msg) => {
            if (!msg.body_cached && !msg.text_body && !msg.html_body) {
              try {
                const withBody = await fetchMessageBody(msg.id, account);
                return withBody;
              } catch (e) {
                console.warn("Failed to fetch body for message:", msg.id, e);
                return msg;
              }
            }
            return msg;
          })
        );

        setMessages(messagesWithBodies.map(formatMessage));
      } else if (conversation?.message_ids && conversation.message_ids.length > 0) {
        // Fallback to old method if no cached ID
        const messageList = await getConversationMessages(
          conversation.message_ids,
          account
        );
        // The getConversationMessages returns CachedMessage[], convert to Message[]
        setMessages(
          messageList.map((m) => ({
            id: `${m.folder}:${m.uid}`,
            envelope: {
              id: m.uid.toString(),
              message_id: m.message_id || undefined,
              in_reply_to: undefined,
              from: m.from_name
                ? `${m.from_name} <${m.from_address}>`
                : m.from_address,
              to: m.to_addresses,
              cc: m.cc_addresses,
              subject: m.subject || "",
              date: m.date || "",
              flags: m.flags,
              has_attachment: m.has_attachment,
            },
            headers: [],
            text_body: m.text_body || undefined,
            html_body: m.html_body || undefined,
            attachments: [],
          }))
        );
      } else {
        setMessages([]);
      }
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
