import { useState, useEffect } from "react";
import md5 from "md5";
import { getGravatarUrl, getAvatarColor, getInitials } from "../../../shared";

interface GravatarModalProps {
  email: string | null;
  name?: string;
  isOpen: boolean;
  onClose: () => void;
}

export function GravatarModal({
  email,
  name,
  isOpen,
  onClose,
}: GravatarModalProps) {
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
    <div className="flex-1 flex flex-col bg-bg-secondary">
      <div className="flex items-center justify-between p-4 border-b border-divider">
        <div className="flex-1 min-w-0">
          <h2 className="text-lg font-semibold text-text-primary truncate">
            {displayName}
          </h2>
          {name && <span className="text-sm text-text-muted">{email}</span>}
        </div>
        <button
          className="w-8 h-8 rounded-full flex items-center justify-center hover:bg-bg-hover transition-colors"
          onClick={onClose}
          title="Close"
        >
          <svg
            className="w-5 h-5 text-text-muted"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="flex-1 flex items-center justify-center p-6">
        {hasGravatar === null ? (
          <div className="spinner spinner-lg" />
        ) : hasGravatar ? (
          <iframe
            src={cardUrl}
            title="Gravatar Profile"
            className="w-full h-full min-h-[300px] border-none rounded-lg bg-white"
          />
        ) : (
          <div className="flex flex-col items-center gap-4">
            <div
              className="w-32 h-32 rounded-full flex items-center justify-center text-4xl font-bold text-white"
              style={{ backgroundColor: getAvatarColor(email) }}
            >
              {getInitials(displayName)}
            </div>
            <p className="text-text-muted text-sm">No Gravatar profile</p>
          </div>
        )}
      </div>
    </div>
  );
}
