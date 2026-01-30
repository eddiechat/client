//! Conversation Grouping by Participant Set
//!
//! Groups messages by normalized participant set (all addresses in From/To/Cc,
//! excluding the user's own address). A conversation is defined by
//! "the same group of people talking".

use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::sync::db::{CachedConversation, CachedMessage, SyncDatabase};
use crate::types::error::EddieError;

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
                    name: if name.is_empty() {
                        None
                    } else {
                        Some(name.to_string())
                    },
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

            let normalized_local = local.split('+').next().unwrap_or(local).to_string();

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
    ///
    /// This method uses atomic SQL operations to avoid race conditions when
    /// multiple threads process messages for the same conversation concurrently.
    /// Instead of read-modify-write in Rust, it:
    /// 1. Uses INSERT OR IGNORE to create the conversation if it doesn't exist
    /// 2. Uses atomic SQL UPDATE to increment counters
    pub fn assign_to_conversation(
        &self,
        account_id: &str,
        user_email: &str,
        message: &CachedMessage,
    ) -> Result<i64, EddieError> {
        // Parse addresses
        let to_addresses: Vec<String> =
            serde_json::from_str(&message.to_addresses).unwrap_or_default();
        let cc_addresses: Option<Vec<String>> = message
            .cc_addresses
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
            return Err(EddieError::Backend(
                "Message has no participants other than user".to_string(),
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
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Check if message is from user (outgoing)
        let user_normalized = Participant::normalize_email(user_email);
        let from_participant = Participant::from_address(&message.from_address);
        let is_outgoing = Participant::normalize_email(&from_participant.email) == user_normalized;

        // Parse flags to check for \Seen
        let flags: Vec<String> = serde_json::from_str(&message.flags).unwrap_or_default();
        let is_seen = flags
            .iter()
            .any(|f| f.to_lowercase() == "\\seen" || f.to_lowercase() == "seen");

        // Prepare last message info
        let last_message_preview = message
            .text_body
            .as_ref()
            .or(message.subject.as_ref())
            .map(|s| {
                let trimmed = s.trim();
                if trimmed.len() > 100 {
                    format!("{}...", &trimmed.chars().take(100).collect::<String>())
                } else {
                    trimmed.to_string()
                }
            });

        let last_message_from = message
            .from_name
            .clone()
            .or_else(|| Some(from_participant.email.clone()));

        // Step 1: Ensure conversation exists (atomic INSERT OR IGNORE)
        // This creates the conversation with initial values if it doesn't exist,
        // or does nothing if it already exists
        let initial_conv = CachedConversation {
            id: 0,
            account_id: account_id.to_string(),
            participant_key: participant_key.clone(),
            participants: participants_json,
            last_message_date: message.date,
            last_message_preview: last_message_preview.clone(),
            last_message_from: last_message_from.clone(),
            message_count: 0, // Will be incremented atomically
            unread_count: 0,  // Will be incremented atomically
            is_outgoing,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let conv_id = self.db.insert_conversation_if_not_exists(&initial_conv)?;

        // Step 2: Atomically increment counters using SQL
        // This avoids the read-modify-write race condition
        let increment_unread = !is_seen && !is_outgoing;

        self.db.increment_conversation_counters(
            conv_id,
            increment_unread,
            message.date,
            last_message_preview.as_deref(),
            last_message_from.as_deref(),
            is_outgoing,
        )?;

        // Step 3: Link message to conversation
        self.db.link_message_to_conversation(conv_id, message.id)?;

        Ok(conv_id)
    }

    /// Rebuild conversations for an account
    ///
    /// This is useful after initial sync or when fixing data issues.
    /// Uses a transaction to ensure atomicity - either all conversations are rebuilt
    /// or none are (on error, the database remains unchanged).
    pub fn rebuild_conversations(
        &self,
        account_id: &str,
        user_email: &str,
    ) -> Result<u32, EddieError> {
        // Get all messages for the account across all folders
        let mut conn = self.db.connection()?;

        // Start a transaction to ensure atomicity
        let tx = conn
            .transaction()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // First, clear existing conversations
        tx.execute(
            "DELETE FROM conversations WHERE account_id = ?1",
            rusqlite::params![account_id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Get messages grouped by participant key
        let mut stmt = tx.prepare(
            "SELECT id, account_id, folder_name, uid, message_id, in_reply_to, references_header,
                    from_address, from_name, to_addresses, cc_addresses, subject, date, flags,
                    has_attachment, body_cached, text_body, html_body, raw_size, created_at, updated_at
             FROM messages WHERE account_id = ?1
             ORDER BY date ASC"
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages: Vec<CachedMessage> = stmt
            .query_map(rusqlite::params![account_id], |row| {
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
                    date: row
                        .get::<_, Option<String>>(12)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    flags: row.get(13)?,
                    has_attachment: row.get::<_, i32>(14)? != 0,
                    body_cached: row.get::<_, i32>(15)? != 0,
                    text_body: row.get(16)?,
                    html_body: row.get(17)?,
                    raw_size: row.get(18)?,
                    created_at: row
                        .get::<_, String>(19)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    updated_at: row
                        .get::<_, String>(20)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            })
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        // Rebuild conversations within the transaction
        let mut count = 0;
        for message in &messages {
            if self
                .assign_to_conversation_with_tx(&tx, account_id, user_email, message)
                .is_ok()
            {
                count += 1;
            }
        }

        // Clean up empty conversations within the transaction
        tx.execute(
            "DELETE FROM conversations WHERE account_id = ?1 AND id NOT IN (
                SELECT DISTINCT conversation_id FROM conversation_messages
            )",
            rusqlite::params![account_id],
        )
        .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Commit the transaction
        tx.commit()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(count)
    }

    /// Assign a message to a conversation within an existing transaction
    fn assign_to_conversation_with_tx(
        &self,
        tx: &rusqlite::Transaction,
        account_id: &str,
        user_email: &str,
        message: &CachedMessage,
    ) -> Result<i64, EddieError> {
        // Parse addresses
        let to_addresses: Vec<String> =
            serde_json::from_str(&message.to_addresses).unwrap_or_default();
        let cc_addresses: Option<Vec<String>> = message
            .cc_addresses
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
            return Err(EddieError::Backend(
                "Message has no participants other than user".to_string(),
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
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Check if message is from user (outgoing)
        let user_normalized = Participant::normalize_email(user_email);
        let from_participant = Participant::from_address(&message.from_address);
        let is_outgoing = Participant::normalize_email(&from_participant.email) == user_normalized;

        // Parse flags to check for \Seen
        let flags: Vec<String> = serde_json::from_str(&message.flags).unwrap_or_default();
        let is_seen = flags
            .iter()
            .any(|f| f.to_lowercase() == "\\seen" || f.to_lowercase() == "seen");

        // Get or create conversation using the transaction
        let existing: Option<CachedConversation> = {
            let mut stmt = tx.prepare(
                "SELECT id, account_id, participant_key, participants, last_message_date, last_message_preview,
                        last_message_from, message_count, unread_count, is_outgoing, created_at, updated_at
                 FROM conversations WHERE account_id = ?1 AND participant_key = ?2"
            ).map_err(|e| EddieError::Backend(e.to_string()))?;

            stmt.query_row(rusqlite::params![account_id, participant_key], |row| {
                Ok(CachedConversation {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    participant_key: row.get(2)?,
                    participants: row.get(3)?,
                    last_message_date: row
                        .get::<_, Option<String>>(4)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    last_message_preview: row.get(5)?,
                    last_message_from: row.get(6)?,
                    message_count: row.get(7)?,
                    unread_count: row.get(8)?,
                    is_outgoing: row.get::<_, i32>(9)? != 0,
                    created_at: row
                        .get::<_, String>(10)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    updated_at: row
                        .get::<_, String>(11)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            })
            .optional()
            .map_err(|e: rusqlite::Error| EddieError::Backend(e.to_string()))?
        };

        let conversation = if let Some(mut conv) = existing {
            // Update existing conversation
            let should_update_last = match (&message.date, &conv.last_message_date) {
                (Some(msg_date), Some(conv_date)) => msg_date >= conv_date,
                (Some(_), None) => true,
                _ => false,
            };

            if should_update_last {
                conv.last_message_date = message.date;
                conv.last_message_preview = message
                    .text_body
                    .as_ref()
                    .or(message.subject.as_ref())
                    .map(|s| {
                        let trimmed = s.trim();
                        if trimmed.len() > 100 {
                            format!("{}...", &trimmed.chars().take(100).collect::<String>())
                        } else {
                            trimmed.to_string()
                        }
                    });
                conv.last_message_from = message
                    .from_name
                    .clone()
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
                last_message_preview: message.text_body.as_ref().or(message.subject.as_ref()).map(
                    |s| {
                        let trimmed = s.trim();
                        if trimmed.len() > 100 {
                            format!("{}...", &trimmed.chars().take(100).collect::<String>())
                        } else {
                            trimmed.to_string()
                        }
                    },
                ),
                last_message_from: message
                    .from_name
                    .clone()
                    .or_else(|| Some(from_participant.email.clone())),
                message_count: 1,
                unread_count: if !is_seen && !is_outgoing { 1 } else { 0 },
                is_outgoing,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        };

        // Save conversation using the transaction
        tx.execute(
            "INSERT INTO conversations (account_id, participant_key, participants, last_message_date,
                last_message_preview, last_message_from, message_count, unread_count, is_outgoing, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
             ON CONFLICT(account_id, participant_key) DO UPDATE SET
                participants = excluded.participants,
                last_message_date = excluded.last_message_date,
                last_message_preview = excluded.last_message_preview,
                last_message_from = excluded.last_message_from,
                message_count = excluded.message_count,
                unread_count = excluded.unread_count,
                is_outgoing = excluded.is_outgoing,
                updated_at = datetime('now')",
            rusqlite::params![
                conversation.account_id,
                conversation.participant_key,
                conversation.participants,
                conversation.last_message_date.map(|dt: DateTime<Utc>| dt.to_rfc3339()),
                conversation.last_message_preview,
                conversation.last_message_from,
                conversation.message_count,
                conversation.unread_count,
                conversation.is_outgoing as i32,
            ],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        let conv_id: i64 = tx
            .query_row(
                "SELECT id FROM conversations WHERE account_id = ?1 AND participant_key = ?2",
                rusqlite::params![account_id, participant_key],
                |row| row.get(0),
            )
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Link message to conversation using the transaction
        tx.execute(
            "INSERT OR IGNORE INTO conversation_messages (conversation_id, message_id) VALUES (?1, ?2)",
            rusqlite::params![conv_id, message.id],
        ).map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(conv_id)
    }

    /// Update conversation when a message is deleted
    #[allow(dead_code)]
    pub fn handle_message_deleted(
        &self,
        account_id: &str,
        _message_id: i64,
    ) -> Result<(), EddieError> {
        // The conversation_messages table has ON DELETE CASCADE,
        // so the link is automatically removed.
        // We just need to clean up empty conversations.
        self.db.delete_empty_conversations(account_id)?;
        Ok(())
    }

    /// Update conversation when message flags change
    #[allow(dead_code)]
    pub fn update_conversation_for_flag_change(
        &self,
        account_id: &str,
        user_email: &str,
        message: &CachedMessage,
        old_flags: &[String],
        new_flags: &[String],
    ) -> Result<(), EddieError> {
        let was_seen = old_flags.iter().any(|f| f.to_lowercase() == "\\seen");
        let is_seen = new_flags.iter().any(|f| f.to_lowercase() == "\\seen");

        if was_seen == is_seen {
            return Ok(()); // No change
        }

        // Get the conversation for this message
        let to_addresses: Vec<String> =
            serde_json::from_str(&message.to_addresses).unwrap_or_default();
        let cc_addresses: Option<Vec<String>> = message
            .cc_addresses
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        let participant_key = Self::generate_participant_key(
            user_email,
            &message.from_address,
            &to_addresses,
            cc_addresses.as_deref(),
        );

        // Check if this is an incoming message
        let user_normalized = Participant::normalize_email(user_email);
        let from_participant = Participant::from_address(&message.from_address);
        let is_outgoing =
            Participant::normalize_email(&from_participant.email) == user_normalized;

        // Only adjust unread count for incoming messages
        if is_outgoing {
            return Ok(());
        }

        // Get the conversation ID
        if let Some(conv) = self
            .db
            .get_conversation_by_key(account_id, &participant_key)?
        {
            // Use atomic SQL update to adjust unread count
            // This avoids read-modify-write race conditions
            let delta = if is_seen && !was_seen {
                // Message was marked as read - decrement
                -1
            } else {
                // Message was marked as unread - increment
                1
            };

            self.db.adjust_conversation_unread_count(conv.id, delta)?;
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
