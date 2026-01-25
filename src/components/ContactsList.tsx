import { useState } from "react";
import type { Contact } from "../types";
import { getAvatarColor, getInitials, getGravatarUrl } from "../lib/utils";

interface ContactsListProps {
  contacts: Contact[];
  loading: boolean;
  error: string | null;
  hasCardDAV: boolean;
  onSelectContact?: (contact: Contact) => void;
  onStartEmail?: (email: string) => void;
  onRefresh: () => void;
}

export function ContactsList({
  contacts,
  loading,
  error,
  hasCardDAV,
  onSelectContact,
  onStartEmail,
  onRefresh,
}: ContactsListProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Filter contacts by search query
  const filteredContacts = searchQuery
    ? contacts.filter((contact) => {
        const searchLower = searchQuery.toLowerCase();
        return (
          contact.full_name.toLowerCase().includes(searchLower) ||
          contact.emails.some((e) => e.email.toLowerCase().includes(searchLower)) ||
          contact.phones.some((p) => p.number.includes(searchQuery)) ||
          contact.organization?.toLowerCase().includes(searchLower)
        );
      })
    : contacts;

  // Group contacts by first letter
  const groupedContacts = filteredContacts.reduce((acc, contact) => {
    const firstLetter = contact.full_name.charAt(0).toUpperCase() || "#";
    if (!acc[firstLetter]) {
      acc[firstLetter] = [];
    }
    acc[firstLetter].push(contact);
    return acc;
  }, {} as Record<string, Contact[]>);

  const sortedLetters = Object.keys(groupedContacts).sort((a, b) => {
    if (a === "#") return 1;
    if (b === "#") return -1;
    return a.localeCompare(b);
  });

  const handleContactClick = (contact: Contact) => {
    setSelectedId(contact.id);
    onSelectContact?.(contact);
  };

  const handleEmailClick = (e: React.MouseEvent, email: string) => {
    e.stopPropagation();
    onStartEmail?.(email);
  };

  if (!hasCardDAV) {
    return (
      <div className="contacts-list">
        <div className="contacts-list-header">
          <h2>Contacts</h2>
        </div>
        <div className="contacts-list-empty">
          <div className="contacts-not-configured">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="contacts-empty-icon">
              <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
              <circle cx="9" cy="7" r="4" />
              <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
              <path d="M16 3.13a4 4 0 0 1 0 7.75" />
            </svg>
            <p>CardDAV not configured</p>
            <p className="contacts-hint">
              Add CardDAV settings to your account configuration to sync contacts.
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="contacts-list">
      <div className="contacts-list-header">
        <h2>Contacts</h2>
        <button className="contacts-refresh-btn" onClick={onRefresh} title="Refresh contacts">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M23 4v6h-6" />
            <path d="M1 20v-6h6" />
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
          </svg>
        </button>
      </div>

      <div className="search-container">
        <div className="search-wrapper">
          <svg className="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.35-4.35" />
          </svg>
          <input
            type="text"
            className="search-input"
            placeholder="Search contacts"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
      </div>

      <div className="contacts-list-content">
        {loading ? (
          <div className="contacts-list-loading">
            <div className="loading-spinner" />
            <span>Loading contacts...</span>
          </div>
        ) : error ? (
          <div className="contacts-list-error">
            <span>Failed to load contacts</span>
            <p className="error-message">{error}</p>
            <button className="retry-btn" onClick={onRefresh}>
              Retry
            </button>
          </div>
        ) : filteredContacts.length === 0 ? (
          <div className="contacts-list-empty">
            {searchQuery ? "No contacts found" : "No contacts yet"}
          </div>
        ) : (
          sortedLetters.map((letter) => (
            <div key={letter} className="contacts-group">
              <div className="contacts-group-header">{letter}</div>
              {groupedContacts[letter].map((contact) => {
                const isSelected = selectedId === contact.id;
                const primaryEmail = contact.emails.find((e) => e.primary) || contact.emails[0];
                const avatarColor = getAvatarColor(primaryEmail?.email || contact.full_name);
                const initials = getInitials(contact.full_name);
                const gravatarUrl = primaryEmail ? getGravatarUrl(primaryEmail.email, 48) : null;

                return (
                  <div
                    key={contact.id}
                    className={`contact-item ${isSelected ? "selected" : ""}`}
                    onClick={() => handleContactClick(contact)}
                  >
                    <div className="contact-avatar" style={{ backgroundColor: avatarColor }}>
                      {gravatarUrl && (
                        <img
                          src={gravatarUrl}
                          alt={contact.full_name}
                          className="contact-avatar-img"
                          onError={(e) => {
                            e.currentTarget.style.display = "none";
                          }}
                        />
                      )}
                      <span className="contact-avatar-initials">{initials}</span>
                    </div>

                    <div className="contact-content">
                      <div className="contact-name">{contact.full_name}</div>
                      {contact.organization && (
                        <div className="contact-organization">{contact.organization}</div>
                      )}
                      {primaryEmail && (
                        <div
                          className="contact-email"
                          onClick={(e) => handleEmailClick(e, primaryEmail.email)}
                        >
                          {primaryEmail.email}
                        </div>
                      )}
                    </div>

                    {primaryEmail && onStartEmail && (
                      <button
                        className="contact-email-btn"
                        onClick={(e) => handleEmailClick(e, primaryEmail.email)}
                        title="Send email"
                      >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" />
                          <polyline points="22,6 12,13 2,6" />
                        </svg>
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

// Contact detail view component
interface ContactDetailProps {
  contact: Contact;
  onClose: () => void;
  onStartEmail?: (email: string) => void;
}

export function ContactDetail({ contact, onClose, onStartEmail }: ContactDetailProps) {
  const primaryEmail = contact.emails.find((e) => e.primary) || contact.emails[0];
  const avatarColor = getAvatarColor(primaryEmail?.email || contact.full_name);
  const initials = getInitials(contact.full_name);
  const gravatarUrl = primaryEmail ? getGravatarUrl(primaryEmail.email, 120) : null;

  return (
    <div className="contact-detail">
      <div className="contact-detail-header">
        <button className="contact-detail-close" onClick={onClose}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
        <h2>Contact</h2>
      </div>

      <div className="contact-detail-content">
        <div className="contact-detail-avatar" style={{ backgroundColor: avatarColor }}>
          {gravatarUrl && (
            <img
              src={gravatarUrl}
              alt={contact.full_name}
              className="contact-detail-avatar-img"
              onError={(e) => {
                e.currentTarget.style.display = "none";
              }}
            />
          )}
          <span className="contact-detail-avatar-initials">{initials}</span>
        </div>

        <h3 className="contact-detail-name">{contact.full_name}</h3>

        {contact.title && contact.organization && (
          <p className="contact-detail-title">
            {contact.title} at {contact.organization}
          </p>
        )}
        {contact.title && !contact.organization && (
          <p className="contact-detail-title">{contact.title}</p>
        )}
        {!contact.title && contact.organization && (
          <p className="contact-detail-title">{contact.organization}</p>
        )}

        {contact.emails.length > 0 && (
          <div className="contact-detail-section">
            <h4>Email</h4>
            {contact.emails.map((email, idx) => (
              <div key={idx} className="contact-detail-row">
                <span className="contact-detail-label">
                  {email.type || "email"}
                  {email.primary && " (primary)"}
                </span>
                <a
                  href={`mailto:${email.email}`}
                  className="contact-detail-value contact-detail-link"
                  onClick={(e) => {
                    if (onStartEmail) {
                      e.preventDefault();
                      onStartEmail(email.email);
                    }
                  }}
                >
                  {email.email}
                </a>
              </div>
            ))}
          </div>
        )}

        {contact.phones.length > 0 && (
          <div className="contact-detail-section">
            <h4>Phone</h4>
            {contact.phones.map((phone, idx) => (
              <div key={idx} className="contact-detail-row">
                <span className="contact-detail-label">
                  {phone.type || "phone"}
                  {phone.primary && " (primary)"}
                </span>
                <a href={`tel:${phone.number}`} className="contact-detail-value contact-detail-link">
                  {phone.number}
                </a>
              </div>
            ))}
          </div>
        )}

        {contact.addresses.length > 0 && (
          <div className="contact-detail-section">
            <h4>Address</h4>
            {contact.addresses.map((addr, idx) => (
              <div key={idx} className="contact-detail-row">
                <span className="contact-detail-label">
                  {addr.type || "address"}
                  {addr.primary && " (primary)"}
                </span>
                <span className="contact-detail-value contact-detail-address">
                  {[addr.street, addr.city, addr.state, addr.postal_code, addr.country]
                    .filter(Boolean)
                    .join(", ")}
                </span>
              </div>
            ))}
          </div>
        )}

        {contact.birthday && (
          <div className="contact-detail-section">
            <h4>Birthday</h4>
            <div className="contact-detail-row">
              <span className="contact-detail-value">{contact.birthday}</span>
            </div>
          </div>
        )}

        {contact.notes && (
          <div className="contact-detail-section">
            <h4>Notes</h4>
            <p className="contact-detail-notes">{contact.notes}</p>
          </div>
        )}
      </div>
    </div>
  );
}
