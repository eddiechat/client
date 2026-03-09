use rusqlite::params;
use uuid::Uuid;
use crate::services::logger;

use super::DbPool;
use super::entities;
use crate::error::EddieError;
use crate::services::sync::helpers::email_normalization::normalize_email;

pub fn is_sent(gmail_labels: &str, imap_folder: &str, from_address: &str, self_emails: &[String]) -> bool {
    // Gmail: check labels
    if gmail_labels.contains("Sent") {
        return true;
    }
    // Non-Gmail: check if fetched from a Sent folder
    let folder_lower = imap_folder.to_lowercase();
    if folder_lower.contains("sent") {
        return true;
    }
    // Fallback: check if from_address matches a self email (normalize to handle Gmail dots)
    let normalized_from = normalize_email(from_address);
    self_emails.iter().any(|e| normalize_email(e) == normalized_from)
}

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
    pub gmail_labels: String,      // JSON
    pub classification: Option<String>,
    pub is_important: bool,
    pub distilled_text: Option<String>,
    pub processed_at: Option<i64>,
    pub participant_key: String,
    pub conversation_id: String,
    pub classification_headers: String, // JSON map of RFC headers for classification
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
                gmail_labels, fetched_at, classification, is_important, distilled_text,
                processed_at, participant_key, conversation_id, classification_headers
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20,
                ?21, ?22, ?23, ?24,
                ?25, ?26, ?27, ?28
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
                msg.gmail_labels,
                now,
                msg.classification,
                msg.is_important as i32,
                msg.distilled_text,
                msg.processed_at,
                msg.participant_key,
                msg.conversation_id,
                msg.classification_headers,
            ],
        );

        match result {
            Ok(_) => count += 1,
            Err(e) => logger::warn(&format!("Failed to insert message {}: {}", msg.message_id, e)),
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
    pub body_html: Option<String>,
    pub to_addresses: String,
    pub cc_addresses: String,
    pub bcc_addresses: String,
    pub imap_folder: String,
    pub gmail_labels: String,
    pub has_attachments: bool,
}

pub fn get_unprocessed_messages(pool: &DbPool, account_id: &str) -> Result<Vec<UnprocessedMessage>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, from_address, subject, in_reply_to, references_ids,
                    body_text, body_html, to_addresses, cc_addresses, bcc_addresses,
                    imap_folder, gmail_labels, has_attachments
             FROM messages WHERE account_id = ?1 AND processed_at IS NULL"
        )?;

    let rows = stmt
        .query_map(params![account_id], |row| {
            Ok(UnprocessedMessage {
                id: row.get(0)?,
                from_address: row.get(1)?,
                subject: row.get(2)?,
                in_reply_to: row.get(3)?,
                references_ids: row.get::<_, String>(4).unwrap_or_else(|_| "[]".to_string()),
                body_text: row.get(5)?,
                body_html: row.get(6)?,
                to_addresses: row.get::<_, String>(7).unwrap_or_else(|_| "[]".to_string()),
                cc_addresses: row.get::<_, String>(8).unwrap_or_else(|_| "[]".to_string()),
                bcc_addresses: row.get::<_, String>(9).unwrap_or_else(|_| "[]".to_string()),
                imap_folder: row.get::<_, String>(10).unwrap_or_default(),
                gmail_labels: row.get::<_, String>(11).unwrap_or_else(|_| "[]".to_string()),
                has_attachments: row.get::<_, i32>(12).unwrap_or(0) != 0,
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
    source: &str,
    confidence: f32,
    reason: &str,
    is_important: bool,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE messages SET classification = ?1, classification_source = ?2, \
         classification_confidence = ?3, classification_reason = ?4, \
         is_important = ?5, processed_at = ?6 WHERE id = ?7",
        params![classification, source, confidence as f64, reason, is_important as i32, now, message_id],
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

/// Returns (to_addresses, cc_addresses) for new messages sent by the user.
/// "New" = processed_at IS NULL (not yet classified).
/// Used to incrementally update the trust network as new sent messages arrive.
pub fn get_new_sent_recipients(pool: &DbPool, account_id: &str) -> Result<Vec<(String, String)>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT m.to_addresses, m.cc_addresses
         FROM messages m
         WHERE m.account_id = ?1
           AND m.processed_at IS NULL
           AND m.from_address IN (
               SELECT email FROM entities
               WHERE account_id = ?1 AND trust_level IN ('user', 'alias')
           )"
    )?;

    let rows = stmt.query_map(params![account_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
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
    pub message_id: String,
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
    pub imap_uid: u32,
    pub imap_folder: String,
    pub in_reply_to: Option<String>,
    pub references_ids: String,
    pub distilled_text: Option<String>,
    pub is_sent: bool,
}

const MESSAGE_COLUMNS: &str =
    "id, date, from_address, from_name, to_addresses, cc_addresses,
     subject, body_text, body_html, has_attachments, imap_flags, distilled_text,
     gmail_labels, imap_folder, message_id, imap_uid, in_reply_to, references_ids";

fn map_message_row(row: &rusqlite::Row) -> rusqlite::Result<(Message, String, String, String)> {
    let gmail_labels: String = row.get(12)?;
    let imap_folder: String = row.get(13)?;
    let from_address: String = row.get(2)?;
    Ok((Message {
        id: row.get(0)?,
        message_id: row.get(14)?,
        date: row.get(1)?,
        from_address: from_address.clone(),
        from_name: row.get(3)?,
        to_addresses: row.get(4)?,
        cc_addresses: row.get(5)?,
        subject: row.get(6)?,
        body_text: row.get(7)?,
        body_html: row.get(8)?,
        has_attachments: row.get::<_, i32>(9)? != 0,
        imap_flags: row.get(10)?,
        imap_uid: row.get(15)?,
        imap_folder: imap_folder.clone(),
        in_reply_to: row.get(16)?,
        references_ids: row.get(17)?,
        distilled_text: row.get(11)?,
        is_sent: false, // computed by caller
    }, gmail_labels, imap_folder, from_address))
}

fn collect_messages(
    rows: rusqlite::Rows,
    self_emails: &[String],
) -> Result<Vec<Message>, EddieError> {
    let mut messages = Vec::new();
    let mapped: Vec<_> = rows.mapped(map_message_row).collect();
    for row in mapped {
        let (mut msg, labels, folder, from) = row.map_err(|e| EddieError::Database(e.to_string()))?;
        msg.is_sent = is_sent(&labels, &folder, &from, self_emails);
        messages.push(msg);
    }
    Ok(messages)
}

pub fn fetch_conversation_messages(
    pool: &DbPool,
    account_id: &str,
    conversation_id: &str,
) -> Result<Vec<Message>, EddieError> {
    let self_emails = entities::get_self_emails(pool, account_id)?;
    let conn = pool.get()?;
    let query = format!(
        "SELECT {} FROM messages WHERE account_id = ?1 AND conversation_id = ?2 ORDER BY date ASC",
        MESSAGE_COLUMNS
    );
    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query(params![account_id, conversation_id])?;
    collect_messages(rows, &self_emails)
}

pub fn fetch_skill_match_messages(
    pool: &DbPool,
    account_id: &str,
    skill_id: &str,
) -> Result<Vec<Message>, EddieError> {
    let self_emails = entities::get_self_emails(pool, account_id)?;
    let conn = pool.get()?;

    let cols = MESSAGE_COLUMNS.replace(|c: char| c == '\n', " ");
    let prefixed = cols.split(", ").map(|c| format!("m.{}", c.trim())).collect::<Vec<_>>().join(", ");
    let query = format!(
        "SELECT {} FROM messages m JOIN skill_matches sm ON sm.message_id = m.id WHERE sm.skill_id = ?1 AND m.account_id = ?2 ORDER BY m.date DESC",
        prefixed
    );
    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query(params![skill_id, account_id])?;
    collect_messages(rows, &self_emails)
}

pub fn fetch_thread_messages(
    pool: &DbPool,
    account_id: &str,
    thread_id: &str,
) -> Result<Vec<Message>, EddieError> {
    let self_emails = entities::get_self_emails(pool, account_id)?;
    let conn = pool.get()?;
    let query = format!(
        "SELECT {} FROM messages WHERE account_id = ?1 AND thread_id = ?2 ORDER BY date ASC",
        MESSAGE_COLUMNS
    );
    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query(params![account_id, thread_id])?;
    collect_messages(rows, &self_emails)
}

pub fn fetch_recent_messages(
    pool: &DbPool,
    account_id: &str,
    limit: u32,
) -> Result<Vec<Message>, EddieError> {
    let self_emails = entities::get_self_emails(pool, account_id)?;
    let conn = pool.get()?;
    let query = format!(
        "SELECT {} FROM messages WHERE account_id = ?1 AND body_text IS NOT NULL ORDER BY date DESC LIMIT ?2",
        MESSAGE_COLUMNS
    );
    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query(params![account_id, limit])?;
    collect_messages(rows, &self_emails)
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

/// Returns (imap_uid, imap_flags) for all messages in a folder, ordered by UID DESC (latest first).
pub fn get_uids_and_flags_for_folder(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
) -> Result<Vec<(u32, String)>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT imap_uid, imap_flags FROM messages
         WHERE account_id = ?1 AND imap_folder = ?2
         ORDER BY imap_uid DESC"
    )?;

    let rows = stmt.query_map(params![account_id, folder], |row| {
        Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Batch-update imap_flags for messages identified by (uid, new_flags_json).
/// Returns the number of rows actually updated.
pub fn update_flags_batch(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
    updates: &[(u32, String)],
) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;
    let mut count = 0;

    for (uid, flags) in updates {
        let rows = tx.execute(
            "UPDATE messages SET imap_flags = ?1
             WHERE account_id = ?2 AND imap_folder = ?3 AND imap_uid = ?4",
            params![flags, account_id, folder, *uid as i64],
        )?;
        count += rows;
    }

    tx.commit()?;
    Ok(count)
}

/// Batch-update imap_flags and gmail_labels for messages.
/// Each update is (uid, new_flags_json, new_labels_json).
pub fn update_flags_and_labels_batch(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
    updates: &[(u32, String, String)],
) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;
    let mut count = 0;

    for (uid, flags, labels) in updates {
        let rows = tx.execute(
            "UPDATE messages SET imap_flags = ?1, gmail_labels = ?2
             WHERE account_id = ?3 AND imap_folder = ?4 AND imap_uid = ?5",
            params![flags, labels, account_id, folder, *uid as i64],
        )?;
        count += rows;
    }

    tx.commit()?;
    Ok(count)
}

pub struct MessageImapInfo {
    pub account_id: String,
    pub imap_uid: u32,
    pub imap_folder: String,
    pub body_html: Option<String>,
}

pub fn get_message_imap_info(pool: &DbPool, message_id: &str) -> Result<MessageImapInfo, EddieError> {
    let conn = pool.get()?;
    conn.query_row(
        "SELECT account_id, imap_uid, imap_folder, body_html FROM messages WHERE id = ?1",
        params![message_id],
        |row| {
            Ok(MessageImapInfo {
                account_id: row.get(0)?,
                imap_uid: row.get(1)?,
                imap_folder: row.get(2)?,
                body_html: row.get(3)?,
            })
        },
    ).map_err(|e| EddieError::Database(format!("Message not found: {}", e)))
}

pub fn update_body_html_by_id(pool: &DbPool, message_id: &str, html: &str) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE messages SET body_html = ?1 WHERE id = ?2",
        params![html, message_id],
    )?;
    Ok(())
}

pub fn update_body_html_by_uid(
    pool: &DbPool,
    account_id: &str,
    uid: u32,
    html: &str,
) -> Result<(), EddieError> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE messages SET body_html = ?1 WHERE account_id = ?2 AND imap_uid = ?3",
        params![html, account_id, uid as i64],
    )?;
    Ok(())
}

pub fn count_messages(pool: &DbPool, account_id: &str) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE account_id = ?1",
        params![account_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// Returns (imap_uid, imap_flags, gmail_labels) for all messages in a folder.
pub fn get_uids_flags_and_labels_for_folder(
    pool: &DbPool,
    account_id: &str,
    folder: &str,
) -> Result<Vec<(u32, String, String)>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT imap_uid, imap_flags, gmail_labels FROM messages
         WHERE account_id = ?1 AND imap_folder = ?2
         ORDER BY imap_uid DESC"
    )?;

    let rows = stmt.query_map(params![account_id, folder], |row| {
        Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Delete a message by its message_id (RFC 5322 Message-ID, not DB id).
/// Used to remove optimistic OUTBOX placeholders after successful send.
pub fn delete_message_by_message_id(pool: &DbPool, message_id: &str) -> Result<bool, EddieError> {
    let conn = pool.get()?;
    let rows = conn.execute(
        "DELETE FROM messages WHERE message_id = ?1",
        params![message_id],
    )?;
    Ok(rows > 0)
}

/// Optimistically mark messages as seen by adding \\Seen to their imap_flags JSON array.
pub fn mark_messages_seen(pool: &DbPool, message_ids: &[String]) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;
    let mut count = 0;

    for id in message_ids {
        let current_flags: String = match tx.query_row(
            "SELECT imap_flags FROM messages WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut flags: Vec<String> = serde_json::from_str(&current_flags).unwrap_or_default();
        if !flags.iter().any(|f| f == "Seen") {
            flags.push("Seen".to_string());
            let new_flags = serde_json::to_string(&flags).unwrap_or_default();
            let rows = tx.execute(
                "UPDATE messages SET imap_flags = ?1 WHERE id = ?2",
                params![new_flags, id],
            )?;
            count += rows;
        }
    }

    tx.commit()?;
    Ok(count)
}