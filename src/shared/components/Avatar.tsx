import { useState, useEffect, useMemo, useRef } from "react";
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

  // Memoize expensive calculations to avoid recalculating on every render
  const gravatarUrl = useMemo(
    () => {
      if (email) {
        const url = getGravatarUrl(email, size);
        // Debug logging
        if (import.meta.env.DEV) {
          console.log('[Avatar] email:', email, 'name:', name, 'gravatarUrl:', url);
        }
        return url;
      }
      return null;
    },
    [email, size]
  );
  const avatarColor = useMemo(
    () => getAvatarColor(email || name),
    [email, name]
  );
  const initials = useMemo(() => getInitials(name), [name]);

  // Reset image loading state when gravatar URL changes
  // Initialize with current URL to prevent unnecessary reset on first mount
  const prevUrlRef = useRef<string | null>(gravatarUrl);
  useEffect(() => {
    if (prevUrlRef.current !== gravatarUrl) {
      setImageStatus("loading");
      prevUrlRef.current = gravatarUrl;
    }
  }, [gravatarUrl]);

  const sizeClass = size <= 32 ? "text-xs" : size <= 40 ? "text-sm" : "text-lg";

  const baseClasses = `flex items-center justify-center rounded-full text-white font-semibold uppercase overflow-hidden relative ${sizeClass}`;
  const interactiveClasses = onClick ? "cursor-pointer" : "";

  return (
    <div
      className={`${baseClasses} ${interactiveClasses} ${className}`}
      style={{
        backgroundColor: imageStatus === "loaded" ? "transparent" : avatarColor,
        width: size,
        height: size,
        minWidth: size,
      }}
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
          style={{ display: imageStatus === "loaded" ? "block" : "none" }}
          onError={() => {
            if (import.meta.env.DEV) {
              console.log('[Avatar] Image failed to load:', email, gravatarUrl);
            }
            setImageStatus("error");
          }}
          onLoad={() => {
            if (import.meta.env.DEV) {
              console.log('[Avatar] Image loaded successfully:', email, gravatarUrl);
            }
            setImageStatus("loaded");
          }}
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
