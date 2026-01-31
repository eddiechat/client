use serde::{Deserialize, Serialize};

/// Represents a contact from CardDAV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// Unique identifier (usually the vCard UID)
    pub id: String,

    /// Full name (FN property in vCard)
    pub full_name: String,

    /// Given name (first name)
    pub given_name: Option<String>,

    /// Family name (last name)
    pub family_name: Option<String>,

    /// Nickname
    pub nickname: Option<String>,

    /// Email addresses
    pub emails: Vec<ContactEmail>,

    /// Phone numbers
    pub phones: Vec<ContactPhone>,

    /// Physical addresses
    pub addresses: Vec<ContactAddress>,

    /// Organization name
    pub organization: Option<String>,

    /// Job title
    pub title: Option<String>,

    /// Birthday (ISO date string)
    pub birthday: Option<String>,

    /// Notes/comments
    pub notes: Option<String>,

    /// URL to photo (if any)
    pub photo_url: Option<String>,

    /// The raw vCard data (for editing/updating)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_vcard: Option<String>,

    /// ETag for optimistic concurrency control
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,

    /// The href/path on the CardDAV server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

/// Contact email with type label
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactEmail {
    /// The email address
    pub email: String,
    /// Type (e.g., "work", "home", "other")
    #[serde(rename = "type")]
    pub email_type: Option<String>,
    /// Whether this is the primary/preferred email
    #[serde(default)]
    pub primary: bool,
}

/// Contact phone number with type label
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactPhone {
    /// The phone number
    pub number: String,
    /// Type (e.g., "work", "home", "mobile", "fax")
    #[serde(rename = "type")]
    pub phone_type: Option<String>,
    /// Whether this is the primary/preferred phone
    #[serde(default)]
    pub primary: bool,
}

/// Physical address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactAddress {
    /// Type (e.g., "work", "home")
    #[serde(rename = "type")]
    pub address_type: Option<String>,
    /// Street address
    pub street: Option<String>,
    /// City
    pub city: Option<String>,
    /// State/Province
    pub state: Option<String>,
    /// Postal code
    pub postal_code: Option<String>,
    /// Country
    pub country: Option<String>,
    /// Whether this is the primary/preferred address
    #[serde(default)]
    pub primary: bool,
}

/// CardDAV address book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBook {
    /// Display name of the address book
    pub name: String,
    /// Path/href on the server
    pub href: String,
    /// Description (if any)
    pub description: Option<String>,
    /// Number of contacts (if known)
    pub contact_count: Option<usize>,
}

/// Request to create or update a contact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveContactRequest {
    /// Account name (optional, uses default if not provided)
    pub account: Option<String>,
    /// The contact data
    pub contact: Contact,
}

impl Contact {
    /// Create a new empty contact with a generated ID
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            full_name: String::new(),
            given_name: None,
            family_name: None,
            nickname: None,
            emails: Vec::new(),
            phones: Vec::new(),
            addresses: Vec::new(),
            organization: None,
            title: None,
            birthday: None,
            notes: None,
            photo_url: None,
            raw_vcard: None,
            etag: None,
            href: None,
        }
    }

    /// Get the primary email address
    pub fn primary_email(&self) -> Option<&str> {
        self.emails
            .iter()
            .find(|e| e.primary)
            .or_else(|| self.emails.first())
            .map(|e| e.email.as_str())
    }

    /// Get the primary phone number
    pub fn primary_phone(&self) -> Option<&str> {
        self.phones
            .iter()
            .find(|p| p.primary)
            .or_else(|| self.phones.first())
            .map(|p| p.number.as_str())
    }
}

impl Default for Contact {
    fn default() -> Self {
        Self::new()
    }
}
