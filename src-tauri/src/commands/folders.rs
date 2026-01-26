use tracing::info;

use crate::backend;
use crate::types::Folder;

/// List all folders for an account
#[tauri::command]
pub async fn list_folders(account: Option<String>) -> Result<Vec<Folder>, String> {
    info!("Tauri command: list_folders - account: {:?}", account);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend.list_folders().await.map_err(|e| e.to_string())
}

/// Create a new folder
#[tauri::command]
pub async fn create_folder(account: Option<String>, name: String) -> Result<(), String> {
    info!(
        "Tauri command: create_folder - account: {:?}, name: {}",
        account, name
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .create_folder(&name)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a folder
#[tauri::command]
pub async fn delete_folder(account: Option<String>, name: String) -> Result<(), String> {
    info!(
        "Tauri command: delete_folder - account: {:?}, name: {}",
        account, name
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .delete_folder(&name)
        .await
        .map_err(|e| e.to_string())
}

/// Expunge a folder (permanently delete messages marked as deleted)
#[tauri::command]
pub async fn expunge_folder(account: Option<String>, name: String) -> Result<(), String> {
    info!(
        "Tauri command: expunge_folder - account: {:?}, name: {}",
        account, name
    );

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .expunge_folder(&name)
        .await
        .map_err(|e| e.to_string())
}
