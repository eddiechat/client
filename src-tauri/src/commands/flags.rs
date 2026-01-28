use tauri::State;
use tracing::{info, warn};

use crate::backend;
use crate::commands::sync::SyncManager;
use crate::sync::db::{get_active_connection_config, init_config_db};
use crate::types::FlagRequest;

/// Helper to get account ID from optional parameter
fn get_account_id(account: Option<&str>) -> Result<String, String> {
    if let Some(id) = account {
        Ok(id.to_string())
    } else {
        init_config_db().map_err(|e| e.to_string())?;
        let active_config = get_active_connection_config().map_err(|e| e.to_string())?;
        active_config
            .map(|c| c.account_id)
            .ok_or_else(|| "No active account configured".to_string())
    }
}

/// Add flags to messages
#[tauri::command]
pub async fn add_flags(
    request: FlagRequest,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), String> {
    info!("Tauri command: add_flags - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .add_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| e.to_string())?;

    // Update cache after successful server operation
    let account_id = get_account_id(request.account.as_deref())?;
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

    Ok(())
}

/// Remove flags from messages
#[tauri::command]
pub async fn remove_flags(
    request: FlagRequest,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), String> {
    info!("Tauri command: remove_flags - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .remove_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| e.to_string())?;

    // Update cache after successful server operation
    let account_id = get_account_id(request.account.as_deref())?;
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

    Ok(())
}

/// Set flags on messages (replace existing flags)
#[tauri::command]
pub async fn set_flags(
    request: FlagRequest,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), String> {
    info!("Tauri command: set_flags - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    // Execute on server
    backend
        .set_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| e.to_string())?;

    // Update cache after successful server operation
    let account_id = get_account_id(request.account.as_deref())?;
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

    Ok(())
}

/// Mark messages as read (convenience function)
#[tauri::command]
pub async fn mark_as_read(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
    sync_manager: State<'_, SyncManager>,
) -> Result<(), String> {
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
) -> Result<(), String> {
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
) -> Result<(), String> {
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
