import type { CSSProperties } from "react";
import { initials } from "../lib/helpers";

interface PartitionedAvatarProps {
  /** [email, name] pairs for the group participants */
  participants: [string, string][];
  /** Box size in pixels. Default 44 (w-11 h-11). */
  sizePx?: number;
}

const LIGHT_PALETTE = [
  "#FF5A5F", "#4A90E2", "#43B89C", "#9B72CF",
  "#FF9F1C", "#2EC4B6", "#FF6584", "#6D28D9",
];

const DARK_PALETTE = [
  "#e91e63", "#9c27b0", "#673ab7", "#3f51b5",
  "#2196f3", "#03a9f4", "#00bcd4", "#009688",
  "#4caf50", "#8bc34a", "#ff9800", "#ff5722",
];

// Palette index groups for 2-person "same color zone" pairing
const LIGHT_ZONES = [
  [0, 6, 4],  // warm: red, pink, amber
  [1, 2, 5],  // cool: blue, teal, cyan
  [3, 7],     // deep: purple, deep-purple
];
const DARK_ZONES = [
  [0, 10, 11],   // warm: pink, orange, deep-orange
  [4, 5, 6, 7],  // cool: blue, light-blue, cyan, teal
  [1, 2, 3],     // deep: purple, deep-purple, indigo
];

function charSum(s: string): number {
  return s.split("").reduce((a, c) => a + c.charCodeAt(0), 0);
}

function flatColor(name: string, isDark: boolean): string {
  const palette = isDark ? DARK_PALETTE : LIGHT_PALETTE;
  return palette[charSum(name) % palette.length];
}

function pairColors(n0: string, n1: string, gHash: number, isDark: boolean): [string, string] {
  const zones = isDark ? DARK_ZONES : LIGHT_ZONES;
  const palette = isDark ? DARK_PALETTE : LIGHT_PALETTE;
  const zone = zones[gHash % zones.length];
  const i0 = charSum(n0) % zone.length;
  let i1 = charSum(n1) % zone.length;
  if (i1 === i0) i1 = (i0 + 1) % zone.length;
  return [palette[zone[i0]], palette[zone[i1]]];
}

interface SlotDef {
  label: string;
  color: string;
  pos: CSSProperties; // absolute position + size
}

const CELL_BASE: CSSProperties = {
  position: "absolute",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: "#fff",
  fontWeight: 800,
  letterSpacing: "-0.5px",
};

export function PartitionedAvatar({ participants, sizePx = 44 }: PartitionedAvatarProps) {
  const isDark = document.documentElement.classList.contains("dark");
  const count = participants.length;
  const gHash = participants.reduce((acc, [email, name]) => acc + charSum(name || email), 0);

  const S = sizePx;
  const G = 1.5;           // gap between cells
  const H = (S - G) / 2;  // half size (accounts for gap)

  let slots: SlotDef[] = [];

  if (count === 1) {
    const label = participants[0][1] || participants[0][0];
    slots = [{ label, color: flatColor(label, isDark), pos: { top: 0, left: 0, right: 0, bottom: 0 } }];

  } else if (count === 2) {
    const l0 = participants[0][1] || participants[0][0];
    const l1 = participants[1][1] || participants[1][0];
    const [c0, c1] = pairColors(l0, l1, gHash, isDark);
    if (gHash % 2 === 0) {
      // Horizontal: top/bottom halves
      slots = [
        { label: l0, color: c0, pos: { top: 0, left: 0, right: 0, height: H } },
        { label: l1, color: c1, pos: { bottom: 0, left: 0, right: 0, height: H } },
      ];
    } else {
      // Vertical: left/right halves
      slots = [
        { label: l0, color: c0, pos: { top: 0, left: 0, bottom: 0, width: H } },
        { label: l1, color: c1, pos: { top: 0, right: 0, bottom: 0, width: H } },
      ];
    }

  } else if (count === 3) {
    const parts = participants.slice(0, 3).map(([email, name]) => {
      const label = name || email;
      return { label, color: flatColor(label, isDark) };
    });
    const layout = gHash % 4;
    if (layout === 0) {
      // Big piece left
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, bottom: 0, width: H } },
        { label: parts[1].label, color: parts[1].color, pos: { top: 0, right: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { bottom: 0, right: 0, width: H, height: H } },
      ];
    } else if (layout === 1) {
      // Big piece right
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, width: H, height: H } },
        { label: parts[1].label, color: parts[1].color, pos: { bottom: 0, left: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { top: 0, right: 0, bottom: 0, width: H } },
      ];
    } else if (layout === 2) {
      // Big piece top
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, right: 0, height: H } },
        { label: parts[1].label, color: parts[1].color, pos: { bottom: 0, left: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { bottom: 0, right: 0, width: H, height: H } },
      ];
    } else {
      // Big piece bottom
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, width: H, height: H } },
        { label: parts[1].label, color: parts[1].color, pos: { top: 0, right: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { bottom: 0, left: 0, right: 0, height: H } },
      ];
    }

  } else {
    // 4+ persons: 2×2 corners, optionally replace one with *
    const four = participants.slice(0, 4).map(([email, name]) => {
      const label = name || email;
      return { label, color: flatColor(label, isDark) };
    });
    if (count > 4) {
      const idx = gHash % 4;
      four[idx] = { label: "*", color: four[idx].color };
    }
    slots = [
      { label: four[0].label, color: four[0].color, pos: { top: 0, left: 0, width: H, height: H } },
      { label: four[1].label, color: four[1].color, pos: { top: 0, right: 0, width: H, height: H } },
      { label: four[2].label, color: four[2].color, pos: { bottom: 0, left: 0, width: H, height: H } },
      { label: four[3].label, color: four[3].color, pos: { bottom: 0, right: 0, width: H, height: H } },
    ];
  }

  return (
    <div
      style={{
        position: "relative",
        width: S,
        height: S,
        borderRadius: isDark ? "9999px" : "11px",
        overflow: "hidden",
        flexShrink: 0,
        background: "var(--color-bg-secondary)",
      }}
    >
      {slots.map((slot, i) => (
        <div
          key={i}
          style={{
            ...CELL_BASE,
            background: slot.color,
            fontSize: slot.label === "*" ? "16px" : "10px",
            ...slot.pos,
          }}
        >
          {slot.label === "*" ? "*" : initials(slot.label)[0]}
        </div>
      ))}
    </div>
  );
}
