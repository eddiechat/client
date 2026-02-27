interface LogoPillProps {
  height?: number;
}

// viewBox: 0 0 100 100 (square)
// Gap G=8 used for both the vertical divider and horizontal divider.
// Blue left:     x=0..44,  y=0..100  — TL+BL corners rounded (r=13)
// Yellow top:    x=52..100, y=0..46  — TR corner rounded (r=11)
// Yellow bottom: x=52..100, y=54..100 — BR corner rounded (r=11)

const BLUE = "#5BBCF5";
const YELLOW = "#F5C43A";

export function LogoPill({ height = 44 }: LogoPillProps) {
  return (
    <svg
      width={height}
      height={height}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={{ flexShrink: 0 }}
    >
      {/* Blue left piece — TL and BL rounded (r=13), right side sharp */}
      <path
        fill={BLUE}
        d="M 13 0 L 44 0 L 44 100 L 13 100 A 13 13 0 0 1 0 87 L 0 13 A 13 13 0 0 1 13 0 Z"
      />
      {/* Yellow top piece — TR rounded (r=11), all other corners sharp */}
      <path
        fill={YELLOW}
        d="M 52 0 L 89 0 A 11 11 0 0 1 100 11 L 100 46 L 52 46 Z"
      />
      {/* Yellow bottom piece — BR rounded (r=11), all other corners sharp */}
      <path
        fill={YELLOW}
        d="M 52 54 L 100 54 L 100 89 A 11 11 0 0 1 89 100 L 52 100 Z"
      />
    </svg>
  );
}
