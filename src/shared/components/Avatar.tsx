import { useGravatar } from "../lib/gravatar";
import { avatarBg, avatarTextColor, initials } from "../lib/helpers";

interface AvatarProps {
  name: string;
  email?: string;
  /** Tailwind spacing unit (7 = w-7 h-7 = 28px, 11 = w-11 h-11 = 44px) */
  size: number;
  /** Font size class for initials, e.g. "text-[15px]" */
  fontSize?: string;
  /** Extra className for the outer container */
  className?: string;
}

const SIZE_PX: Record<number, number> = {
  7: 28,
  8: 32,
  9: 36,
  10: 40,
  11: 44,
  12: 48,
};

export function Avatar({
  name,
  email,
  size,
  fontSize = "text-[15px]",
  className = "",
}: AvatarProps) {
  const px = SIZE_PX[size] ?? size * 4;
  const gravatarSrc = useGravatar(email, px * 2);

  return (
    <div
      className={`w-${size} h-${size} avatar-shape flex items-center justify-center font-bold ${fontSize} ${className}`}
      style={{ background: avatarBg(name), color: avatarTextColor(name) }}
    >
      {gravatarSrc ? (
        <img
          src={gravatarSrc}
          alt=""
          className={`w-${size} h-${size} avatar-shape object-cover`}
        />
      ) : (
        initials(name)
      )}
    </div>
  );
}
