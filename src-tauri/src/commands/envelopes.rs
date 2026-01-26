use tracing::info;

use crate::backend;
use crate::types::{Envelope, ListEnvelopesRequest, ListEnvelopesResponse};

/// List envelopes (email metadata) for a folder
#[tauri::command]
pub async fn list_envelopes(
    request: ListEnvelopesRequest,
) -> Result<ListEnvelopesResponse, String> {
    info!("Tauri command: list_envelopes - {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    // Frontend uses 1-based pages, backend uses 0-based indexing
    let page = request.page.unwrap_or(1).saturating_sub(1);
    let page_size = request.page_size.unwrap_or(50);

    let envelopes = backend
        .list_envelopes(request.folder.as_deref(), page, page_size)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ListEnvelopesResponse {
        envelopes,
        page,
        page_size,
        total: None, // IMAP doesn't easily provide total count
    })
}

/// Thread envelopes (group by conversation)
#[tauri::command]
pub async fn thread_envelopes(
    account: Option<String>,
    folder: Option<String>,
    _envelope_id: Option<String>,
    _query: Option<String>,
) -> Result<Vec<Envelope>, String> {
    info!(
        "Tauri command: thread_envelopes - account: {:?}, folder: {:?}",
        account, folder
    );

    // For now, just return list of envelopes - threading requires additional implementation
    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .list_envelopes(folder.as_deref(), 0, 100)
        .await
        .map_err(|e| e.to_string())
}
