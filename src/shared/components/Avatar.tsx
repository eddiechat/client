import { useState, useEffect } from "react";
import { getAvatarColor, getInitials, getGravatarUrl } from "../lib/utils";

interface AvatarProps {
  email: string | null;
  name: string;
  size?: number;
  className?: string;
  title?: string;
  onClick?: () => void;
}

export function Avatar({
  email,
  name,
  size = 40,
  className = "",
  title,
  onClick,
}: AvatarProps) {
  const [imageStatus, setImageStatus] = useState<"loading" | "loaded" | "error">(
    "loading"
  );
  const gravatarUrl = email ? getGravatarUrl(email, size) : null;
  const avatarColor = getAvatarColor(email || name);
  const initials = getInitials(name);

  useEffect(() => {
    setImageStatus("loading");
  }, [email]);

  const sizeClass = size <= 32 ? "text-xs" : size <= 40 ? "text-sm" : "text-lg";

  const baseClasses = `flex items-center justify-center rounded-full text-white font-semibold uppercase overflow-hidden relative ${sizeClass}`;
  const interactiveClasses = onClick ? "cursor-pointer" : "";

  return (
    <div
      className={`${baseClasses} ${interactiveClasses} ${className}`}
      style={{ backgroundColor: avatarColor, width: size, height: size, minWidth: size }}
      title={title}
      onClick={onClick}
      role={onClick ? "button" : undefined}
      tabIndex={onClick ? 0 : undefined}
    >
      {gravatarUrl && (
        <img
          src={gravatarUrl}
          alt={name}
          className="absolute inset-0 w-full h-full object-cover rounded-full"
          onError={(e) => {
            e.currentTarget.style.display = "none";
            setImageStatus("error");
          }}
          onLoad={() => setImageStatus("loaded")}
        />
      )}
      <span
        className="avatar-initials"
        style={{ display: imageStatus === "loaded" ? "none" : "block" }}
      >
        {initials}
      </span>
    </div>
  );
}
