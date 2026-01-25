import { useState, useEffect, useCallback } from "react";
import * as api from "../lib/api";
import type { Conversation, Message } from "../types";

// Hook for managing conversations (Signal-like messaging view)
export function useConversations(account?: string) {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchConversations = useCallback(async () => {
    try {
      setLoading(true);
      const conversationList = await api.listConversations(account);
      setConversations(conversationList);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [account]);

  useEffect(() => {
    fetchConversations();
  }, [fetchConversations]);

  return {
    conversations,
    loading,
    error,
    refresh: fetchConversations,
    listConversations: fetchConversations,
  };
}

// Hook for getting messages in a conversation
export function useConversationMessages(
  messageIds: string[],
  account?: string
) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchMessages = useCallback(async () => {
    if (messageIds.length === 0) {
      setMessages([]);
      return;
    }

    try {
      setLoading(true);
      const messageList = await api.getConversationMessages(messageIds, account);
      setMessages(messageList);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [messageIds, account]);

  useEffect(() => {
    fetchMessages();
  }, [fetchMessages]);

  return {
    messages,
    loading,
    error,
    refresh: fetchMessages,
    getConversationMessages: fetchMessages,
  };
}
