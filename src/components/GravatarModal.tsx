import md5 from "md5";

interface GravatarModalProps {
  email: string | null;
  isOpen: boolean;
  onClose: () => void;
}

export function GravatarModal({ email, isOpen, onClose }: GravatarModalProps) {
  if (!isOpen || !email) return null;

  const hash = md5(email.trim().toLowerCase());
  const cardUrl = `https://gravatar.com/${hash}.card`;

  const handleOverlayClick = (e: React.MouseEvent) => {
    // Close only when clicking the overlay, not the modal content
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  return (
    <div className="gravatar-modal-overlay" onClick={handleOverlayClick}>
      <div className="gravatar-modal">
        <button className="gravatar-modal-close" onClick={onClose} title="Close">
          &times;
        </button>
        <iframe
          src={cardUrl}
          width="415"
          height="228"
          style={{ border: 0, margin: 0, padding: 0 }}
          title="Gravatar Profile"
        />
      </div>
    </div>
  );
}
