use tracing::info;

use crate::backend;
use crate::types::FlagRequest;

/// Add flags to messages
#[tauri::command]
pub async fn add_flags(request: FlagRequest) -> Result<(), String> {
    info!("Tauri command: add_flags - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    backend
        .add_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| e.to_string())
}

/// Remove flags from messages
#[tauri::command]
pub async fn remove_flags(request: FlagRequest) -> Result<(), String> {
    info!("Tauri command: remove_flags - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    backend
        .remove_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| e.to_string())
}

/// Set flags on messages (replace existing flags)
#[tauri::command]
pub async fn set_flags(request: FlagRequest) -> Result<(), String> {
    info!("Tauri command: set_flags - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let id_refs: Vec<&str> = request.ids.iter().map(|s| s.as_str()).collect();
    let flag_refs: Vec<&str> = request.flags.iter().map(|s| s.as_str()).collect();

    backend
        .set_flags(request.folder.as_deref(), &id_refs, &flag_refs)
        .await
        .map_err(|e| e.to_string())
}

/// Mark messages as read (convenience function)
#[tauri::command]
pub async fn mark_as_read(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
) -> Result<(), String> {
    add_flags(FlagRequest {
        account,
        folder,
        ids,
        flags: vec!["\\Seen".to_string()],
    })
    .await
}

/// Mark messages as unread (convenience function)
#[tauri::command]
pub async fn mark_as_unread(
    account: Option<String>,
    folder: Option<String>,
    ids: Vec<String>,
) -> Result<(), String> {
    remove_flags(FlagRequest {
        account,
        folder,
        ids,
        flags: vec!["\\Seen".to_string()],
    })
    .await
}

/// Toggle starred/flagged status (convenience function)
#[tauri::command]
pub async fn toggle_flagged(
    account: Option<String>,
    folder: Option<String>,
    id: String,
    is_flagged: bool,
) -> Result<(), String> {
    let request = FlagRequest {
        account,
        folder,
        ids: vec![id],
        flags: vec!["\\Flagged".to_string()],
    };

    if is_flagged {
        remove_flags(request).await
    } else {
        add_flags(request).await
    }
}
