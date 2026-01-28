import { useState, useEffect } from "react";
import { getAvatarColor, getInitials, getGravatarUrl } from "../lib/utils";

interface AvatarProps {
  email: string | null;
  name: string;
  size?: number;
  className?: string;
  title?: string;
}

export function Avatar({ email, name, size = 40, className = "", title }: AvatarProps) {
  const [imageStatus, setImageStatus] = useState<'loading' | 'loaded' | 'error'>('loading');
  const gravatarUrl = email ? getGravatarUrl(email, size) : null;
  const avatarColor = getAvatarColor(email || name);
  const initials = getInitials(name);

  useEffect(() => {
    setImageStatus('loading');
  }, [email]);

  const sizeClass = size <= 32 ? "text-xs" : size <= 40 ? "text-sm" : "text-lg";

  return (
    <div
      className={`flex items-center justify-center rounded-full text-white font-semibold uppercase overflow-hidden relative ${sizeClass} ${className}`}
      style={{ backgroundColor: avatarColor, width: size, height: size, minWidth: size }}
      title={title}
    >
      {gravatarUrl && imageStatus !== 'error' && (
        <img
          src={gravatarUrl}
          alt={name}
          className="absolute inset-0 w-full h-full object-cover rounded-full"
          style={{ display: imageStatus === 'loaded' ? 'block' : 'none' }}
          onLoad={() => setImageStatus('loaded')}
          onError={() => setImageStatus('error')}
        />
      )}
      {(imageStatus !== 'loaded' || !gravatarUrl) && (
        <span className="avatar-initials">{initials}</span>
      )}
    </div>
  );
}
