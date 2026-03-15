use crate::adapters::sqlite::{self, DbPool, action_queue};
use crate::adapters::sqlite::accounts;
use crate::adapters::imap::{connection, folders};
use crate::adapters::smtp;
use crate::error::EddieError;
use crate::services::logger;

/// Replay all pending actions for all onboarded accounts.
/// Called at the start of each worker tick, before incremental sync.
pub async fn replay_pending_actions(
    pool: &DbPool,
) -> Result<(), EddieError> {
    let account_ids = accounts::list_onboarded_account_ids(pool)?;

    for account_id in &account_ids {
        let actions = action_queue::get_pending(pool, account_id)?;
        if actions.is_empty() {
            continue;
        }

        logger::debug(&format!(
            "Replaying {} pending actions for account {}",
            actions.len(),
            account_id
        ));

        // Check write_mode before connecting — skip IMAP-mutating actions unless write_mode
        let write_mode = sqlite::settings::get_setting(pool, "write_mode")?
            .map(|v| v == "true")
            .unwrap_or(false);

        // We connect to IMAP if there are actions that need it
        let needs_imap = actions.iter().any(|a| a.action_type == "mark_read" || a.action_type == "send");
        let mut imap_conn = if needs_imap && (write_mode || actions.iter().any(|a| a.action_type == "send")) {
            let creds = accounts::get_credentials(pool, account_id)?
                .ok_or(EddieError::AccountNotFound(account_id.clone()))?;
            Some(
                connection::connect_with_tls(
                    &creds.host, creds.port, creds.tls,
                    &creds.email, &creds.password, true, // write_mode: SELECT for mutations
                ).await?
            )
        } else {
            None
        };

        for action in &actions {
            action_queue::mark_in_progress(pool, &action.id)?;

            let result = execute_action(pool, imap_conn.as_mut(), action, write_mode).await;

            match result {
                Ok(()) => {
                    action_queue::mark_completed(pool, &action.id)?;
                    logger::debug(&format!("Action {} completed: {}", action.id, action.action_type));
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    action_queue::mark_failed(pool, &action.id, &err_msg)?;
                    logger::warn(&format!(
                        "Action {} failed (retry {}/{}): {}",
                        action.id, action.retry_count + 1, action.max_retries, err_msg
                    ));
                }
            }
        }

        // Logout IMAP if we connected
        if let Some(mut conn) = imap_conn {
            conn.session.logout().await.ok();
        }
    }

    Ok(())
}

async fn execute_action(
    pool: &DbPool,
    imap_conn: Option<&mut connection::ImapConnection>,
    action: &action_queue::QueuedAction,
    write_mode: bool,
) -> Result<(), EddieError> {
    match action.action_type.as_str() {
        "mark_read" => execute_mark_read(pool, imap_conn, action, write_mode).await,
        "send" => execute_send(pool, imap_conn, action).await,
        _ => Err(EddieError::InvalidInput(format!("Unknown action type: {}", action.action_type))),
    }
}

/// Mark messages as read on IMAP server.
/// Payload: { "folder": "INBOX", "uids": [123, 456] }
async fn execute_mark_read(
    _pool: &DbPool,
    imap_conn: Option<&mut connection::ImapConnection>,
    action: &action_queue::QueuedAction,
    write_mode: bool,
) -> Result<(), EddieError> {
    if !write_mode {
        return Err(EddieError::Backend("Read-only mode: skipping IMAP STORE".into()));
    }

    let conn = imap_conn.ok_or(EddieError::Backend("No IMAP connection for mark_read".into()))?;

    let payload: serde_json::Value = serde_json::from_str(&action.payload)
        .map_err(|e| EddieError::InvalidInput(format!("Invalid mark_read payload: {}", e)))?;

    let folder = payload["folder"]
        .as_str()
        .ok_or(EddieError::InvalidInput("mark_read: missing folder".into()))?;

    let uids: Vec<u32> = payload["uids"]
        .as_array()
        .ok_or(EddieError::InvalidInput("mark_read: missing uids".into()))?
        .iter()
        .filter_map(|v| v.as_u64().map(|u| u as u32))
        .collect();

    if uids.is_empty() {
        return Ok(());
    }

    conn.select_folder(folder).await?;
    conn.store_flags(&uids, "+FLAGS (\\Seen)").await?;

    Ok(())
}

/// Send an email via SMTP, then APPEND to Sent folder.
/// Payload: { "from", "from_name", "to", "cc", "subject", "body", "in_reply_to", "references", "message_db_id", "message_id" }
async fn execute_send(
    pool: &DbPool,
    imap_conn: Option<&mut connection::ImapConnection>,
    action: &action_queue::QueuedAction,
) -> Result<(), EddieError> {
    let payload: serde_json::Value = serde_json::from_str(&action.payload)
        .map_err(|e| EddieError::InvalidInput(format!("Invalid send payload: {}", e)))?;

    let from = payload["from"].as_str()
        .ok_or(EddieError::InvalidInput("send: missing from".into()))?;
    let from_name = payload["from_name"].as_str().map(|s| s.to_string());
    let to: Vec<String> = payload["to"].as_array()
        .ok_or(EddieError::InvalidInput("send: missing to".into()))?
        .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
    let cc: Vec<String> = payload["cc"].as_array()
        .unwrap_or(&vec![])
        .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
    let subject = payload["subject"].as_str()
        .ok_or(EddieError::InvalidInput("send: missing subject".into()))?;
    let body = payload["body"].as_str()
        .ok_or(EddieError::InvalidInput("send: missing body".into()))?;
    let in_reply_to = payload["in_reply_to"].as_str().map(|s| s.to_string());
    let references: Vec<String> = payload["references"].as_array()
        .unwrap_or(&vec![])
        .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();

    // Get SMTP credentials
    let smtp_creds = accounts::get_smtp_credentials(pool, &action.account_id)?
        .ok_or(EddieError::AccountNotFound(action.account_id.clone()))?;

    let message_id = payload["message_id"].as_str().map(|s| s.to_string());

    let smtp_msg = smtp::SmtpMessage {
        from: from.to_string(),
        from_name,
        to,
        cc,
        subject: subject.to_string(),
        body: body.to_string(),
        in_reply_to,
        references,
        message_id,
    };

    // Send via SMTP
    let raw_message = smtp::send_message(
        &smtp_creds.host, smtp_creds.port, smtp_creds.tls,
        &smtp_creds.email, &smtp_creds.password,
        &smtp_msg,
    ).await?;

    logger::info(&format!("Email sent to {:?}", smtp_msg.to));

    // APPEND to Sent folder via IMAP
    // Gmail auto-copies sent messages, so skip APPEND for Gmail accounts.
    if let Some(conn) = imap_conn {
        if conn.has_gmail_ext {
            logger::debug("Gmail account — skipping APPEND (auto-copied to Sent)");
        } else {
            let folder_list = folders::list_folders(&mut conn.session).await?;
            if let Some(sent_folder) = folders::find_sent_folder(&folder_list) {
                conn.append_message(&sent_folder, &["\\Seen"], &raw_message).await?;
                logger::debug(&format!("Appended sent message to {}", sent_folder));
            } else {
                logger::warn("No Sent folder found — message not saved to IMAP");
            }
        }
    }

    Ok(())
}
