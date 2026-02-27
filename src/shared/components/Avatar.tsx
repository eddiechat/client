import { useGravatar } from "../lib/gravatar";
import { avatarBg, avatarTextColor, textColorForBg, initials } from "../lib/helpers";

interface AvatarProps {
  name: string;
  email?: string;
  /** Tailwind spacing unit (7 = w-7 h-7 = 28px, 11 = w-11 h-11 = 44px) */
  size: number;
  /** Font size class for initials, e.g. "text-[15px]" */
  fontSize?: string;
  /** Extra className for the outer container */
  className?: string;
  /** Override background color (e.g. from a stored palette) */
  color?: string;
}

const SIZE_PX: Record<number, number> = {
  7: 28,
  8: 32,
  9: 36,
  10: 40,
  11: 44,
  12: 48,
  13: 52,
};

export function Avatar({
  name,
  email,
  size,
  fontSize = "text-[15px]",
  className = "",
  color,
}: AvatarProps) {
  const px = SIZE_PX[size] ?? size * 4;
  const gravatarSrc = useGravatar(email, px * 2);
  const bg = color ?? avatarBg(name);
  const fg = color ? textColorForBg(color) : avatarTextColor(name);

  return (
    <div
      className={`avatar-shape flex items-center justify-center font-extrabold ${fontSize} ${className}`}
      style={{ width: px, height: px, flexShrink: 0, background: bg, color: fg, letterSpacing: "-0.5px", position: "relative" }}
    >
      {initials(name)}
      {gravatarSrc && (
        <img
          src={gravatarSrc}
          alt=""
          style={{ width: px, height: px, position: "absolute", top: 0, left: 0 }}
          className="avatar-shape object-cover"
        />
      )}
    </div>
  );
}
