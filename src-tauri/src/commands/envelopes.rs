//! Envelope Tauri commands
//!
//! Commands for listing email envelopes (metadata).
//! These are deprecated in favor of the sync engine.

use tracing::{info, warn};

use crate::backend;
use crate::types::{EddieError, Envelope, ListEnvelopesRequest, ListEnvelopesResponse};

/// List envelopes (email metadata) for a folder
///
/// **DEPRECATED**: Fetches directly from IMAP.
/// Use sync engine and read from SQLite cache for better performance.
#[tauri::command]
pub async fn list_envelopes(
    request: ListEnvelopesRequest,
) -> Result<ListEnvelopesResponse, EddieError> {
    warn!("DEPRECATED: list_envelopes called - migrate to sync engine equivalent");
    info!("Listing envelopes: {:?}", request);

    let backend = backend::get_backend(request.account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    // Frontend uses 1-based pages, backend uses 0-based indexing
    let page = request.page.unwrap_or(1).saturating_sub(1);
    let page_size = request.page_size.unwrap_or(50);

    let envelopes = backend
        .list_envelopes(request.folder.as_deref(), page, page_size)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    Ok(ListEnvelopesResponse {
        envelopes,
        page,
        page_size,
        total: None,
    })
}

/// Thread envelopes (group by conversation)
///
/// **DEPRECATED**: Fetches directly from IMAP.
/// Use sync engine and read from SQLite cache for better performance.
#[tauri::command]
pub async fn thread_envelopes(
    account: Option<String>,
    folder: Option<String>,
    _envelope_id: Option<String>,
    _query: Option<String>,
) -> Result<Vec<Envelope>, EddieError> {
    warn!("DEPRECATED: thread_envelopes called - migrate to sync engine equivalent");
    info!("Threading envelopes");

    let backend = backend::get_backend(account.as_deref())
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))?;

    backend
        .list_envelopes(folder.as_deref(), 0, 100)
        .await
        .map_err(|e| EddieError::Backend(e.to_string()))
}
