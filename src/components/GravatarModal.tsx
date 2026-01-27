import { useState, useEffect } from "react";
import md5 from "md5";
import { getGravatarUrl, getAvatarColor, getInitials } from "../lib/utils";

interface GravatarModalProps {
  email: string | null;
  name?: string;
  isOpen: boolean;
  onClose: () => void;
}

export function GravatarModal({ email, name, isOpen, onClose }: GravatarModalProps) {
  const [hasGravatar, setHasGravatar] = useState<boolean | null>(null);

  useEffect(() => {
    if (!email) return;

    setHasGravatar(null);
    const img = new Image();
    img.onload = () => setHasGravatar(true);
    img.onerror = () => setHasGravatar(false);
    img.src = getGravatarUrl(email, 200);
  }, [email]);

  if (!isOpen || !email) return null;

  const hash = md5(email.trim().toLowerCase());
  const cardUrl = `https://gravatar.com/${hash}.card`;
  const displayName = name || email;

  return (
    <div className="gravatar-panel">
      <div className="gravatar-panel-header">
        <div className="gravatar-panel-info">
          <h2 className="gravatar-panel-title">{displayName}</h2>
          {name && <span className="gravatar-panel-email">{email}</span>}
        </div>
        <button className="gravatar-panel-close" onClick={onClose} title="Close">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="gravatar-panel-content">
        {hasGravatar === null ? (
          <div className="gravatar-loading">
            <div className="loading-spinner" />
          </div>
        ) : hasGravatar ? (
          <iframe
            src={cardUrl}
            title="Gravatar Profile"
          />
        ) : (
          <div className="gravatar-fallback">
            <div
              className="gravatar-fallback-avatar"
              style={{ backgroundColor: getAvatarColor(email) }}
            >
              {getInitials(displayName)}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
