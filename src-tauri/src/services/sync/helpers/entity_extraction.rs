use std::collections::HashMap;
use crate::adapters::sqlite::{DbPool, entities, messages};
use crate::services::sync::helpers::email_normalization::normalize_email;
use crate::adapters::sqlite::entities::NewEntity;
use crate::error::EddieError;
use crate::services::logger;

/// Extracts connections from newly arrived sent messages.
///
/// Queries messages where `processed_at IS NULL` and the sender is the user,
/// parses recipient lists (to/cc), and upserts them as connection entities.
/// Must run BEFORE classify_messages() since that sets processed_at.
pub fn extract_entities_from_new_messages(pool: &DbPool, account_id: &str) -> Result<usize, EddieError> {
    let sent_rows = messages::get_new_sent_recipients(pool, account_id)?;
    if sent_rows.is_empty() {
        return Ok(0);
    }

    let self_emails = entities::get_self_emails(pool, account_id)?;
    let self_normalized: Vec<String> = self_emails.iter().map(|e| normalize_email(e)).collect();

    let mut counts: HashMap<String, usize> = HashMap::new();

    for (to_json, cc_json) in &sent_rows {
        for json in [to_json, cc_json] {
            if let Ok(addrs) = serde_json::from_str::<Vec<String>>(json) {
                for addr in addrs {
                    let normalized = normalize_email(&addr);
                    if !normalized.is_empty() && !self_normalized.contains(&normalized) {
                        *counts.entry(normalized).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    if counts.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now().timestamp_millis();
    let new_entities: Vec<NewEntity> = counts
        .iter()
        .map(|(email, count)| NewEntity {
            account_id: account_id.to_string(),
            email: email.clone(),
            display_name: None,
            trust_level: "connection".to_string(),
            source: Some("sent_scan".to_string()),
            first_seen: now,
            last_seen: Some(now),
            sent_count: Some(*count as i32),
            metadata: None,
        })
        .collect();

    let upserted = entities::upsert_entities(pool, &new_entities)?;
    logger::debug(&format!(
        "Entity extraction: {} new sent messages → {} connections upserted",
        sent_rows.len(), upserted
    ));
    Ok(upserted)
}
