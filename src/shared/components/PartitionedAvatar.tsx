import type { CSSProperties } from "react";
import { avatarGroupPalette, storeConversationColors, initials, textColorForBg } from "../lib/helpers";

interface PartitionedAvatarProps {
  /** [email, name] pairs for the group participants */
  participants: [string, string][];
  /** Box size in pixels. Default 44 (w-11 h-11). */
  sizePx?: number;
  /** If provided, stores the picked palette keyed by this ID for later retrieval. */
  conversationId?: string;
}

function charSum(s: string): number {
  return s.split("").reduce((a, c) => a + c.charCodeAt(0), 0);
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
  fontWeight: 800,
  letterSpacing: "-0.5px",
};

export function PartitionedAvatar({ participants, sizePx = 54, conversationId }: PartitionedAvatarProps) {
  const isDark = document.documentElement.classList.contains("dark");
  const count = participants.length;
  const gHash = participants.reduce((acc, [email, name]) => acc + charSum(name || email), 0);
  const palette = avatarGroupPalette(gHash);

  if (conversationId) storeConversationColors(conversationId, palette, participants);

  /** Assign a color from the group palette based on participant index. */
  function colorAt(index: number): string {
    return palette[index % palette.length];
  }

  const S = sizePx;
  const G = 1.5;           // gap between cells
  const H = (S - G) / 2;  // half size (accounts for gap)

  let slots: SlotDef[] = [];

  if (count === 1) {
    const label = participants[0][1] || participants[0][0];
    slots = [{ label, color: colorAt(0), pos: { top: 0, left: 0, right: 0, bottom: 0 } }];

  } else if (count === 2) {
    const l0 = participants[0][1] || participants[0][0];
    const l1 = participants[1][1] || participants[1][0];
    const c0 = colorAt(0);
    const c1 = colorAt(1);
    if (gHash % 2 === 0) {
      slots = [
        { label: l0, color: c0, pos: { top: 0, left: 0, right: 0, height: H } },
        { label: l1, color: c1, pos: { bottom: 0, left: 0, right: 0, height: H } },
      ];
    } else {
      slots = [
        { label: l0, color: c0, pos: { top: 0, left: 0, bottom: 0, width: H } },
        { label: l1, color: c1, pos: { top: 0, right: 0, bottom: 0, width: H } },
      ];
    }

  } else if (count === 3) {
    const parts = participants.slice(0, 3).map(([email, name], i) => {
      const label = name || email;
      return { label, color: colorAt(i) };
    });
    const layout = gHash % 4;
    if (layout === 0) {
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, bottom: 0, width: H } },
        { label: parts[1].label, color: parts[1].color, pos: { top: 0, right: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { bottom: 0, right: 0, width: H, height: H } },
      ];
    } else if (layout === 1) {
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, width: H, height: H } },
        { label: parts[1].label, color: parts[1].color, pos: { bottom: 0, left: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { top: 0, right: 0, bottom: 0, width: H } },
      ];
    } else if (layout === 2) {
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, right: 0, height: H } },
        { label: parts[1].label, color: parts[1].color, pos: { bottom: 0, left: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { bottom: 0, right: 0, width: H, height: H } },
      ];
    } else {
      slots = [
        { label: parts[0].label, color: parts[0].color, pos: { top: 0, left: 0, width: H, height: H } },
        { label: parts[1].label, color: parts[1].color, pos: { top: 0, right: 0, width: H, height: H } },
        { label: parts[2].label, color: parts[2].color, pos: { bottom: 0, left: 0, right: 0, height: H } },
      ];
    }

  } else {
    const four = participants.slice(0, 4).map(([email, name], i) => {
      const label = name || email;
      return { label, color: colorAt(i) };
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
            color: textColorForBg(slot.color),
            fontSize: slot.label === "*" ? "20px" : "12px",
            ...slot.pos,
          }}
        >
          {slot.label === "*" ? "*" : initials(slot.label)[0]}
        </div>
      ))}
    </div>
  );
}
