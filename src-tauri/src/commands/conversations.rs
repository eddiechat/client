//! Conversation Tauri commands
//!
//! Commands for managing email conversations.
//! These are deprecated in favor of the sync engine.

use chrono::{Duration, Utc};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::backend;
use crate::types::conversation::{extract_name, normalize_email, Conversation};
use crate::types::{EddieError, Envelope, Message};

/// Envelope with its source folder for proper message ID tracking
struct EnvelopeWithFolder {
    envelope: Envelope,
    folder: String,
}

impl EnvelopeWithFolder {
    /// Create a folder-qualified message ID in the format "folder:id"
    fn qualified_id(&self) -> String {
        format!("{}:{}", self.folder, self.envelope.id)
    }
}

/// Build conversations from envelopes
fn build_conversations(envelopes: Vec<EnvelopeWithFolder>, user_email: &str) -> Vec<Conversation> {
    let user_email_normalized = normalize_email(user_email);
    let mut conv_map: HashMap<String, Conversation> = HashMap::new();

    let user_name_from_email = user_email.split('@').next().unwrap_or("me").to_string();
    let mut user_display_name = user_name_from_email.clone();

    for env_with_folder in envelopes {
        let envelope = &env_with_folder.envelope;
        let qualified_id = env_with_folder.qualified_id();

        let from_email = normalize_email(&envelope.from);
        let from_name = extract_name(&envelope.from);

        if from_email == user_email_normalized && !from_name.contains('@') {
            user_display_name = from_name.clone();
        }

        let user_is_sender = from_email == user_email_normalized;
        let user_is_recipient = envelope
            .to
            .iter()
            .any(|to| normalize_email(to) == user_email_normalized);
        let user_in_conversation = user_is_sender || user_is_recipient;

        let mut other_participants: Vec<String> = vec![];
        let mut other_names: Vec<String> = vec![];

        if from_email != user_email_normalized {
            other_participants.push(from_email.clone());
            other_names.push(from_name.clone());
        }

        for to in envelope.to.iter() {
            let to_email = normalize_email(to);
            let to_name = extract_name(to);
            if to_email != user_email_normalized && !other_participants.contains(&to_email) {
                other_participants.push(to_email);
                other_names.push(to_name);
            }
        }

        let mut participants: Vec<String> = vec![];
        let mut participant_names: Vec<String> = vec![];

        if user_in_conversation {
            participants.push(user_email_normalized.clone());
            participant_names.push(user_display_name.clone());
        }
        participants.extend(other_participants.clone());
        participant_names.extend(other_names.clone());

        let key = if other_participants.is_empty() {
            Conversation::participants_key(&[user_email_normalized.clone()])
        } else {
            Conversation::participants_key(&other_participants)
        };
        let is_outgoing = normalize_email(&envelope.from) == user_email_normalized;
        let is_unread = !envelope.flags.iter().any(|f| f.to_lowercase() == "seen");

        if let Some(conv) = conv_map.get_mut(&key) {
            conv.message_ids.push(qualified_id);
            if is_unread {
                conv.unread_count += 1;
            }
            if envelope.date > conv.last_message_date {
                conv.last_message_date = envelope.date.clone();
                conv.last_message_preview = envelope.subject.clone();
                conv.last_message_from = if is_outgoing {
                    "You".to_string()
                } else {
                    extract_name(&envelope.from)
                };
                conv.is_outgoing = is_outgoing;
            }
        } else {
            conv_map.insert(
                key.clone(),
                Conversation {
                    id: key,
                    participants: participants.clone(),
                    participant_names,
                    last_message_date: envelope.date.clone(),
                    last_message_preview: envelope.subject.clone(),
                    last_message_from: if is_outgoing {
                        "You".to_string()
                    } else {
                        extract_name(&envelope.from)
                    },
                    unread_count: if is_unread { 1 } else { 0 },
                    message_ids: vec![qualified_id],
                    is_outgoing,
                    user_name: user_display_name.clone(),
                    user_in_conversation,
                },
            );
        }
    }

    for conv in conv_map.values_mut() {
        conv.user_name = user_display_name.clone();
        if conv.user_in_conversation && !conv.participant_names.is_empty() {
            conv.participant_names[0] = user_display_name.clone();
        }
    }

    let mut conversations: Vec<Conversation> = conv_map.into_values().collect();
    conversations.sort_by(|a, b| b.last_message_date.cmp(&a.last_message_date));
    conversations
}

/// List conversations for an account
///
/// **DEPRECATED**: This command fetches directly from IMAP.
/// Use `get_cached_conversations` instead for better performance and offline support.
#[tauri::command]
pub async fn list_conversations(account: Option<String>) -> Result<Vec<Conversation>, EddieError> {
    warn!("DEPRECATED: list_conversations called - migrate to get_cached_conversations");
    info!("Listing conversations");

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let user_email = backend.get_email();

    let folders = backend.list_folders().await.unwrap_or_default();
    let mut folders_to_fetch = vec!["INBOX".to_string()];

    for folder in &folders {
        let name_lower = folder.name.to_lowercase();
        if name_lower.contains("sent")
            || name_lower.contains("envoy")
            || name_lower.contains("gesendet")
        {
            info!("Found sent folder: {}", folder.name);
            folders_to_fetch.push(folder.name.clone());
        }
    }

    info!("Will fetch from folders: {:?}", folders_to_fetch);

    let one_year_ago = Utc::now() - Duration::days(365);
    let mut all_envelopes: Vec<EnvelopeWithFolder> = Vec::new();

    for folder in &folders_to_fetch {
        match backend.list_envelopes(Some(folder), 0, 1000).await {
            Ok(envelopes) => {
                info!("Fetched {} envelopes from {}", envelopes.len(), folder);
                let recent: Vec<EnvelopeWithFolder> = envelopes
                    .into_iter()
                    .filter(|e| {
                        if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&e.date) {
                            date > one_year_ago
                        } else {
                            true
                        }
                    })
                    .map(|envelope| EnvelopeWithFolder {
                        envelope,
                        folder: folder.clone(),
                    })
                    .collect();
                all_envelopes.extend(recent);
            }
            Err(e) => {
                info!("Could not fetch from folder {}: {}", folder, e);
            }
        }
    }

    info!("Fetched {} envelopes total", all_envelopes.len());

    let conversations = build_conversations(all_envelopes, &user_email);
    Ok(conversations)
}

/// Parse a folder-qualified message ID in the format "folder:id"
fn parse_qualified_id(qualified_id: &str) -> Option<(String, String)> {
    if let Some(colon_pos) = qualified_id.find(':') {
        let folder = &qualified_id[..colon_pos];
        let id = &qualified_id[colon_pos + 1..];
        if !folder.is_empty() && !id.is_empty() {
            return Some((folder.to_string(), id.to_string()));
        }
    }
    None
}

/// Get messages for a conversation by message IDs
///
/// **DEPRECATED**: This command fetches directly from IMAP.
/// Use sync engine and read from SQLite cache for better performance and offline support.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_conversation_messages(
    account: Option<String>,
    messageIds: Vec<String>,
) -> Result<Vec<Message>, EddieError> {
    warn!("DEPRECATED: get_conversation_messages called - migrate to sync engine equivalent");
    info!("Getting {} conversation messages", messageIds.len());

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let mut messages = Vec::new();

    for qualified_id in &messageIds {
        if let Some((folder, msg_id)) = parse_qualified_id(qualified_id) {
            match backend.get_message(Some(&folder), &msg_id, true).await {
                Ok(msg) => messages.push(msg),
                Err(e) => {
                    info!(
                        "Could not fetch message {} from folder {}: {}",
                        msg_id, folder, e
                    );
                }
            }
        } else {
            // Fallback for legacy unqualified IDs
            info!(
                "Warning: unqualified message ID '{}', trying fallback folders",
                qualified_id
            );
            let folders = backend.list_folders().await.unwrap_or_default();
            let mut folders_to_try = vec!["INBOX".to_string()];
            for folder in &folders {
                let name_lower = folder.name.to_lowercase();
                if name_lower.contains("sent")
                    || name_lower.contains("envoy")
                    || name_lower.contains("gesendet")
                {
                    folders_to_try.push(folder.name.clone());
                }
            }
            for folder in &folders_to_try {
                match backend.get_message(Some(folder), qualified_id, true).await {
                    Ok(msg) => {
                        messages.push(msg);
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }
    }

    // Sort messages by date ascending (oldest first)
    messages.sort_by(|a, b| a.envelope.date.cmp(&b.envelope.date));

    Ok(messages)
}
