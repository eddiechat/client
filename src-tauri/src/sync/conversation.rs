//! Conversation Grouping by Participant Set
//!
//! Groups messages by normalized participant set (all addresses in From/To/Cc,
//! excluding the user's own address). A conversation is defined by
//! "the same group of people talking".

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::sync::db::{CachedConversation, CachedMessage, SyncDatabase};
use crate::types::error::HimalayaError;

/// Participant information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Participant {
    pub email: String,
    pub name: Option<String>,
}

impl Participant {
    /// Create from an email address string (may include name)
    pub fn from_address(addr: &str) -> Self {
        let addr = addr.trim();

        // Handle "Name <email>" format
        if let (Some(name_end), Some(email_start)) = (addr.rfind('<'), addr.rfind('>')) {
            if email_start > name_end {
                let email = addr[name_end + 1..email_start].trim().to_lowercase();
                let name = addr[..name_end].trim().trim_matches('"').trim();
                return Self {
                    email,
                    name: if name.is_empty() { None } else { Some(name.to_string()) },
                };
            }
        }

        // Plain email address
        Self {
            email: addr.to_lowercase(),
            name: None,
        }
    }

    /// Normalize the email address for comparison
    pub fn normalize_email(email: &str) -> String {
        let email = email.to_lowercase();

        // Handle Gmail dot-insensitivity and plus addressing
        if email.ends_with("@gmail.com") || email.ends_with("@googlemail.com") {
            if let Some(at_pos) = email.find('@') {
                let local = &email[..at_pos];
                let domain = &email[at_pos..];

                // Remove dots and everything after +
                let normalized_local = local
                    .replace('.', "")
                    .split('+')
                    .next()
                    .unwrap_or(local)
                    .to_string();

                return format!("{}{}", normalized_local, domain);
            }
        }

        // For other providers, just handle plus addressing
        if let Some(at_pos) = email.find('@') {
            let local = &email[..at_pos];
            let domain = &email[at_pos..];

            let normalized_local = local
                .split('+')
                .next()
                .unwrap_or(local)
                .to_string();

            return format!("{}{}", normalized_local, domain);
        }

        email
    }
}

/// Conversation grouper
pub struct ConversationGrouper {
    db: Arc<SyncDatabase>,
}

impl ConversationGrouper {
    /// Create a new conversation grouper
    pub fn new(db: Arc<SyncDatabase>) -> Self {
        Self { db }
    }

    /// Generate a participant key for a message
    ///
    /// The key is a sorted, comma-separated list of normalized email addresses,
    /// excluding the user's own address.
    pub fn generate_participant_key(
        user_email: &str,
        from: &str,
        to: &[String],
        cc: Option<&[String]>,
    ) -> String {
        let user_normalized = Participant::normalize_email(user_email);

        let mut participants: HashSet<String> = HashSet::new();

        // Add from address
        let from_participant = Participant::from_address(from);
        let from_normalized = Participant::normalize_email(&from_participant.email);
        if from_normalized != user_normalized {
            participants.insert(from_normalized);
        }

        // Add to addresses
        for addr in to {
            let participant = Participant::from_address(addr);
            let normalized = Participant::normalize_email(&participant.email);
            if normalized != user_normalized {
                participants.insert(normalized);
            }
        }

        // Add cc addresses
        if let Some(cc_list) = cc {
            for addr in cc_list {
                let participant = Participant::from_address(addr);
                let normalized = Participant::normalize_email(&participant.email);
                if normalized != user_normalized {
                    participants.insert(normalized);
                }
            }
        }

        // Sort and join
        let mut sorted: Vec<String> = participants.into_iter().collect();
        sorted.sort();
        sorted.join(",")
    }

    /// Extract participant info from a message
    pub fn extract_participants(
        user_email: &str,
        from: &str,
        to: &[String],
        cc: Option<&[String]>,
    ) -> Vec<Participant> {
        let user_normalized = Participant::normalize_email(user_email);
        let mut seen: HashSet<String> = HashSet::new();
        let mut participants: Vec<Participant> = Vec::new();

        // Add from
        let from_participant = Participant::from_address(from);
        let from_normalized = Participant::normalize_email(&from_participant.email);
        if from_normalized != user_normalized && !seen.contains(&from_normalized) {
            seen.insert(from_normalized);
            participants.push(from_participant);
        }

        // Add to
        for addr in to {
            let participant = Participant::from_address(addr);
            let normalized = Participant::normalize_email(&participant.email);
            if normalized != user_normalized && !seen.contains(&normalized) {
                seen.insert(normalized);
                participants.push(participant);
            }
        }

        // Add cc
        if let Some(cc_list) = cc {
            for addr in cc_list {
                let participant = Participant::from_address(addr);
                let normalized = Participant::normalize_email(&participant.email);
                if normalized != user_normalized && !seen.contains(&normalized) {
                    seen.insert(normalized);
                    participants.push(participant);
                }
            }
        }

        participants
    }

    /// Assign a message to a conversation (creates if needed)
    pub fn assign_to_conversation(
        &self,
        account_id: &str,
        user_email: &str,
        message: &CachedMessage,
    ) -> Result<i64, HimalayaError> {
        // Parse addresses
        let to_addresses: Vec<String> = serde_json::from_str(&message.to_addresses)
            .unwrap_or_default();
        let cc_addresses: Option<Vec<String>> = message.cc_addresses
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        // Generate participant key
        let participant_key = Self::generate_participant_key(
            user_email,
            &message.from_address,
            &to_addresses,
            cc_addresses.as_deref(),
        );

        // Skip messages with no other participants (self-sent only)
        if participant_key.is_empty() {
            return Err(HimalayaError::Backend(
                "Message has no participants other than user".to_string()
            ));
        }

        // Extract participant info
        let participants = Self::extract_participants(
            user_email,
            &message.from_address,
            &to_addresses,
            cc_addresses.as_deref(),
        );

        let participants_json = serde_json::to_string(&participants)
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Check if message is from user (outgoing)
        let user_normalized = Participant::normalize_email(user_email);
        let from_participant = Participant::from_address(&message.from_address);
        let is_outgoing = Participant::normalize_email(&from_participant.email) == user_normalized;

        // Parse flags to check for \Seen
        let flags: Vec<String> = serde_json::from_str(&message.flags).unwrap_or_default();
        let is_seen = flags.iter().any(|f| f.to_lowercase() == "\\seen" || f.to_lowercase() == "seen");

        // Get or create conversation
        let existing = self.db.get_conversation_by_key(account_id, &participant_key)?;

        let conversation = if let Some(mut conv) = existing {
            // Update existing conversation
            let should_update_last = match (&message.date, &conv.last_message_date) {
                (Some(msg_date), Some(conv_date)) => msg_date >= conv_date,
                (Some(_), None) => true,
                _ => false,
            };

            if should_update_last {
                conv.last_message_date = message.date;
                conv.last_message_preview = message.text_body.as_ref()
                    .or(message.subject.as_ref())
                    .map(|s| {
                        let trimmed = s.trim();
                        if trimmed.len() > 100 {
                            format!("{}...", &trimmed.chars().take(100).collect::<String>())
                        } else {
                            trimmed.to_string()
                        }
                    });
                conv.last_message_from = message.from_name.clone()
                    .or_else(|| Some(from_participant.email.clone()));
                conv.is_outgoing = is_outgoing;
            }

            conv.message_count += 1;
            if !is_seen && !is_outgoing {
                conv.unread_count += 1;
            }

            conv
        } else {
            // Create new conversation
            CachedConversation {
                id: 0,
                account_id: account_id.to_string(),
                participant_key: participant_key.clone(),
                participants: participants_json,
                last_message_date: message.date,
                last_message_preview: message.text_body.as_ref()
                    .or(message.subject.as_ref())
                    .map(|s| {
                        let trimmed = s.trim();
                        if trimmed.len() > 100 {
                            format!("{}...", &trimmed.chars().take(100).collect::<String>())
                        } else {
                            trimmed.to_string()
                        }
                    }),
                last_message_from: message.from_name.clone()
                    .or_else(|| Some(from_participant.email.clone())),
                message_count: 1,
                unread_count: if !is_seen && !is_outgoing { 1 } else { 0 },
                is_outgoing,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        };

        // Save conversation
        let conv_id = self.db.upsert_conversation(&conversation)?;

        // Link message to conversation
        self.db.link_message_to_conversation(conv_id, message.id)?;

        Ok(conv_id)
    }

    /// Rebuild conversations for an account
    ///
    /// This is useful after initial sync or when fixing data issues.
    pub fn rebuild_conversations(&self, account_id: &str, user_email: &str) -> Result<u32, HimalayaError> {
        // Get all messages for the account across all folders
        let conn = self.db.connection()?;

        // First, clear existing conversations
        conn.execute(
            "DELETE FROM conversations WHERE account_id = ?1",
            rusqlite::params![account_id],
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Get messages grouped by participant key
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE account_id = ?1
             ORDER BY date ASC"
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let messages: Vec<CachedMessage> = stmt.query_map(
            rusqlite::params![account_id],
            |row| {
                Ok(CachedMessage {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    folder_name: row.get(2)?,
                    uid: row.get(3)?,
                    message_id: row.get(4)?,
                    in_reply_to: row.get(5)?,
                    references: row.get(6)?,
                    from_address: row.get(7)?,
                    from_name: row.get(8)?,
                    to_addresses: row.get(9)?,
                    cc_addresses: row.get(10)?,
                    subject: row.get(11)?,
                    date: row.get::<_, Option<String>>(12)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    flags: row.get(13)?,
                    has_attachment: row.get::<_, i32>(14)? != 0,
                    body_cached: row.get::<_, i32>(15)? != 0,
                    text_body: row.get(16)?,
                    html_body: row.get(17)?,
                    raw_size: row.get(18)?,
                    created_at: row.get::<_, String>(19)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    updated_at: row.get::<_, String>(20)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            }
        ).map_err(|e| HimalayaError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);
        drop(conn);

        let mut count = 0;
        for message in messages {
            if self.assign_to_conversation(account_id, user_email, &message).is_ok() {
                count += 1;
            }
        }

        // Clean up empty conversations
        self.db.delete_empty_conversations(account_id)?;

        Ok(count)
    }

    /// Update conversation when a message is deleted
    pub fn handle_message_deleted(&self, account_id: &str, _message_id: i64) -> Result<(), HimalayaError> {
        // The conversation_messages table has ON DELETE CASCADE,
        // so the link is automatically removed.
        // We just need to clean up empty conversations.
        self.db.delete_empty_conversations(account_id)?;
        Ok(())
    }

    /// Update conversation when message flags change
    pub fn update_conversation_for_flag_change(
        &self,
        account_id: &str,
        user_email: &str,
        message: &CachedMessage,
        old_flags: &[String],
        new_flags: &[String],
    ) -> Result<(), HimalayaError> {
        let was_seen = old_flags.iter().any(|f| f.to_lowercase() == "\\seen");
        let is_seen = new_flags.iter().any(|f| f.to_lowercase() == "\\seen");

        if was_seen == is_seen {
            return Ok(()); // No change
        }

        // Get the conversation for this message
        let to_addresses: Vec<String> = serde_json::from_str(&message.to_addresses)
            .unwrap_or_default();
        let cc_addresses: Option<Vec<String>> = message.cc_addresses
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        let participant_key = Self::generate_participant_key(
            user_email,
            &message.from_address,
            &to_addresses,
            cc_addresses.as_deref(),
        );

        if let Some(mut conv) = self.db.get_conversation_by_key(account_id, &participant_key)? {
            // Check if this is an incoming message
            let user_normalized = Participant::normalize_email(user_email);
            let from_participant = Participant::from_address(&message.from_address);
            let is_outgoing = Participant::normalize_email(&from_participant.email) == user_normalized;

            if !is_outgoing {
                if is_seen && !was_seen {
                    // Message was marked as read
                    conv.unread_count = conv.unread_count.saturating_sub(1);
                } else if !is_seen && was_seen {
                    // Message was marked as unread
                    conv.unread_count += 1;
                }

                self.db.upsert_conversation(&conv)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_participant_from_address() {
        let p = Participant::from_address("John Doe <john@example.com>");
        assert_eq!(p.email, "john@example.com");
        assert_eq!(p.name, Some("John Doe".to_string()));

        let p = Participant::from_address("john@example.com");
        assert_eq!(p.email, "john@example.com");
        assert_eq!(p.name, None);

        let p = Participant::from_address("\"Jane Doe\" <jane@example.com>");
        assert_eq!(p.email, "jane@example.com");
        assert_eq!(p.name, Some("Jane Doe".to_string()));
    }

    #[test]
    fn test_normalize_gmail() {
        assert_eq!(
            Participant::normalize_email("john.doe@gmail.com"),
            "johndoe@gmail.com"
        );
        assert_eq!(
            Participant::normalize_email("john.doe+test@gmail.com"),
            "johndoe@gmail.com"
        );
    }

    #[test]
    fn test_generate_participant_key() {
        let key = ConversationGrouper::generate_participant_key(
            "me@example.com",
            "alice@example.com",
            &["bob@example.com".to_string(), "me@example.com".to_string()],
            None,
        );

        // Should exclude me@example.com and sort
        assert_eq!(key, "alice@example.com,bob@example.com");
    }

    #[test]
    fn test_participant_key_normalization() {
        let key1 = ConversationGrouper::generate_participant_key(
            "me@gmail.com",
            "john.doe@gmail.com",
            &["me@gmail.com".to_string()],
            None,
        );

        let key2 = ConversationGrouper::generate_participant_key(
            "me@gmail.com",
            "johndoe@gmail.com",
            &["me@gmail.com".to_string()],
            None,
        );

        // Should be the same due to Gmail dot normalization
        assert_eq!(key1, key2);
    }
}
