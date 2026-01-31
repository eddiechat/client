use tracing::info;

use crate::backend::carddav::get_carddav_backend;
use crate::types::contact::{AddressBook, Contact, SaveContactRequest};

/// List all contacts from the CardDAV server
#[tauri::command]
pub async fn list_contacts(account: Option<String>) -> Result<Vec<Contact>, String> {
    info!("Tauri command: list_contacts");

    let backend = get_carddav_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend.list_contacts().await.map_err(|e| e.to_string())
}

/// Get a single contact by ID
#[tauri::command]
pub async fn get_contact(account: Option<String>, contact_id: String) -> Result<Contact, String> {
    info!("Tauri command: get_contact - {}", contact_id);

    let backend = get_carddav_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .get_contact(&contact_id)
        .await
        .map_err(|e| e.to_string())
}

/// Create a new contact
#[tauri::command]
pub async fn create_contact(request: SaveContactRequest) -> Result<Contact, String> {
    info!("Tauri command: create_contact - {}", request.contact.full_name);

    let backend = get_carddav_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .create_contact(&request.contact)
        .await
        .map_err(|e| e.to_string())
}

/// Update an existing contact
#[tauri::command]
pub async fn update_contact(request: SaveContactRequest) -> Result<Contact, String> {
    info!("Tauri command: update_contact - {}", request.contact.full_name);

    let backend = get_carddav_backend(request.account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .update_contact(&request.contact)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a contact
#[tauri::command]
pub async fn delete_contact(
    account: Option<String>,
    contact_id: String,
    href: Option<String>,
) -> Result<(), String> {
    info!("Tauri command: delete_contact - {}", contact_id);

    let backend = get_carddav_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .delete_contact(&contact_id, href.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// List available address books
#[tauri::command]
pub async fn list_address_books(account: Option<String>) -> Result<Vec<AddressBook>, String> {
    info!("Tauri command: list_address_books");

    let backend = get_carddav_backend(account.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    backend
        .list_address_books()
        .await
        .map_err(|e| e.to_string())
}

/// Check if CardDAV is configured for an account
#[tauri::command]
pub async fn has_carddav_config(account: Option<String>) -> Result<bool, String> {
    info!("Tauri command: has_carddav_config");

    let config = crate::config::get_config().map_err(|e| e.to_string())?;

    let (_, account_config) = match account {
        Some(ref name) => config
            .get_account(Some(name))
            .ok_or_else(|| format!("Account '{}' not found", name))?,
        None => config
            .get_account(None)
            .ok_or_else(|| "No default account configured".to_string())?,
    };

    Ok(account_config.carddav.is_some())
}
