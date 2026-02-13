use rusqlite::params;
use uuid::Uuid;
use tracing::warn;

use super::DbPool;
use crate::types::error::EddieError;

/// Represents a message ready to be stored in the database.
/// This is decoupled from IMAP — any source can produce this.
pub struct NewMessage {
    pub account_id: String,
    pub message_id: String,
    pub imap_uid: u32,
    pub imap_folder: String,
    pub date: i64,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: String,      // JSON
    pub cc_addresses: String,      // JSON
    pub bcc_addresses: String,     // JSON
    pub subject: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub size_bytes: Option<u32>,
    pub has_attachments: bool,
    pub in_reply_to: Option<String>,
    pub references_ids: String,    // JSON
    pub imap_flags: String,        // JSON
    pub classification: Option<String>,
    pub is_important: bool,
    pub distilled_text: Option<String>,
    pub processed_at: Option<i64>,
    pub participant_key: String,
    pub conversation_id: String,
}

pub fn insert_messages(pool: &DbPool, messages: &[NewMessage]) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;

    let mut count = 0;

    for msg in messages {
        let now = chrono::Utc::now().timestamp_millis();
        let result = tx.execute(
            "INSERT OR IGNORE INTO messages (
                id, account_id, message_id, imap_uid, imap_folder,
                date, from_address, from_name, to_addresses, cc_addresses,
                bcc_addresses, subject, body_text, body_html, size_bytes,
                has_attachments, in_reply_to, references_ids, imap_flags,
                fetched_at, classification, is_important, distilled_text,
                processed_at, participant_key, conversation_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19,
                ?20, ?21, ?22, ?23,
                ?24, ?25, ?26
            )",
            params![
                Uuid::new_v4().to_string(),
                msg.account_id,
                msg.message_id,
                msg.imap_uid,
                msg.imap_folder,
                msg.date,
                msg.from_address,
                msg.from_name,
                msg.to_addresses,
                msg.cc_addresses,
                msg.bcc_addresses,
                msg.subject,
                msg.body_text,
                msg.body_html,
                msg.size_bytes.map(|s| s as i64),
                msg.has_attachments as i32,
                msg.in_reply_to,
                msg.references_ids,
                msg.imap_flags,
                now,
                msg.classification,
                msg.is_important as i32,
                msg.distilled_text,
                msg.processed_at,
                msg.participant_key,
                msg.conversation_id,
            ],
        );

        match result {
            Ok(_) => count += 1,
            Err(e) => warn!("Failed to insert message {}: {}", msg.message_id, e),
        }
    }

    tx.commit()?;
    Ok(count)
}

pub struct UnprocessedMessage {
    pub id: String,
    pub from_address: String,
    pub subject: Option<String>,
    pub in_reply_to: Option<String>,
    pub references_ids: String,
    pub body_text: Option<String>,
}

pub fn get_unprocessed_messages(pool: &DbPool, account_id: &str) -> Result<Vec<UnprocessedMessage>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, from_address, subject, in_reply_to, references_ids, body_text
             FROM messages WHERE account_id = ?1 AND processed_at IS NULL"
        )?;

    let rows = stmt
        .query_map(params![account_id], |row| {
            Ok(UnprocessedMessage {
                id: row.get(0)?,
                from_address: row.get(1)?,
                subject: row.get(2)?,
                in_reply_to: row.get(3)?,
                references_ids: row.get(4)?,
                body_text: row.get(5)?,
            })
        })?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

pub fn update_classification(
    pool: &DbPool,
    message_id: &str,
    classification: &str,
    is_important: bool,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE messages SET classification = ?1, is_important = ?2, processed_at = ?3 WHERE id = ?4",
        params![classification, is_important as i32, now, message_id],
    )?;
    Ok(())
}

pub fn update_body_by_uid(
    pool: &DbPool,
    account_id: &str,
    uid: u32,
    body_text: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE messages SET body_text = ?1 WHERE account_id = ?2 AND imap_uid = ?3",
        params![body_text, account_id, uid as i64],
    )?;
    Ok(())
}

pub struct UnExtractedMessage {
    pub id: String,
    pub body_text: String,
}

pub fn get_unextracted_messages(pool: &DbPool, account_id: &str) -> Result<Vec<UnExtractedMessage>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, body_text
                FROM messages WHERE account_id = ?1 AND distilled_text IS NULL AND body_text IS NOT NULL"
        )?;

    let rows = stmt
        .query_map(params![account_id], |row| {
            Ok(UnExtractedMessage {
                id: row.get(0)?,
                body_text: row.get(1)?,
            })
        })?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

pub fn update_extracted(
    pool: &DbPool,
    message_id: &str,
    extracted: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE messages SET distilled_text = ?1, processed_at = ?2 WHERE id = ?3",
        params![extracted, now, message_id],
    )?;
    Ok(())
}

pub fn reset_classifications(pool: &DbPool, account_id: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE messages SET processed_at = NULL, classification = NULL, distilled_text = NULL WHERE account_id = ?1",
        params![account_id],
    )?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct Message {
    pub id: String,
    pub date: i64,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: String,
    pub cc_addresses: String,
    pub subject: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub has_attachments: bool,
    pub imap_flags: String,
    pub distilled_text: Option<String>,
}

pub fn fetch_conversation_messages(
    pool: &DbPool,
    account_id: &str,
    conversation_id: &str,
) -> Result<Vec<Message>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, date, from_address, from_name, to_addresses, cc_addresses,
                subject, body_text, body_html, has_attachments, imap_flags, distilled_text
         FROM messages
         WHERE account_id = ?1 AND conversation_id = ?2
         ORDER BY date ASC",
    )?;

    let rows = stmt.query_map(params![account_id, conversation_id], |row| {
        Ok(Message {
            id: row.get(0)?,
            date: row.get(1)?,
            from_address: row.get(2)?,
            from_name: row.get(3)?,
            to_addresses: row.get(4)?,
            cc_addresses: row.get(5)?,
            subject: row.get(6)?,
            body_text: row.get(7)?,
            body_html: row.get(8)?,
            has_attachments: row.get::<_, i32>(9)? != 0,
            imap_flags: row.get(10)?,
            distilled_text: row.get(11)?,
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

pub fn get_uids_for_folder(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
) -> Result<std::collections::HashSet<u32>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT imap_uid FROM messages WHERE account_id = ?1 AND imap_folder = ?2"
    )?;

    let uids = stmt.query_map(params![account_id, folder], |row| {
        row.get::<_, u32>(0)
    })?
    .filter_map(|r| r.ok())
    .collect();

    Ok(uids)
}