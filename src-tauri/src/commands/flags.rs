//! Flag Tauri commands
//!
//! Commands for managing email message flags (read, starred, etc.).

use tauri::State;
use tracing::{info, warn};

use crate::backend;
use crate::services::resolve_account_id;
use crate::state::SyncManager;
use crate::sync::db::is_read_only_mode;
use crate::types::{EddieError, FlagRequest};

/// Add flags to messages
#[tauri::command]
pub async fn add_flags(
    request: FlagRequest,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    // Check read-only mode
    if is_read_only_mode()? {
        info!(
            "Read-only mode: Blocked add_flags - account: {:?}, folder: {:?}, flags: {:?}, ids: {} items",
            request.account, request.folder, request.flags, request.ids.len()
        );
        return Err(EddieError::ReadOnlyMode);
    }

    info!("Adding flags: {:?}", request.flags);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .add_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    // Update cache after successful server operation
    update_cache_add_flags(&sync_manager, &request).await;

    Ok(())
}

/// Remove flags from messages
#[tauri::command]
pub async fn remove_flags(
    request: FlagRequest,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    // Check read-only mode
    if is_read_only_mode()? {
        info!(
            "Read-only mode: Blocked remove_flags - account: {:?}, folder: {:?}, flags: {:?}, ids: {} items",
            request.account, request.folder, request.flags, request.ids.len()
        );
        return Err(EddieError::ReadOnlyMode);
    }

    info!("Removing flags: {:?}", request.flags);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .remove_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    // Update cache after successful server operation
    update_cache_remove_flags(&sync_manager, &request).await;

    Ok(())
}

/// Set flags on messages (replace existing flags)
#[tauri::command]
pub async fn set_flags(
    request: FlagRequest,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    // Check read-only mode
    if is_read_only_mode()? {
        info!(
            "Read-only mode: Blocked set_flags - account: {:?}, folder: {:?}, flags: {:?}, ids: {} items",
            request.account, request.folder, request.flags, request.ids.len()
        );
        return Err(EddieError::ReadOnlyMode);
    }

    info!("Setting flags: {:?}", request.flags);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .set_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    // Update cache after successful server operation
    update_cache_set_flags(&sync_manager, &request).await;

    Ok(())
}

/// Mark messages as read (convenience function)
#[tauri::command]
pub async fn mark_as_read(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    add_flags(
        FlagRequest {
            account,
            folder,
            ids,
            flags: vec!["\\Seen".to_string()],
        },
        sync_manager,
    )
    .await
}

/// Mark messages as unread (convenience function)
#[tauri::command]
pub async fn mark_as_unread(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    remove_flags(
        FlagRequest {
            account,
            folder,
            ids,
            flags: vec!["\\Seen".to_string()],
        },
        sync_manager,
    )
    .await
}

/// Toggle starred/flagged status (convenience function)
#[tauri::command]
pub async fn toggle_flagged(
    account: Option<String>,
    folder: Option<String>,
    id: String,
    is_flagged: bool,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), EddieError> {
    let request = FlagRequest {
        account,
        folder,
        ids: vec![id],
        flags: vec!["\\Flagged".to_string()],
    };

    if is_flagged {
        remove_flags(request, sync_manager).await
    } else {
        add_flags(request, sync_manager).await
    }
}

// ========== Cache Update Helpers ==========

async fn update_cache_add_flags(sync_manager: &SyncManager, request: &FlagRequest) {
    let account_id = match resolve_account_id(request.account.as_deref()) {
        Ok(id) => id,
        Err(_) => return,
    };
    let folder = request.folder.as_deref().unwrap_or("INBOX");

    if let Some(engine) = sync_manager.get(&account_id).await {
        let db = engine.read().await.database();
        for id in &request.ids {
            if let Ok(uid) = id.parse::<u32>() {
                if let Err(e) = db.add_message_flags(&account_id, folder, uid, &request.flags) {
                    warn!("Failed to update cache flags for UID {}: {}", uid, e);
                }
            }
        }
    }
}

async fn update_cache_remove_flags(sync_manager: &SyncManager, request: &FlagRequest) {
    let account_id = match resolve_account_id(request.account.as_deref()) {
        Ok(id) => id,
        Err(_) => return,
    };
    let folder = request.folder.as_deref().unwrap_or("INBOX");

    if let Some(engine) = sync_manager.get(&account_id).await {
        let db = engine.read().await.database();
        for id in &request.ids {
            if let Ok(uid) = id.parse::<u32>() {
                if let Err(e) = db.remove_message_flags(&account_id, folder, uid, &request.flags) {
                    warn!("Failed to update cache flags for UID {}: {}", uid, e);
                }
            }
        }
    }
}

async fn update_cache_set_flags(sync_manager: &SyncManager, request: &FlagRequest) {
    let account_id = match resolve_account_id(request.account.as_deref()) {
        Ok(id) => id,
        Err(_) => return,
    };
    let folder = request.folder.as_deref().unwrap_or("INBOX");

    if let Some(engine) = sync_manager.get(&account_id).await {
        let db = engine.read().await.database();
        for id in &request.ids {
            if let Ok(uid) = id.parse::<u32>() {
                if let Err(e) = db.set_message_flags_vec(&account_id, folder, uid, &request.flags) {
                    warn!("Failed to update cache flags for UID {}: {}", uid, e);
                }
            }
        }
    }
}
