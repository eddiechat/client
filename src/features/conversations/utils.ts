import type { Conversation } from "../../tauri";
import { getFirstName } from "../../shared";

/**
 * Get display name parts for conversation (first names, excluding user).
 */
export function getConversationNameParts(
  conversation: Conversation
): { name: string; isUser: boolean }[] {
  if (conversation.participant_names.length === 0) {
    return [{ name: "Unknown", isUser: false }];
  }

  const parts: { name: string; isUser: boolean }[] = [];

  if (
    conversation.user_in_conversation &&
    conversation.participant_names.length > 1
  ) {
    // User is in the conversation - only show other participants (skip index 0 which is the user)
    for (
      let i = 1;
      i < conversation.participant_names.length && parts.length < 2;
      i++
    ) {
      const firstName = getFirstName(conversation.participant_names[i]);
      parts.push({ name: firstName, isUser: false });
    }

    // Handle more than 3 participants (user + 2 others shown + remaining)
    if (conversation.participant_names.length > 3) {
      const remaining = conversation.participant_names.length - 3;
      parts.push({ name: `+${remaining}`, isUser: false });
    }
  } else {
    // User is not in this conversation - just show the participants
    for (
      let i = 0;
      i < conversation.participant_names.length && parts.length < 2;
      i++
    ) {
      const firstName = getFirstName(conversation.participant_names[i]);
      parts.push({ name: firstName, isUser: false });
    }

    // Handle more than 2 participants
    if (conversation.participant_names.length > 2) {
      const remaining = conversation.participant_names.length - 2;
      parts.push({ name: `+${remaining}`, isUser: false });
    }
  }

  return parts;
}

/**
 * Get conversation display name as a single string.
 */
export function getConversationName(conversation: Conversation): string {
  return getConversationNameParts(conversation)
    .map((p) => p.name)
    .join(", ");
}

/**
 * Get tooltip text showing full names and emails for conversation header.
 */
export function getHeaderAvatarTooltip(conversation: Conversation): string {
  return conversation.participants
    .map((email, index) => {
      const name = conversation.participant_names[index];
      return name && name !== email && !name.includes("@")
        ? `${name} <${email}>`
        : email;
    })
    .join("\n");
}

/**
 * Get sender name from a "Name <email>" string.
 */
export function getSenderName(from: string): string {
  const cleanName = from.replace(/<[^>]+>/g, "").trim();
  if (!cleanName || cleanName.includes("@")) {
    const match = from.match(/<([^>]+)>/);
    const email = match ? match[1] : from;
    return email.split("@")[0];
  }
  return cleanName;
}

/**
 * Get tooltip text for message avatar.
 */
export function getAvatarTooltip(from: string, messageId?: string): string {
  const name = getSenderName(from);
  const emailMatch = from.match(/<([^>]+)>/);
  const email = emailMatch
    ? emailMatch[1]
    : from.replace(/^[^<]*/, "").trim();
  let tooltip =
    name && email && name !== email && !name.includes("@")
      ? `${name} <${email}>`
      : email || from;
  if (import.meta.env.DEV && messageId) tooltip += `\nID: ${messageId}`;
  return tooltip;
}

/**
 * Check if a message is outgoing (sent by the current user).
 */
export function isOutgoing(
  from: string,
  currentAccountEmail?: string
): boolean {
  if (!currentAccountEmail) return false;
  const fromEmail = from.toLowerCase();
  const accountEmail = currentAccountEmail.toLowerCase();
  return (
    fromEmail.includes(accountEmail) ||
    accountEmail.includes(fromEmail.replace(/<|>/g, "").split("@")[0])
  );
}
