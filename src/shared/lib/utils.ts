/**
 * Shared utility functions used across the application.
 */

import md5 from "md5";
// @ts-expect-error - no types available for browser version
import EmailReplyParser from "email-reply-parser-browser";

// ========== Avatar Utilities ==========

/**
 * Generate a consistent color from a string, using email for consistency.
 */
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

/**
 * Get initials from a name or email.
 */
export function getInitials(name: string): string {
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

/**
 * Generate Gravatar URL from email.
 */
export function getGravatarUrl(email: string, size: number = 40): string {
  const hash = md5(email.trim().toLowerCase());
  // Use 404 to trigger error handler when no gravatar exists (shows initials as fallback)
  return `https://www.gravatar.com/avatar/${hash}?s=${size}&d=404`;
}

// ========== Email Utilities ==========

/**
 * Extract email from a participant string.
 * Handles both "Name <email>" and "email" formats.
 */
export function extractEmail(participant: string): string {
  const match = participant.match(/<([^>]+)>/);
  if (match) {
    return match[1].trim().toLowerCase();
  }
  return participant.trim().toLowerCase();
}

/**
 * Extract first name from a full name.
 */
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

// ========== Email Content Parsing ==========

/**
 * Parse email content to extract the visible reply.
 * Removes quoted text, signatures, etc.
 * Truncates to 20 lines maximum with ellipsis if longer.
 */
export function parseEmailContent(emailBody: string | undefined | null): string {
  if (!emailBody) return "";

  const parser = new EmailReplyParser().read(emailBody);
  const visibleText = parser.getVisibleText().trim();

  // Truncate to 20 lines max
  const lines = visibleText.split('\n');
  if (lines.length > 20) {
    return lines.slice(0, 20).join('\n') + '\n...';
  }

  return visibleText;
}

/**
 * Check if a message has content that differs from the rendered (parsed) content.
 * This indicates the message can be expanded to show the full original.
 */
export function hasExpandableContent(
  textBody: string | undefined | null,
  htmlBody: string | undefined | null
): boolean {
  if (!textBody && !htmlBody) return false;

  // If there's HTML content, it's expandable
  if (htmlBody && htmlBody.trim().length > 0) return true;

  // Compare parsed content with original
  if (textBody) {
    const parsed = parseEmailContent(textBody);
    const original = textBody.trim();
    // Check if content differs (accounting for whitespace normalization)
    return parsed !== original && original.length > parsed.length;
  }

  return false;
}

// ========== Date/Time Utilities ==========

/**
 * Format a message timestamp for display.
 */
export function formatMessageTime(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

/**
 * Format a date separator label.
 */
export function formatDateSeparator(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) return date.toLocaleDateString([], { weekday: "long" });
  return date.toLocaleDateString([], {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: now.getFullYear() !== date.getFullYear() ? "numeric" : undefined,
  });
}

/**
 * Check if two dates are on different days.
 */
export function isDifferentDay(date1: string, date2: string): boolean {
  const d1 = new Date(date1);
  const d2 = new Date(date2);
  return (
    d1.getFullYear() !== d2.getFullYear() ||
    d1.getMonth() !== d2.getMonth() ||
    d1.getDate() !== d2.getDate()
  );
}

// ========== File Utilities ==========

/**
 * Format file size in human-readable format.
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

/**
 * Get file icon type from MIME type.
 */
export function getFileIconType(mimeType: string): string {
  if (mimeType.startsWith("image/")) return "image";
  if (mimeType.startsWith("video/")) return "video";
  if (mimeType.startsWith("audio/")) return "audio";
  if (mimeType.includes("pdf")) return "pdf";
  if (mimeType.includes("zip") || mimeType.includes("compressed")) return "archive";
  if (mimeType.includes("word") || mimeType.includes("document")) return "document";
  if (mimeType.includes("sheet") || mimeType.includes("excel")) return "spreadsheet";
  return "file";
}
