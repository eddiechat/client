//! CardDAV backend for contact management
//!
//! This module implements the CardDAV protocol for fetching and managing contacts
//! from CardDAV-compatible servers (like Gmail, iCloud, Fastmail, etc.)

use base64::Engine;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use std::collections::HashMap;
use std::process::Command;
use tracing::info;

use crate::config::{self, AccountConfig, AuthConfig, CardDAVConfig, PasswordSource};
use crate::types::contact::{AddressBook, Contact, ContactAddress, ContactEmail, ContactPhone};
use crate::types::error::HimalayaError;

/// CardDAV backend for contact operations
pub struct CardDAVBackend {
    config: CardDAVConfig,
    account_email: String,
    client: Client,
    auth_header: String,
}

impl CardDAVBackend {
    /// Create a new CardDAV backend for an account
    pub async fn new(account_name: &str) -> Result<Self, HimalayaError> {
        let app_config = config::get_config()?;
        let (_, account_config) = app_config
            .get_account(Some(account_name))
            .ok_or_else(|| HimalayaError::AccountNotFound(account_name.to_string()))?;

        Self::from_account_config(account_config.clone()).await
    }

    /// Create from account config
    async fn from_account_config(account_config: AccountConfig) -> Result<Self, HimalayaError> {
        let carddav = account_config
            .carddav
            .as_ref()
            .ok_or_else(|| HimalayaError::Config("No CardDAV configuration".to_string()))?
            .clone();

        // Build HTTP client
        let mut client_builder = Client::builder();

        if !carddav.tls {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        let client = client_builder
            .build()
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        // Build auth header
        let auth_header = Self::build_auth_header(&carddav.auth).await?;

        Ok(Self {
            config: carddav,
            account_email: account_config.email.clone(),
            client,
            auth_header,
        })
    }

    /// Create backend for default account
    pub async fn default() -> Result<Self, HimalayaError> {
        let config = config::get_config()?;
        let account_name = config
            .default_account_name()
            .ok_or_else(|| HimalayaError::Config("No accounts configured".to_string()))?
            .to_string();

        Self::new(&account_name).await
    }

    /// Build HTTP Basic auth header
    async fn build_auth_header(auth: &AuthConfig) -> Result<String, HimalayaError> {
        match auth {
            AuthConfig::Password { user, password } => {
                let passwd = Self::resolve_password(password).await?;
                let credentials = format!("{}:{}", user, passwd);
                let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
                Ok(format!("Basic {}", encoded))
            }
            AuthConfig::OAuth2 { .. } => {
                Err(HimalayaError::Config("OAuth2 not yet supported for CardDAV".to_string()))
            }
        }
    }

    /// Resolve password from PasswordSource
    async fn resolve_password(source: &PasswordSource) -> Result<String, HimalayaError> {
        match source {
            PasswordSource::Raw(password) => Ok(password.clone()),
            PasswordSource::Command { command } => {
                info!("Executing password command for CardDAV");
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .map_err(|e| {
                        HimalayaError::Config(format!("Failed to run password command: {}", e))
                    })?;

                if !output.status.success() {
                    return Err(HimalayaError::Config("Password command failed".to_string()));
                }

                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
        }
    }

    /// Get default headers for CardDAV requests
    fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&self.auth_header).unwrap(),
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );
        headers
    }

    /// Discover the principal URL for the user
    async fn discover_principal(&self) -> Result<String, HimalayaError> {
        info!("Discovering CardDAV principal URL");

        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:current-user-principal/>
  </d:prop>
</d:propfind>"#;

        let response = self
            .client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &self.config.url)
            .headers(self.default_headers())
            .header("Depth", "0")
            .body(body)
            .send()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        if !status.is_success() && status.as_u16() != 207 {
            return Err(HimalayaError::Backend(format!(
                "Failed to discover principal: {} - {}",
                status, text
            )));
        }

        // Parse the XML response to find current-user-principal
        Self::parse_principal_response(&text, &self.config.url)
    }

    /// Parse principal response XML
    fn parse_principal_response(xml: &str, base_url: &str) -> Result<String, HimalayaError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut in_principal = false;
        let mut href = None;

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("current-user-principal") {
                        in_principal = true;
                    } else if in_principal && name.contains("href") {
                        // For empty elements, try to get text content
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            href = Some(t.unescape().unwrap_or_default().to_string());
                        }
                    }
                }
                Ok(Event::Text(e)) if in_principal && href.is_none() => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    if !text.trim().is_empty() {
                        href = Some(text);
                    }
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("current-user-principal") {
                        in_principal = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(HimalayaError::Parse(format!(
                        "Failed to parse principal XML: {}",
                        e
                    )));
                }
                _ => {}
            }
        }

        match href {
            Some(h) => {
                // Handle relative URLs
                if h.starts_with('/') {
                    let url = url::Url::parse(base_url)
                        .map_err(|e| HimalayaError::Parse(e.to_string()))?;
                    Ok(format!("{}://{}{}", url.scheme(), url.host_str().unwrap_or(""), h))
                } else if h.starts_with("http") {
                    Ok(h)
                } else {
                    Ok(format!("{}/{}", base_url.trim_end_matches('/'), h))
                }
            }
            None => {
                // Fallback: try using the base URL as principal
                Ok(base_url.to_string())
            }
        }
    }

    /// Discover the address book home set
    async fn discover_addressbook_home(&self, principal_url: &str) -> Result<String, HimalayaError> {
        info!("Discovering CardDAV address book home");

        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop>
    <c:addressbook-home-set/>
  </d:prop>
</d:propfind>"#;

        let response = self
            .client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), principal_url)
            .headers(self.default_headers())
            .header("Depth", "0")
            .body(body)
            .send()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        if !status.is_success() && status.as_u16() != 207 {
            return Err(HimalayaError::Backend(format!(
                "Failed to discover addressbook home: {} - {}",
                status, text
            )));
        }

        Self::parse_addressbook_home_response(&text, principal_url)
    }

    /// Parse addressbook home response
    fn parse_addressbook_home_response(xml: &str, base_url: &str) -> Result<String, HimalayaError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut in_home = false;
        let mut href = None;

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("addressbook-home-set") {
                        in_home = true;
                    } else if in_home && name.contains("href") {
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            href = Some(t.unescape().unwrap_or_default().to_string());
                        }
                    }
                }
                Ok(Event::Text(e)) if in_home && href.is_none() => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    if !text.trim().is_empty() {
                        href = Some(text);
                    }
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("addressbook-home-set") {
                        in_home = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(HimalayaError::Parse(format!(
                        "Failed to parse addressbook home XML: {}",
                        e
                    )));
                }
                _ => {}
            }
        }

        match href {
            Some(h) => {
                if h.starts_with('/') {
                    let url = url::Url::parse(base_url)
                        .map_err(|e| HimalayaError::Parse(e.to_string()))?;
                    Ok(format!("{}://{}{}", url.scheme(), url.host_str().unwrap_or(""), h))
                } else if h.starts_with("http") {
                    Ok(h)
                } else {
                    Ok(format!("{}/{}", base_url.trim_end_matches('/'), h))
                }
            }
            None => {
                // Fallback: use principal URL
                Ok(base_url.to_string())
            }
        }
    }

    /// List available address books
    pub async fn list_address_books(&self) -> Result<Vec<AddressBook>, HimalayaError> {
        info!("Listing CardDAV address books");

        // If address book is configured, just return that one
        if let Some(ref address_book) = self.config.address_book {
            return Ok(vec![AddressBook {
                name: "Contacts".to_string(),
                href: address_book.clone(),
                description: None,
                contact_count: None,
            }]);
        }

        // Discover principal and address book home
        let principal = self.discover_principal().await?;
        let home = self.discover_addressbook_home(&principal).await?;

        // List collections in the address book home
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop>
    <d:displayname/>
    <d:resourcetype/>
    <c:addressbook-description/>
  </d:prop>
</d:propfind>"#;

        let response = self
            .client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &home)
            .headers(self.default_headers())
            .header("Depth", "1")
            .body(body)
            .send()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        if !status.is_success() && status.as_u16() != 207 {
            return Err(HimalayaError::Backend(format!(
                "Failed to list address books: {} - {}",
                status, text
            )));
        }

        Self::parse_address_books_response(&text, &home)
    }

    /// Parse address books response
    fn parse_address_books_response(xml: &str, base_url: &str) -> Result<Vec<AddressBook>, HimalayaError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut address_books = Vec::new();
        let mut current_href: Option<String> = None;
        let mut current_name: Option<String> = None;
        let mut current_desc: Option<String> = None;
        let mut is_addressbook = false;
        let mut in_response = false;
        let mut in_prop = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("response") && !name.contains("multistatus") {
                        in_response = true;
                        current_href = None;
                        current_name = None;
                        current_desc = None;
                        is_addressbook = false;
                    } else if name.contains("prop") && !name.contains("propstat") {
                        in_prop = true;
                    } else if in_response && name.contains("href") && current_href.is_none() {
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            current_href = Some(t.unescape().unwrap_or_default().to_string());
                        }
                    } else if in_prop && name.contains("displayname") {
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            current_name = Some(t.unescape().unwrap_or_default().to_string());
                        }
                    } else if in_prop && name.contains("addressbook-description") {
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            current_desc = Some(t.unescape().unwrap_or_default().to_string());
                        }
                    }
                }
                Ok(Event::Empty(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if in_prop && name.contains("addressbook") && !name.contains("description") && !name.contains("home") {
                        is_addressbook = true;
                    }
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("response") && !name.contains("multistatus") {
                        if is_addressbook {
                            if let Some(href) = current_href.take() {
                                let full_href = if href.starts_with('/') {
                                    let url = url::Url::parse(base_url).ok();
                                    url.map(|u| format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), href))
                                        .unwrap_or(href)
                                } else if href.starts_with("http") {
                                    href
                                } else {
                                    format!("{}/{}", base_url.trim_end_matches('/'), href)
                                };

                                address_books.push(AddressBook {
                                    name: current_name.take().unwrap_or_else(|| "Contacts".to_string()),
                                    href: full_href,
                                    description: current_desc.take(),
                                    contact_count: None,
                                });
                            }
                        }
                        in_response = false;
                    } else if name.contains("prop") && !name.contains("propstat") {
                        in_prop = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(HimalayaError::Parse(format!(
                        "Failed to parse address books XML: {}",
                        e
                    )));
                }
                _ => {}
            }
        }

        Ok(address_books)
    }

    /// Get the address book URL to use
    async fn get_address_book_url(&self) -> Result<String, HimalayaError> {
        if let Some(ref address_book) = self.config.address_book {
            return Ok(address_book.clone());
        }

        let books = self.list_address_books().await?;
        books
            .into_iter()
            .next()
            .map(|b| b.href)
            .ok_or_else(|| HimalayaError::Config("No address books found".to_string()))
    }

    /// List all contacts from the address book
    pub async fn list_contacts(&self) -> Result<Vec<Contact>, HimalayaError> {
        info!("Fetching contacts from CardDAV server");

        let address_book_url = self.get_address_book_url().await?;

        // Use REPORT with addressbook-query to fetch all contacts
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<c:addressbook-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop>
    <d:getetag/>
    <c:address-data/>
  </d:prop>
</c:addressbook-query>"#;

        let response = self
            .client
            .request(reqwest::Method::from_bytes(b"REPORT").unwrap(), &address_book_url)
            .headers(self.default_headers())
            .header("Depth", "1")
            .body(body)
            .send()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        if !status.is_success() && status.as_u16() != 207 {
            return Err(HimalayaError::Backend(format!(
                "Failed to fetch contacts: {} - {}",
                status, text
            )));
        }

        Self::parse_contacts_response(&text, &address_book_url)
    }

    /// Parse contacts response from REPORT
    fn parse_contacts_response(xml: &str, base_url: &str) -> Result<Vec<Contact>, HimalayaError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut contacts = Vec::new();
        let mut current_href: Option<String> = None;
        let mut current_etag: Option<String> = None;
        let mut current_vcard: Option<String> = None;
        let mut in_response = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("response") && !name.contains("multistatus") {
                        in_response = true;
                        current_href = None;
                        current_etag = None;
                        current_vcard = None;
                    } else if in_response && name.contains("href") && current_href.is_none() {
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            current_href = Some(t.unescape().unwrap_or_default().to_string());
                        }
                    } else if in_response && name.contains("getetag") {
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            let etag = t.unescape().unwrap_or_default().to_string();
                            current_etag = Some(etag.trim_matches('"').to_string());
                        }
                    } else if in_response && name.contains("address-data") {
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            current_vcard = Some(t.unescape().unwrap_or_default().to_string());
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.contains("response") && !name.contains("multistatus") {
                        if let Some(vcard) = current_vcard.take() {
                            if !vcard.trim().is_empty() {
                                if let Ok(mut contact) = Self::parse_vcard(&vcard) {
                                    contact.raw_vcard = Some(vcard);
                                    contact.etag = current_etag.take();
                                    if let Some(href) = current_href.take() {
                                        let full_href = if href.starts_with('/') {
                                            let url = url::Url::parse(base_url).ok();
                                            url.map(|u| format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), href))
                                                .unwrap_or(href)
                                        } else {
                                            href
                                        };
                                        contact.href = Some(full_href);
                                    }
                                    contacts.push(contact);
                                }
                            }
                        }
                        in_response = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(HimalayaError::Parse(format!(
                        "Failed to parse contacts XML: {}",
                        e
                    )));
                }
                _ => {}
            }
        }

        info!("Parsed {} contacts", contacts.len());
        Ok(contacts)
    }

    /// Parse a vCard string into a Contact
    fn parse_vcard(vcard: &str) -> Result<Contact, HimalayaError> {
        let mut contact = Contact::new();
        let mut current_property: Option<String> = None;
        let mut property_params: HashMap<String, String> = HashMap::new();

        for line in vcard.lines() {
            let line = line.trim();

            // Handle line continuation
            if line.starts_with(' ') || line.starts_with('\t') {
                if let Some(ref mut prop) = current_property {
                    prop.push_str(line.trim());
                }
                continue;
            }

            // Parse property
            if let Some((name_part, value)) = line.split_once(':') {
                current_property = Some(value.to_string());
                property_params.clear();

                // Parse name and parameters
                let parts: Vec<&str> = name_part.split(';').collect();
                let property_name = parts[0].to_uppercase();

                // Parse parameters
                for param in parts.iter().skip(1) {
                    if let Some((key, val)) = param.split_once('=') {
                        property_params.insert(key.to_uppercase(), val.to_string());
                    } else {
                        // TYPE parameter shorthand
                        property_params.insert("TYPE".to_string(), param.to_string());
                    }
                }

                let value = current_property.as_ref().unwrap();

                match property_name.as_str() {
                    "UID" => {
                        contact.id = value.clone();
                    }
                    "FN" => {
                        contact.full_name = Self::decode_vcard_value(value);
                    }
                    "N" => {
                        // Format: Family;Given;Middle;Prefix;Suffix
                        let parts: Vec<&str> = value.split(';').collect();
                        if let Some(family) = parts.first() {
                            if !family.is_empty() {
                                contact.family_name = Some(Self::decode_vcard_value(family));
                            }
                        }
                        if let Some(given) = parts.get(1) {
                            if !given.is_empty() {
                                contact.given_name = Some(Self::decode_vcard_value(given));
                            }
                        }
                    }
                    "NICKNAME" => {
                        contact.nickname = Some(Self::decode_vcard_value(value));
                    }
                    "EMAIL" => {
                        let email_type = property_params.get("TYPE").cloned();
                        let pref = property_params.contains_key("PREF")
                            || email_type.as_ref().map(|t| t.to_uppercase().contains("PREF")).unwrap_or(false);
                        contact.emails.push(ContactEmail {
                            email: Self::decode_vcard_value(value),
                            email_type: email_type.map(|t| t.replace("PREF,", "").replace(",PREF", "").replace("PREF", "")),
                            primary: pref,
                        });
                    }
                    "TEL" => {
                        let phone_type = property_params.get("TYPE").cloned();
                        let pref = property_params.contains_key("PREF")
                            || phone_type.as_ref().map(|t| t.to_uppercase().contains("PREF")).unwrap_or(false);
                        contact.phones.push(ContactPhone {
                            number: Self::decode_vcard_value(value),
                            phone_type: phone_type.map(|t| t.replace("PREF,", "").replace(",PREF", "").replace("PREF", "")),
                            primary: pref,
                        });
                    }
                    "ADR" => {
                        // Format: PO Box;Extended;Street;City;State;Postal;Country
                        let parts: Vec<&str> = value.split(';').collect();
                        let addr_type = property_params.get("TYPE").cloned();
                        let pref = property_params.contains_key("PREF")
                            || addr_type.as_ref().map(|t| t.to_uppercase().contains("PREF")).unwrap_or(false);

                        contact.addresses.push(ContactAddress {
                            address_type: addr_type.map(|t| t.replace("PREF,", "").replace(",PREF", "").replace("PREF", "")),
                            street: parts.get(2).filter(|s| !s.is_empty()).map(|s| Self::decode_vcard_value(s)),
                            city: parts.get(3).filter(|s| !s.is_empty()).map(|s| Self::decode_vcard_value(s)),
                            state: parts.get(4).filter(|s| !s.is_empty()).map(|s| Self::decode_vcard_value(s)),
                            postal_code: parts.get(5).filter(|s| !s.is_empty()).map(|s| Self::decode_vcard_value(s)),
                            country: parts.get(6).filter(|s| !s.is_empty()).map(|s| Self::decode_vcard_value(s)),
                            primary: pref,
                        });
                    }
                    "ORG" => {
                        contact.organization = Some(Self::decode_vcard_value(value.split(';').next().unwrap_or(value)));
                    }
                    "TITLE" => {
                        contact.title = Some(Self::decode_vcard_value(value));
                    }
                    "BDAY" => {
                        contact.birthday = Some(value.clone());
                    }
                    "NOTE" => {
                        contact.notes = Some(Self::decode_vcard_value(value));
                    }
                    "PHOTO" => {
                        // Handle PHOTO URL
                        if value.starts_with("http") {
                            contact.photo_url = Some(value.clone());
                        }
                    }
                    _ => {}
                }
            }
        }

        // Generate ID if not present
        if contact.id.is_empty() {
            contact.id = uuid::Uuid::new_v4().to_string();
        }

        Ok(contact)
    }

    /// Decode vCard escaped values
    fn decode_vcard_value(value: &str) -> String {
        value
            .replace("\\n", "\n")
            .replace("\\N", "\n")
            .replace("\\,", ",")
            .replace("\\;", ";")
            .replace("\\\\", "\\")
    }

    /// Encode value for vCard
    fn encode_vcard_value(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace(';', "\\;")
            .replace(',', "\\,")
            .replace('\n', "\\n")
    }

    /// Generate vCard from Contact
    fn generate_vcard(contact: &Contact) -> String {
        let mut lines = vec![
            "BEGIN:VCARD".to_string(),
            "VERSION:3.0".to_string(),
            format!("UID:{}", contact.id),
            format!("FN:{}", Self::encode_vcard_value(&contact.full_name)),
        ];

        // N property
        let family = contact.family_name.as_deref().unwrap_or("");
        let given = contact.given_name.as_deref().unwrap_or("");
        lines.push(format!(
            "N:{};{};;;",
            Self::encode_vcard_value(family),
            Self::encode_vcard_value(given)
        ));

        // Nickname
        if let Some(ref nickname) = contact.nickname {
            lines.push(format!("NICKNAME:{}", Self::encode_vcard_value(nickname)));
        }

        // Emails
        for email in &contact.emails {
            let mut parts = vec!["EMAIL".to_string()];
            if let Some(ref t) = email.email_type {
                parts.push(format!("TYPE={}", t));
            }
            if email.primary {
                parts.push("PREF".to_string());
            }
            lines.push(format!("{}:{}", parts.join(";"), email.email));
        }

        // Phones
        for phone in &contact.phones {
            let mut parts = vec!["TEL".to_string()];
            if let Some(ref t) = phone.phone_type {
                parts.push(format!("TYPE={}", t));
            }
            if phone.primary {
                parts.push("PREF".to_string());
            }
            lines.push(format!("{}:{}", parts.join(";"), phone.number));
        }

        // Addresses
        for addr in &contact.addresses {
            let mut parts = vec!["ADR".to_string()];
            if let Some(ref t) = addr.address_type {
                parts.push(format!("TYPE={}", t));
            }
            if addr.primary {
                parts.push("PREF".to_string());
            }
            let value = format!(
                ";;{};{};{};{};{}",
                addr.street.as_deref().map(Self::encode_vcard_value).unwrap_or_default(),
                addr.city.as_deref().map(Self::encode_vcard_value).unwrap_or_default(),
                addr.state.as_deref().map(Self::encode_vcard_value).unwrap_or_default(),
                addr.postal_code.as_deref().unwrap_or(""),
                addr.country.as_deref().map(Self::encode_vcard_value).unwrap_or_default(),
            );
            lines.push(format!("{}:{}", parts.join(";"), value));
        }

        // Organization
        if let Some(ref org) = contact.organization {
            lines.push(format!("ORG:{}", Self::encode_vcard_value(org)));
        }

        // Title
        if let Some(ref title) = contact.title {
            lines.push(format!("TITLE:{}", Self::encode_vcard_value(title)));
        }

        // Birthday
        if let Some(ref bday) = contact.birthday {
            lines.push(format!("BDAY:{}", bday));
        }

        // Notes
        if let Some(ref notes) = contact.notes {
            lines.push(format!("NOTE:{}", Self::encode_vcard_value(notes)));
        }

        lines.push("END:VCARD".to_string());

        lines.join("\r\n")
    }

    /// Create a new contact
    pub async fn create_contact(&self, contact: &Contact) -> Result<Contact, HimalayaError> {
        info!("Creating contact: {}", contact.full_name);

        let address_book_url = self.get_address_book_url().await?;
        let vcard = Self::generate_vcard(contact);
        let contact_url = format!(
            "{}/{}.vcf",
            address_book_url.trim_end_matches('/'),
            contact.id
        );

        let mut headers = self.default_headers();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/vcard; charset=utf-8"));

        let response = self
            .client
            .put(&contact_url)
            .headers(headers)
            .body(vcard.clone())
            .send()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(HimalayaError::Backend(format!(
                "Failed to create contact: {} - {}",
                status, text
            )));
        }

        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_matches('"').to_string());

        let mut created = contact.clone();
        created.href = Some(contact_url);
        created.etag = etag;
        created.raw_vcard = Some(vcard);

        info!("Contact created successfully");
        Ok(created)
    }

    /// Update an existing contact
    pub async fn update_contact(&self, contact: &Contact) -> Result<Contact, HimalayaError> {
        info!("Updating contact: {}", contact.full_name);

        let href = contact
            .href
            .as_ref()
            .ok_or_else(|| HimalayaError::Config("Contact has no href for update".to_string()))?;

        let vcard = Self::generate_vcard(contact);

        let mut headers = self.default_headers();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/vcard; charset=utf-8"));

        // Add If-Match header for optimistic concurrency if we have an etag
        if let Some(ref etag) = contact.etag {
            headers.insert(
                reqwest::header::IF_MATCH,
                HeaderValue::from_str(&format!("\"{}\"", etag)).unwrap(),
            );
        }

        let response = self
            .client
            .put(href)
            .headers(headers)
            .body(vcard.clone())
            .send()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(HimalayaError::Backend(format!(
                "Failed to update contact: {} - {}",
                status, text
            )));
        }

        let new_etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_matches('"').to_string());

        let mut updated = contact.clone();
        updated.etag = new_etag;
        updated.raw_vcard = Some(vcard);

        info!("Contact updated successfully");
        Ok(updated)
    }

    /// Delete a contact
    pub async fn delete_contact(&self, contact_id: &str, href: Option<&str>) -> Result<(), HimalayaError> {
        info!("Deleting contact: {}", contact_id);

        let url = if let Some(h) = href {
            h.to_string()
        } else {
            // Try to find the contact to get its href
            let contacts = self.list_contacts().await?;
            contacts
                .into_iter()
                .find(|c| c.id == contact_id)
                .and_then(|c| c.href)
                .ok_or_else(|| HimalayaError::Config("Contact not found".to_string()))?
        };

        let response = self
            .client
            .delete(&url)
            .headers(self.default_headers())
            .send()
            .await
            .map_err(|e| HimalayaError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() && status.as_u16() != 404 {
            let text = response.text().await.unwrap_or_default();
            return Err(HimalayaError::Backend(format!(
                "Failed to delete contact: {} - {}",
                status, text
            )));
        }

        info!("Contact deleted successfully");
        Ok(())
    }

    /// Get a single contact by ID
    pub async fn get_contact(&self, contact_id: &str) -> Result<Contact, HimalayaError> {
        // For now, we fetch all contacts and filter
        // A more efficient implementation would use addressbook-multiget
        let contacts = self.list_contacts().await?;
        contacts
            .into_iter()
            .find(|c| c.id == contact_id)
            .ok_or_else(|| HimalayaError::Config(format!("Contact '{}' not found", contact_id)))
    }
}

/// Get CardDAV backend for account (or default)
pub async fn get_carddav_backend(account: Option<&str>) -> Result<CardDAVBackend, HimalayaError> {
    match account {
        Some(name) => CardDAVBackend::new(name).await,
        None => CardDAVBackend::default().await,
    }
}
