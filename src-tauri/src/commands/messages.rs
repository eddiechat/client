use serde::Serialize;

use crate::adapters::sqlite::{self, DbPool, messages::NewMessage, conversations::compute_conversation_id, entities::{upsert_entities, NewEntity}};
use crate::services::sync::helpers::message_builder::compute_participant_key;
use crate::services::sync::helpers::email_normalization::normalize_email;
use crate::error::EddieError;
use crate::services::logger;
use tokio::sync::mpsc;

#[derive(Debug, Serialize)]
pub struct SendResult {
    pub message_id: String,
    pub conversation_id: String,
}

#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    pool: tauri::State<'_, DbPool>,
    wake_tx: tauri::State<'_, mpsc::Sender<()>>,
    account_id: String,
    from_email: String,
    from_name: Option<String>,
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,
    body: String,
    in_reply_to: Option<String>,
    references: Vec<String>,
) -> Result<SendResult, EddieError> {
    let self_emails = sqlite::entities::get_self_emails(&pool, &account_id)?;

    // Compute conversation placement
    let participant_key = compute_participant_key(
        &from_email,
        &to,
        &cc,
        &self_emails,
    );
    let conversation_id = compute_conversation_id(&participant_key);

    let now = chrono::Utc::now().timestamp_millis();
    let db_id = uuid::Uuid::new_v4().to_string();
    let placeholder_message_id = format!("{}.eddie@local", uuid::Uuid::new_v4());

    // Insert optimistic message into local DB
    let to_json = serde_json::to_string(&to).unwrap_or_default();
    let cc_json = serde_json::to_string(&cc).unwrap_or_default();
    let refs_json = serde_json::to_string(&references).unwrap_or_default();

    let new_msg = NewMessage {
        account_id: account_id.clone(),
        message_id: placeholder_message_id.clone(),
        imap_uid: 0,
        imap_folder: "OUTBOX".to_string(),
        date: now,
        from_address: normalize_email(&from_email),
        from_name: from_name.clone(),
        to_addresses: to_json,
        cc_addresses: cc_json,
        bcc_addresses: "[]".to_string(),
        subject: Some(subject.clone()),
        body_text: Some(body.clone()),
        body_html: None,
        size_bytes: None,
        has_attachments: false,
        in_reply_to: in_reply_to.clone(),
        references_ids: refs_json,
        imap_flags: "[\"Seen\"]".to_string(),
        gmail_labels: "[]".to_string(),
        classification: Some("chat".to_string()),
        is_important: false,
        distilled_text: Some(body.clone()),
        processed_at: Some(now),
        participant_key,
        conversation_id: conversation_id.clone(),
    };

    sqlite::messages::insert_messages(&pool, &[new_msg])?;
    logger::debug(&format!("Optimistic message inserted: id={}", db_id));

    // Upsert recipients into entities table to expand trust network
    let self_normalized: std::collections::HashSet<String> = self_emails.iter()
        .map(|e| normalize_email(e))
        .collect();
    let recipient_entities: Vec<NewEntity> = to.iter().chain(cc.iter())
        .map(|e| normalize_email(e))
        .filter(|e| !self_normalized.contains(e))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|email| NewEntity {
            account_id: account_id.clone(),
            email,
            display_name: None,
            trust_level: "connection".to_string(),
            source: Some("compose".to_string()),
            first_seen: now,
            last_seen: Some(now),
            sent_count: Some(1),
            metadata: None,
        })
        .collect();
    if !recipient_entities.is_empty() {
        let count = upsert_entities(&pool, &recipient_entities)?;
        logger::debug(&format!("Upserted {} recipient entities from compose", count));
    }

    // Queue the send action
    let payload = serde_json::json!({
        "from": from_email,
        "from_name": from_name,
        "to": to,
        "cc": cc,
        "subject": subject,
        "body": body,
        "in_reply_to": in_reply_to,
        "references": references,
        "message_db_id": db_id,
        "placeholder_message_id": placeholder_message_id,
    });

    sqlite::action_queue::enqueue(
        &pool,
        &account_id,
        "send",
        &payload.to_string(),
    )?;

    // Rebuild conversations so the new message shows up immediately
    crate::services::sync::worker::process_changes(&app, &pool, &account_id)?;

    // Wake worker to replay the send action
    let _ = wake_tx.send(()).await;

    Ok(SendResult {
        message_id: db_id,
        conversation_id,
    })
}
