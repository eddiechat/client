import md5 from "md5";
// @ts-expect-error - no types available for browser version
import EmailReplyParser from "email-reply-parser-browser";
import type { Conversation } from "../types";

// Generate a consistent color from a string, using email for consistency
export function getAvatarColor(nameOrEmail: string): string {
  const colors = [
    "#e91e63", // pink
    "#9c27b0", // purple
    "#673ab7", // deep purple
    "#3f51b5", // indigo
    "#2196f3", // blue
    "#03a9f4", // light blue
    "#00bcd4", // cyan
    "#009688", // teal
    "#4caf50", // green
    "#8bc34a", // light green
    "#ff9800", // orange
    "#ff5722", // deep orange
  ];

  // Extract email if present (e.g., "John Doe <john@example.com>")
  // Use email for consistent colors across different display name variations
  const emailMatch = nameOrEmail.match(/<([^>]+)>/);
  const key = emailMatch
    ? emailMatch[1].toLowerCase()
    : nameOrEmail.includes("@")
      ? nameOrEmail.toLowerCase()
      : nameOrEmail;

  let hash = 0;
  for (let i = 0; i < key.length; i++) {
    hash = key.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

// Get initials from a name or email
export function getInitials(name: string): string {
  // Clean up the name (remove email parts if present)
  const cleanName = name.replace(/<[^>]+>/g, "").trim();

  if (!cleanName) return "?";

  // If it's an email address, use first letter of username
  if (cleanName.includes("@")) {
    return cleanName.split("@")[0].charAt(0).toUpperCase();
  }

  // Get initials from name parts
  const parts = cleanName.split(/\s+/).filter(Boolean);
  if (parts.length === 1) {
    return parts[0].charAt(0).toUpperCase();
  }

  return (parts[0].charAt(0) + parts[parts.length - 1].charAt(0)).toUpperCase();
}

// Extract email from a participant string (handles both "Name <email>" and "email" formats)
export function extractEmail(participant: string): string {
  const match = participant.match(/<([^>]+)>/);
  if (match) {
    return match[1].trim().toLowerCase();
  }
  return participant.trim().toLowerCase();
}

// Generate Gravatar URL from email
export function getGravatarUrl(email: string, size: number = 40): string {
  const hash = md5(email.trim().toLowerCase());
  // Use 404 as default to get a 404 if no gravatar exists (we'll handle fallback)
  return `https://www.gravatar.com/avatar/${hash}?s=${size}&d=404`;
}

// Extract first name from a full name
export function getFirstName(name: string): string {
  // Remove any email parts first
  const cleanName = name.replace(/<[^>]+>/g, "").trim();
  if (!cleanName || cleanName.includes("@")) {
    // It's an email address, use username part
    const email = cleanName || name;
    return email.split("@")[0];
  }
  // Return the first word (first name)
  return cleanName.split(/\s+/)[0];
}

// Get display name parts for conversation (first names, excluding user)
export function getConversationNameParts(conversation: Conversation): { name: string; isUser: boolean }[] {
  if (conversation.participant_names.length === 0) {
    return [{ name: "Unknown", isUser: false }];
  }

  const parts: { name: string; isUser: boolean }[] = [];

  if (conversation.user_in_conversation && conversation.participant_names.length > 1) {
    // User is in the conversation - only show other participants (skip index 0 which is the user)
    for (let i = 1; i < conversation.participant_names.length && parts.length < 2; i++) {
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
    for (let i = 0; i < conversation.participant_names.length && parts.length < 2; i++) {
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

// Parse email content to extract the visible reply (removes quoted text, signatures, etc.)
export function parseEmailContent(emailBody: string | undefined | null): string {
  if (!emailBody) return "";

  const parser = new EmailReplyParser().read(emailBody);
  const visibleText = parser.getVisibleText();

  return visibleText.trim();
}
