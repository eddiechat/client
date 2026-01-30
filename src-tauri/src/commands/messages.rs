//! Message Tauri commands
//!
//! Commands for reading, sending, and managing email messages.

use std::fs;
use std::path::PathBuf;
use tauri::State;
use tracing::{info, warn};

use crate::backend::{self, SendMessageResult};
use crate::services::{build_message, resolve_account_id, ComposeParams};
use crate::state::SyncManager;
use crate::types::responses::AttachmentInfo;
use crate::types::{ComposeAttachment, EddieError, Message, ReadMessageRequest};

/// Read a message by ID
///
/// **DEPRECATED**: Fetches directly from IMAP.
/// Use `fetch_message_body` from sync commands for cached access.
#[tauri::command]
pub async fn read_message(request: ReadMessageRequest) -> Result<Message, EddieError> {
    warn!("DEPRECATED: read_message called - migrate to fetch_message_body");
    info!("Reading message: {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .get_message(request.folder.as_deref(), &request.id, request.preview)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Delete messages
#[tauri::command]
pub async fn delete_messages(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    info!("Deleting messages: {:?}", ids);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .delete_messages(folder.as_deref(), &id_refs)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    // Update cache after successful server operation
    update_cache_after_delete(&sync_manager, account.as_deref(), folder.as_deref(), &ids).await;

    Ok(())
}

/// Update cache after deleting messages
async fn update_cache_after_delete(
    sync_manager: &SyncManager,
    account: Option<&str>,
    folder: Option<&str>,
    ids: &[String],
) {
    let account_id = match resolve_account_id(account) {
        Ok(id) => id,
        Err(_) => return,
    };
    let folder_name = folder.unwrap_or("INBOX");

    if let Some(engine) = sync_manager.get(&account_id).await {
        let db = engine.read().await.database();
        let uids: Vec<u32> = ids.iter().filter_map(|id| id.parse::<u32>().ok()).collect();

        if !uids.is_empty() {
            if let Err(e) = db.delete_messages_by_uids(&account_id, folder_name, &uids) {
                warn!("Failed to delete messages from cache: {}", e);
            }
        }
    }
}

/// Copy messages to another folder
#[tauri::command]
pub async fn copy_messages(
    account: Option<String>,
    source_folder: Option<String>,
    target_folder: String,
    ids: Vec<String>,
) -> Result<(), EddieError> {
    info!("Copying messages to {}: {:?}", target_folder, ids);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
    backend
        .copy_messages(source_folder.as_deref(), &target_folder, &id_refs)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Move messages to another folder
#[tauri::command]
pub async fn move_messages(
    account: Option<String>,
    source_folder: Option<String>,
    target_folder: String,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    info!("Moving messages to {}: {:?}", target_folder, ids);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .move_messages(source_folder.as_deref(), &target_folder, &id_refs)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    // Update cache - delete from source folder
    update_cache_after_delete(&sync_manager, account.as_deref(), source_folder.as_deref(), &ids)
        .await;

    Ok(())
}

/// Send a message via SMTP and save to Sent folder
#[tauri::command]
pub async fn send_message(
    account: Option<String>,
    message: String,
) -> Result<Option<SendMessageResult>, EddieError> {
    info!("Sending message, length: {}", message.len());

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .send_message(message.as_bytes())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Save a message to a folder (drafts)
#[tauri::command]
pub async fn save_message(
    account: Option<String>,
    folder: Option<String>,
    message: String,
) -> Result<String, EddieError> {
    info!("Saving message to folder: {:?}", folder);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .save_message(folder.as_deref(), message.as_bytes())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Send a message with attachments via SMTP and save to Sent folder
#[tauri::command]
pub async fn send_message_with_attachments(
    account: Option<String>,
    from: String,
    to: Vec<String>,
    cc: Option<Vec<String>>,
    subject: String,
    body: String,
    attachments: Vec<ComposeAttachment>,
) -> Result<Option<SendMessageResult>, EddieError> {
    info!("Sending message with {} attachments", attachments.len());

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    // Build the MIME message using the message service
    let raw_message = build_message(ComposeParams {
        from,
        to,
        cc,
        subject,
        body,
        attachments,
    })?;

    backend
        .send_message(&raw_message)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Get attachment information for a message (without downloading content)
#[tauri::command]
pub async fn get_message_attachments(
    account: Option<String>,
    folder: Option<String>,
    id: String,
) -> Result<Vec<AttachmentInfo>, EddieError> {
    info!("Getting attachments for message: {}", id);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let attachments = backend
        .get_attachment_info(folder.as_deref(), &id)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(attachments
        .into_iter()
        .enumerate()
        .map(|(index, a)| AttachmentInfo {
            index,
            filename: a.filename.unwrap_or_else(|| format!("attachment_{}", index)),
            mime_type: a.mime_type,
            size: a.size,
        })
        .collect())
}

/// Download a specific attachment from a message
#[tauri::command]
pub async fn download_attachment(
    account: Option<String>,
    folder: Option<String>,
    id: String,
    attachment_index: usize,
    download_dir: Option<String>,
) -> Result<String, EddieError> {
    info!(
        "Downloading attachment {} from message {}",
        attachment_index, id
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let download_path = resolve_download_path(download_dir)?;

    let file_path = backend
        .download_attachment(folder.as_deref(), &id, attachment_index, &download_path)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Download all attachments from a message
#[tauri::command]
pub async fn download_attachments(
    account: Option<String>,
    folder: Option<String>,
    id: String,
    download_dir: Option<String>,
) -> Result<Vec<String>, EddieError> {
    info!("Downloading all attachments from message {}", id);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let download_path = resolve_download_path(download_dir)?;

    let files = backend
        .download_all_attachments(folder.as_deref(), &id, &download_path)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(files
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// Resolve download path from optional parameter
fn resolve_download_path(download_dir: Option<String>) -> Result<PathBuf, EddieError> {
    let download_path: PathBuf = match download_dir {
        Some(dir) => dir.into(),
        None => dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
    };

    // Create directory if it doesn't exist
    fs::create_dir_all(&download_path)?;

    Ok(download_path)
}
