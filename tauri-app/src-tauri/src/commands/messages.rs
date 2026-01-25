use std::fs;
use std::path::PathBuf;
use tracing::info;

use crate::backend;
use crate::types::{Message, ReadMessageRequest};

/// Read a message by ID
#[tauri::command]
pub async fn read_message(request: ReadMessageRequest) -> Result<Message, String> {
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
) -> Result<(), String> {
    info!(
        "Tauri command: delete_messages - account: {:?}, folder: {:?}, ids: {:?}",
        account, folder, ids
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
    backend
        .delete_messages(folder.as_deref(), &id_refs)
        .await
        .map_err(|e| e.to_string())
}

/// Copy messages to another folder
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
}

/// Move messages to another folder
#[tauri::command]
pub async fn move_messages(
    account: Option<String>,
    source_folder: Option<String>,
    target_folder: String,
    ids: Vec<String>,
) -> Result<(), String> {
    info!(
        "Tauri command: move_messages - account: {:?}, source: {:?}, target: {}",
        account, source_folder, target_folder
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
    backend
        .move_messages(source_folder.as_deref(), &target_folder, &id_refs)
        .await
        .map_err(|e| e.to_string())
}

/// Send a message
#[tauri::command]
pub async fn send_message(account: Option<String>, message: String) -> Result<(), String> {
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
