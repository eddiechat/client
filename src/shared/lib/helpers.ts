import type { Conversation, Message } from "../../tauri";

export function hash(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = ((h << 5) - h + s.charCodeAt(i)) | 0;
  return Math.abs(h);
}

const COLOR_SETS = [
  ["#2c2775", "#7873b3", "#8ac4eb", "#aedcc5"],
  ["#feefb0", "#fedb55", "#71cbe5", "#0badd0"],
  ["#869bcd", "#fdcee5", "#fbf6ba", "#6eb7df"],
  ["#fdf8df", "#f8c568", "#dd6b25", "#6c1148"],
  ["#f9cf82", "#e35d25", "#b71e75", "#63246a"],
  ["#f6c952", "#f8e8b2", "#0c6748", "#76bc79"],
  ["#f5f8f9", "#f4ce17", "#476058", "#46464d"],
  ["#e2c69e", "#ae8260", "#833d3e", "#312d29"],
  ["#182d58", "#d9c0a0", "#eadbc7", "#fdfcf8"],
  ["#ad6eae", "#a4dcf9", "#cbecf7", "#fefad3"],
  ["#9c2d21", "#fdc75a", "#1e275c", "#2b4d84"],
  ["#b5c092", "#f8dcba", "#ddac7f", "#b89570"],
  ["#746aab", "#ab88bc", "#e2add2", "#fde4e3"],
  ["#8c322e", "#de5643", "#fdc26e", "#4791b0"],
  ["#141d48", "#3cb170", "#9bd1ab", "#fdf6df"],
  ["#fccdd0", "#dfadac", "#ca8688", "#a67778"],
  ["#cbe090", "#7ecac7", "#1478ac", "#144173"],
  ["#ddd0bc", "#928976", "#3a5d71", "#103249"],
  ["#f57522", "#e0ded1", "#534c42", "#310e2e"],
  ["#ea7db2", "#ad62ab", "#7856a5", "#15479b"],
];

const ALL_COLORS = COLOR_SETS.flat();

function charCodeSum(name: string): number {
  return name.split("").reduce((a, c) => a + c.charCodeAt(0), 0);
}

export function avatarBg(name: string): string {
  return ALL_COLORS[charCodeSum(name) % ALL_COLORS.length];
}

/** Pick a random palette based on a group hash, then assign colors within it. */
export function avatarGroupPalette(groupHash: number): string[] {
  return COLOR_SETS[groupHash % COLOR_SETS.length];
}

// ── Conversation color store ────────────────────────────────────────
// Maps conversationId → { palette, emailColors: email→hex }.
// Populated when rendering list views, consumed when inside a conversation.
interface StoredColors {
  palette: string[];
  emailColors: Map<string, string>;
}
const colorStore = new Map<string, StoredColors>();

/** Store the palette and per-email color mapping for a conversation. */
export function storeConversationColors(
  conversationId: string,
  palette: string[],
  participants: [string, string][], // [email, name] pairs in render order
): void {
  const emailColors = new Map<string, string>();
  participants.forEach(([email], i) => {
    emailColors.set(email.toLowerCase(), palette[i % palette.length]);
  });
  colorStore.set(conversationId, { palette, emailColors });
}

/** Get the color for a specific email in a conversation. */
export function getConversationColor(conversationId: string, email: string): string | undefined {
  return colorStore.get(conversationId)?.emailColors.get(email.toLowerCase());
}

/** Get the full stored palette for a conversation. */
export function getStoredPalette(conversationId: string): string[] | undefined {
  return colorStore.get(conversationId)?.palette;
}

export function avatarBorder(name: string): string {
  return ALL_COLORS[charCodeSum(name) % ALL_COLORS.length];
}

/** Return white or dark text depending on perceived luminance of the background. */
export function avatarTextColor(name: string): string {
  const bg = avatarBg(name);
  return luminance(bg) > 0.55 ? "#1a1a1a" : "#fff";
}

export function textColorForBg(hex: string): string {
  return luminance(hex) > 0.55 ? "#1a1a1a" : "#fff";
}

function luminance(hex: string): number {
  const r = parseInt(hex.slice(1, 3), 16) / 255;
  const g = parseInt(hex.slice(3, 5), 16) / 255;
  const b = parseInt(hex.slice(5, 7), 16) / 255;
  return 0.299 * r + 0.587 * g + 0.114 * b;
}

export function firstName(name: string): string {
  const s = name.trim();
  if (s.includes("@")) return s.split("@")[0];
  return s.split(/\s+/)[0];
}

export function initials(name: string) {
  const p = name.trim().split(/[\s@]+/);
  if (p.length >= 2) return (p[0][0] + p[1][0]).toUpperCase();
  return name.slice(0, 2).toUpperCase();
}

export function relTime(ts: number) {
  const m = Math.floor((Date.now() - ts) / 60000);
  if (m < 1) return "now";
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

export function fmtTime(ts: number) {
  const d = new Date(ts);
  const now = new Date();
  if (
    d.getFullYear() !== now.getFullYear() ||
    d.getMonth() !== now.getMonth() ||
    d.getDate() !== now.getDate()
  ) {
    return d.toLocaleDateString([], {
      day: "numeric",
      month: "short",
    });
  }
  return d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

export function fmtDate(ts: number) {
  const d = new Date(ts);
  return d.toLocaleDateString([], {
    weekday: "short",
    year: "numeric",
    month: "short",
    day: "numeric",
  }) + " " + fmtTime(ts);
}

export function displayName(c: Conversation): string {
  if (c.participant_names) {
    try {
      const v = Object.values(
        JSON.parse(c.participant_names) as Record<string, string>
      ).filter(Boolean);
      if (v.length) return v.map(firstName).join(", ");
    } catch {
      /* ignore */
    }
  }
  return c.participant_key;
}

export function participantCount(c: Conversation): number {
  if (!c.participant_names) return 1;
  try {
    return Object.keys(
      JSON.parse(c.participant_names) as Record<string, string>
    ).length;
  } catch {
    return 1;
  }
}

export function participantEmails(c: Conversation): string[] {
  if (!c.participant_names) return [c.participant_key];
  try {
    return Object.keys(
      JSON.parse(c.participant_names) as Record<string, string>
    );
  } catch {
    return [c.participant_key];
  }
}

export function participantNames(c: Conversation): string[] {
  if (!c.participant_names) return [c.participant_key];
  try {
    return Object.values(
      JSON.parse(c.participant_names) as Record<string, string>
    ).filter(Boolean);
  } catch {
    return [c.participant_key];
  }
}

export function participantEntries(c: Conversation): [string, string][] {
  if (!c.participant_names) return [[c.participant_key, c.participant_key]];
  try {
    return Object.entries(
      JSON.parse(c.participant_names) as Record<string, string>
    );
  } catch {
    return [[c.participant_key, c.participant_key]];
  }
}

export function previewPrefix(c: Conversation): string {
  const text = c.last_message_preview || "";
  if (!text) return "";
  if (c.last_message_is_sent) return `me: ${text}`;
  if (c.last_message_from_name) return `${firstName(c.last_message_from_name)}: ${text}`;
  return text;
}

export function dedup(msgs: Message[]): Message[] {
  const seen = new Set<string>();
  return msgs.filter((m) => {
    const key = `${m.from_address}|${m.date}|${(m.distilled_text || m.body_text || m.subject || "").slice(0, 100)}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export const LINE_EMOJIS = [
  "\u2708\uFE0F", "\uD83D\uDCBC", "\uD83D\uDD28", "\uD83C\uDF89",
  "\uD83D\uDCC1", "\uD83D\uDCC5", "\uD83D\uDCB0", "\uD83D\uDCE6",
  "\uD83C\uDFAF", "\uD83D\uDE80", "\uD83C\uDF1F", "\uD83D\uDD2C",
  "\uD83C\uDFA8", "\uD83C\uDF0D", "\uD83D\uDCA1", "\uD83D\uDCDA",
  "\u2615", "\uD83C\uDFE0", "\uD83D\uDEE0\uFE0F", "\uD83C\uDFC6",
];

export const LINE_COLORS = [
  "#FF5A5F", "#4A90E2", "#43B89C", "#9B72CF",
  "#FF9F1C", "#2EC4B6", "#FF6584", "#6D28D9",
];

export function lineEmoji(name: string): string {
  return LINE_EMOJIS[hash(name) % LINE_EMOJIS.length];
}

export function lineColor(name: string): string {
  return LINE_COLORS[hash(name) % LINE_COLORS.length];
}

export function parseAddresses(json: string): string {
  try {
    const arr = JSON.parse(json) as string[];
    if (Array.isArray(arr) && arr.length > 0) return arr.join(", ");
  } catch { /* ignore */ }
  return json;
}

export function hasAddresses(json: string): boolean {
  try {
    const arr = JSON.parse(json) as string[];
    return Array.isArray(arr) && arr.length > 0;
  } catch { return false; }
}
