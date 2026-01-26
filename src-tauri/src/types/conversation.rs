use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub participants: Vec<String>,
    pub participant_names: Vec<String>,
    pub last_message_date: String,
    pub last_message_preview: String,
    pub last_message_from: String,
    pub unread_count: usize,
    pub message_ids: Vec<String>,
    pub is_outgoing: bool,
    pub user_name: String,
    pub user_in_conversation: bool,
}

impl Conversation {
    pub fn participants_key(participants: &[String]) -> String {
        let mut sorted: Vec<String> = participants
            .iter()
            .map(|p| normalize_email(p).to_lowercase())
            .collect();
        sorted.sort();
        sorted.dedup();
        sorted.join(",")
    }
}

pub fn normalize_email(addr: &str) -> String {
    // Extract email from "Name <email>" format
    if let Some(start) = addr.find('<') {
        if let Some(end) = addr.find('>') {
            return addr[start + 1..end].trim().to_lowercase();
        }
    }
    addr.trim().to_lowercase()
}

pub fn extract_name(addr: &str) -> String {
    // Extract name from "Name <email>" format
    if let Some(start) = addr.find('<') {
        let name = addr[..start].trim();
        if !name.is_empty() {
            return name.trim_matches('"').to_string();
        }
    }
    // If no name, use the part before @ as name
    let email = normalize_email(addr);
    email.split('@').next().unwrap_or(&email).to_string()
}
