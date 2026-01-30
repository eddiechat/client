//! Folder Tauri commands
//!
//! Commands for managing email folders/mailboxes.

use tracing::info;

use crate::backend;
use crate::types::{EddieError, Folder};

/// List all folders for an account
#[tauri::command]
pub async fn list_folders(account: Option<String>) -> Result<Vec<Folder>, EddieError> {
    info!("Listing folders");

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .list_folders()
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Create a new folder
#[tauri::command]
pub async fn create_folder(account: Option<String>, name: String) -> Result<(), EddieError> {
    info!("Creating folder: {}", name);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .create_folder(&name)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Delete a folder
#[tauri::command]
pub async fn delete_folder(account: Option<String>, name: String) -> Result<(), EddieError> {
    info!("Deleting folder: {}", name);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .delete_folder(&name)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}

/// Expunge a folder (permanently delete messages marked as deleted)
#[tauri::command]
pub async fn expunge_folder(account: Option<String>, name: String) -> Result<(), EddieError> {
    info!("Expunging folder: {}", name);

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .expunge_folder(&name)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}
