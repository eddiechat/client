use std::fs;
use std::path::PathBuf;
use tauri::State;
use tracing::{info, warn};

use crate::backend;
use crate::commands::sync::SyncManager;
use crate::config;
use crate::types::{Message, ReadMessageRequest};

/// Helper to get account ID from optional parameter
fn get_account_id(account: Option<&str>) -> Result<String, String> {
    if let Some(id) = account {
        Ok(id.to_string())
    } else {
        let app_config = config::get_config().map_err(|e| e.to_string())?;
        app_config
            .default_account_name()
            .map(|s| s.to_string())
            .ok_or_else(|| "No default account configured".to_string())
    }
}

/// Read a message by ID
///
/// **DEPRECATED**: Fetches directly from IMAP.
/// Use `fetch_message_body` from sync commands for cached access.
#[tauri::command]
pub async fn read_message(request: ReadMessageRequest) -> Result<Message, String> {
    warn!("DEPRECATED: read_message called - migrate to fetch_message_body");
    info!("Tauri command: read_message - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .get_message(request.folder.as_deref(), &request.id, request.preview)
        .await
        .map_err(|e| e.to_string())
}

/// Delete messages
#[tauri::command]
pub async fn delete_messages(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), String> {
    info!(
        "Tauri command: delete_messages - account: {:?}, folder: {:?}, ids: {:?}",
        account, folder, ids
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .delete_messages(folder.as_deref(), &id_refs)
        .await
        .map_err(|e| e.to_string())?;

    // Update cache after successful server operation
    let account_id = get_account_id(account.as_deref())?;
    let folder_name = folder.as_deref().unwrap_or("INBOX");

    if let Some(engine) = sync_manager.get(&account_id).await {
        let db = engine.database();

        // Parse UIDs and delete from cache
        let uids: Vec<u32> = ids.iter().filter_map(|id| id.parse::<u32>().ok()).collect();

        if !uids.is_empty() {
            if let Err(e) = db.delete_messages_by_uids(&account_id, folder_name, &uids) {
                warn!("Failed to delete messages from cache: {}", e);
            }
        }
    }

    Ok(())
}

/// Copy messages to another folder
///
/// Note: Cache update is skipped - the next sync will pick up the copied messages.
#[tauri::command]
pub async fn copy_messages(
    account: Option<String>,
    source_folder: Option<String>,
    target_folder: String,
    ids: Vec<String>,
) -> Result<(), String> {
    info!(
        "Tauri command: copy_messages - account: {:?}, source: {:?}, target: {}",
        account, source_folder, target_folder
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
    backend
        .copy_messages(source_folder.as_deref(), &target_folder, &id_refs)
        .await
        .map_err(|e| e.to_string())

    // Note: We don't update the cache here because copy creates new messages
    // with new UIDs in the target folder. The next sync will pick them up.
}

/// Move messages to another folder
#[tauri::command]
pub async fn move_messages(
    account: Option<String>,
    source_folder: Option<String>,
    target_folder: String,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), String> {
    info!(
        "Tauri command: move_messages - account: {:?}, source: {:?}, target: {}",
        account, source_folder, target_folder
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .move_messages(source_folder.as_deref(), &target_folder, &id_refs)
        .await
        .map_err(|e| e.to_string())?;

    // Update cache after successful server operation
    // For move operations, we delete from source folder cache.
    // The messages in target folder will have new UIDs, so the next sync will pick them up.
    let account_id = get_account_id(account.as_deref())?;
    let folder_name = source_folder.as_deref().unwrap_or("INBOX");

    if let Some(engine) = sync_manager.get(&account_id).await {
        let db = engine.database();

        // Parse UIDs and delete from source folder cache
        let uids: Vec<u32> = ids.iter().filter_map(|id| id.parse::<u32>().ok()).collect();

        if !uids.is_empty() {
            if let Err(e) = db.delete_messages_by_uids(&account_id, folder_name, &uids) {
                warn!("Failed to delete moved messages from cache: {}", e);
            }
        }
    }

    Ok(())
}

/// Send a message via SMTP and save to Sent folder
/// Returns the message ID in the Sent folder, or None if no Sent folder was found
#[tauri::command]
pub async fn send_message(
    account: Option<String>,
    message: String,
) -> Result<Option<String>, String> {
    info!(
        "Tauri command: send_message - account: {:?}, len: {}",
        account,
        message.len()
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .send_message(message.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Save a message to a folder (drafts)
#[tauri::command]
pub async fn save_message(
    account: Option<String>,
    folder: Option<String>,
    message: String,
) -> Result<String, String> {
    info!(
        "Tauri command: save_message - account: {:?}, folder: {:?}",
        account, folder
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .save_message(folder.as_deref(), message.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Download attachments from a message
#[tauri::command]
pub async fn download_attachments(
    account: Option<String>,
    folder: Option<String>,
    id: String,
    download_dir: Option<String>,
) -> Result<Vec<String>, String> {
    info!(
        "Tauri command: download_attachments - account: {:?}, folder: {:?}, id: {}, dir: {:?}",
        account, folder, id, download_dir
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    // Get the message to access attachments
    let message = backend
        .get_message(folder.as_deref(), &id, true)
        .await
        .map_err(|e| e.to_string())?;

    // Determine download directory
    let download_path: PathBuf = match download_dir {
        Some(dir) => dir.into(),
        None => dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
    };

    // Create directory if it doesn't exist
    fs::create_dir_all(&download_path).map_err(|e| e.to_string())?;

    let mut downloaded_files = Vec::new();

    // Note: To actually download attachment contents, we'd need to fetch the raw message
    // and parse attachments. For now, return the attachment info.
    for attachment in &message.attachments {
        if let Some(filename) = &attachment.filename {
            let file_path = download_path.join(filename);
            downloaded_files.push(file_path.to_string_lossy().to_string());
        }
    }

    Ok(downloaded_files)
}
