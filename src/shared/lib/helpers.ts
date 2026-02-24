import type { Conversation, Message } from "../../tauri";

export function hash(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = ((h << 5) - h + s.charCodeAt(i)) | 0;
  return Math.abs(h);
}

const AVATAR_PALETTE = [
  "#6EC6A5", "#F2A365", "#7CAED9", "#D98EC0",
  "#B5A4E0", "#E8C96A", "#7BC7C7", "#E88E8E",
];

const DARK_AVATAR_PALETTE = [
  "#e91e63", "#9c27b0", "#673ab7", "#3f51b5",
  "#2196f3", "#03a9f4", "#00bcd4", "#009688",
  "#4caf50", "#8bc34a", "#ff9800", "#ff5722",
];

function charCodeSum(name: string): number {
  return name.split("").reduce((a, c) => a + c.charCodeAt(0), 0);
}

function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

export function avatarBg(name: string): string {
  const palette = isDark() ? DARK_AVATAR_PALETTE : AVATAR_PALETTE;
  return palette[charCodeSum(name) % palette.length];
}

export function avatarBorder(name: string): string {
  return avatarBg(name);
}

export function avatarTextColor(_name: string): string {
  return "#fff";
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
      if (v.length) return v.join(", ");
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
  "#6EC6A5", "#F2A365", "#7CAED9", "#D98EC0",
  "#B5A4E0", "#E8C96A", "#7BC7C7", "#E88E8E",
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
