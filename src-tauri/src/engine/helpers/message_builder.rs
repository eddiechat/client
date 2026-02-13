use crate::adapters::sqlite::conversations::compute_conversation_id;
use crate::adapters::sqlite::messages::NewMessage;
use crate::adapters::imap::envelopes::Envelope;
use crate::engine::helpers::email_normalization::normalize_email;
use chrono::DateTime;

pub fn prepare_messages(
    account_id: &str,
    folder: &str,
    envelopes: &[Envelope],
    self_emails: &[String],
) -> Vec<NewMessage> {
    envelopes
        .iter()
        .map(|e| envelope_to_new_message(e, account_id, folder, self_emails))
        .collect()
}

pub fn compute_participant_key(
    from: &str,
    to: &[String],
    cc: &[String],
    self_emails: &[String],
) -> String {
    let mut all_participants: Vec<String> = Vec::new();

    all_participants.push(normalize_email(from));
    for addr in to {
        all_participants.push(normalize_email(addr));
    }
    for addr in cc {
        all_participants.push(normalize_email(addr));
    }

    let self_normalized: Vec<String> = self_emails.iter().map(|e| normalize_email(e)).collect();
    all_participants.retain(|a| !self_normalized.contains(a));
    all_participants.sort();
    all_participants.dedup();

    if all_participants.is_empty() {
        "__self__".to_string()
    } else {
        all_participants.join("\n")
    }
}

fn parse_date(date_str: &str) -> i64 {
    DateTime::parse_from_rfc2822(date_str)
        .or_else(|_| DateTime::parse_from_str(date_str, "%d-%b-%Y %H:%M:%S %z"))
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

fn envelope_to_new_message(
    envelope: &Envelope,
    account_id: &str,
    folder: &str,
    self_emails: &[String],
) -> NewMessage {
    let participant_key = compute_participant_key(
        &envelope.from_address,
        &envelope.to_addresses,
        &envelope.cc_addresses,
        self_emails,
    );
    let conversation_id = compute_conversation_id(&participant_key);

    let mut to_sorted = envelope.to_addresses.clone();
    to_sorted.sort();
    let mut cc_sorted = envelope.cc_addresses.clone();
    cc_sorted.sort();
    let mut flags_sorted = envelope.imap_flags.clone();
    flags_sorted.sort();

    NewMessage {
        account_id: account_id.to_string(),
        message_id: envelope.message_id.clone(),
        imap_uid: envelope.uid,
        imap_folder: folder.to_string(),
        date: parse_date(&envelope.date),
        from_address: normalize_email(&envelope.from_address),
        from_name: envelope.from_name.clone(),
        to_addresses: serde_json::to_string(&to_sorted).unwrap_or_default(),
        cc_addresses: serde_json::to_string(&cc_sorted).unwrap_or_default(),
        bcc_addresses: "[]".to_string(),
        subject: Some(envelope.subject.clone()),
        body_text: None,
        body_html: None,
        size_bytes: None,
        has_attachments: envelope.has_attachments,
        in_reply_to: envelope.in_reply_to.clone(),
        references_ids: serde_json::to_string(&envelope.references).unwrap_or_else(|_| "[]".to_string()),
        imap_flags: serde_json::to_string(&flags_sorted).unwrap_or_default(),
        classification: None,
        is_important: false,
        distilled_text: None,
        processed_at: None,
        participant_key,
        conversation_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_participant_key_basic() {
        let key = compute_participant_key(
            "alice@example.com",
            &["brian@gmail.com".to_string()],
            &[],
            &["brian@gmail.com".to_string()],
        );
        assert_eq!(key, "alice@example.com");
    }

    #[test]
    fn test_participant_key_multiple() {
        let key = compute_participant_key(
            "charlie@example.com",
            &["brian@gmail.com".to_string(), "alice@example.com".to_string()],
            &[],
            &["brian@gmail.com".to_string()],
        );
        assert_eq!(key, "alice@example.com\ncharlie@example.com");
    }

    #[test]
    fn test_participant_key_self_message() {
        let key = compute_participant_key(
            "brian@gmail.com",
            &["brian@gmail.com".to_string()],
            &[],
            &["brian@gmail.com".to_string()],
        );
        assert_eq!(key, "__self__");
    }

    #[test]
    fn test_participant_key_normalizes() {
        let key = compute_participant_key(
            "Alice@Example.COM",
            &["Brian@Gmail.com".to_string()],
            &[],
            &["brian@gmail.com".to_string()],
        );
        assert_eq!(key, "alice@example.com");
    }

    #[test]
    fn test_conversation_id_deterministic() {
        let id1 = compute_conversation_id("alice@example.com");
        let id2 = compute_conversation_id("alice@example.com");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_conversation_id_different_keys() {
        let id1 = compute_conversation_id("alice@example.com");
        let id2 = compute_conversation_id("bob@example.com");
        assert_ne!(id1, id2);
    }
}