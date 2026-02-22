use crate::adapters::sqlite::{onboarding_tasks, DbPool};
use crate::adapters::sqlite::entities::{upsert_entities, NewEntity};

use crate::adapters::imap::folders;
use crate::adapters::imap::sent_scan::fetch_sent_recipients;
use crate::adapters::imap::connection::ImapConnection;

use crate::services::sync::{helpers, worker};
use crate::services::sync::helpers::email_normalization::normalize_email;
use crate::error::EddieError;

use crate::services::logger;

/// Onboarding phase 2: Build trust network from sent folder
pub async fn run_trust_network(
    app: &tauri::AppHandle,
    pool: &DbPool,
    account_id: &str,
    task: &onboarding_tasks::Task,
) -> Result<(), EddieError> {
    let (creds, self_emails, mut conn) = worker::connect_account(pool, account_id).await?;

    logger::debug("Discovering folders...");
    let folder_list = folders::list_folders(&mut conn.session).await?;
    let sent_folder = folders::find_folder_by_attribute(&folder_list, "Sent")
        .ok_or(EddieError::Backend("No Sent folder found".into()))?;

    let mailbox = conn.select_folder(&sent_folder).await?;
    let server_count = mailbox.exists;
    helpers::status_emit::emit_status(app, "trust_network", &format!("Building trust network from {} sent messages...", server_count));

    logger::debug("Scanning sent folder for connections...");
    let trust_count = build_trust_network(
        &mut conn, pool, &account_id, &creds.email, &self_emails, &sent_folder,
    ).await?;
    logger::debug(&format!("Found {} connections", trust_count));

    worker::process_changes(app, pool, account_id)?;
    onboarding_tasks::mark_task_done(pool, account_id, &task.name)?;
    Ok(())
}

pub async fn build_trust_network(
    conn: &mut ImapConnection,
    pool: &DbPool,
    account_id: &str,
    user_email: &str,
    aliases: &[String],
    sent_folder: &str,
) -> Result<usize, EddieError> {
    let now = chrono::Utc::now().timestamp_millis();

    // Step 1: Insert the user themselves
    let mut entities = vec![NewEntity {
        account_id: account_id.to_string(),
        email: normalize_email(user_email),
        display_name: None,
        trust_level: "user".to_string(),
        source: Some("self".to_string()),
        first_seen: now,
        last_seen: Some(now),
        sent_count: None,
        metadata: None,
    }];

    // Step 2: Insert aliases
    for alias in aliases {
        entities.push(NewEntity {
            account_id: account_id.to_string(),
            email: normalize_email(alias),
            display_name: None,
            trust_level: "alias".to_string(),
            source: Some("self".to_string()),
            first_seen: now,
            last_seen: Some(now),
            sent_count: None,
            metadata: None,
        });
    }

    let start = std::time::Instant::now();
    // Step 3: Scan Sent folder for connections (with per-recipient counts)
    let recipient_counts = fetch_sent_recipients(conn, sent_folder, 500).await?;
    logger::debug(&format!("fetch_sent_recipients took: {:?}", start.elapsed()));

    let self_emails: Vec<String> = std::iter::once(normalize_email(user_email))
        .chain(aliases.iter().map(|a| normalize_email(a)))
        .collect();

    let start = std::time::Instant::now();
    for (email, count) in &recipient_counts {
        let normalized = normalize_email(email);
        if !self_emails.contains(&normalized) {
            entities.push(NewEntity {
                account_id: account_id.to_string(),
                email: normalized,
                display_name: None,
                trust_level: "connection".to_string(),
                source: Some("sent_scan".to_string()),
                first_seen: now,
                last_seen: None,
                sent_count: Some(*count as i32),
                metadata: None,
            });
        }
    }
    logger::debug(&format!("n * entities.push took: {:?}", start.elapsed()));

    upsert_entities(pool, &entities)
}