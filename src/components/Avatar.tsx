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
  const avatarColor = getAvatarColor(name);
  const initials = getInitials(name);

  // Reset status when email changes
  useEffect(() => {
    setImageStatus('loading');
  }, [email]);

  return (
    <div
      className={`avatar-container ${className}`}
      style={{ backgroundColor: avatarColor }}
      title={title}
    >
      {gravatarUrl && imageStatus !== 'error' && (
        <img
          src={gravatarUrl}
          alt={name}
          className="chat-avatar-img"
          style={{ display: imageStatus === 'loaded' ? 'block' : 'none' }}
          onLoad={() => setImageStatus('loaded')}
          onError={() => setImageStatus('error')}
        />
      )}
      {(imageStatus !== 'loaded' || !gravatarUrl) && (
        <span className="chat-avatar-initials">{initials}</span>
      )}
    </div>
  );
}
