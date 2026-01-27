import md5 from "md5";

interface GravatarModalProps {
  email: string | null;
  name?: string;
  isOpen: boolean;
  onClose: () => void;
}

export function GravatarModal({ email, name, isOpen, onClose }: GravatarModalProps) {
  if (!isOpen || !email) return null;

  const hash = md5(email.trim().toLowerCase());
  const cardUrl = `https://gravatar.com/${hash}.card`;

  return (
    <div className="gravatar-panel">
      <div className="gravatar-panel-header">
        <div className="gravatar-panel-info">
          <h2 className="gravatar-panel-title">{name || email}</h2>
          {name && <span className="gravatar-panel-email">{email}</span>}
        </div>
        <button className="gravatar-panel-close" onClick={onClose} title="Close">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="gravatar-panel-content">
        <iframe
          src={cardUrl}
          title="Gravatar Profile"
        />
      </div>
    </div>
  );
}
