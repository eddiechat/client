//! Eddie Chat - Email client application
//!
//! This module provides the main Tauri application setup and configuration.
//!
//! ## Module Organization
//!
//! - `commands/`: Tauri command handlers (thin wrappers)
//! - `services/`: Business logic (Tauri-agnostic)
//! - `state/`: Application state management
//! - `types/`: Data structures and types
//! - `backend/`: Email protocol implementation
//! - `sync/`: Sync engine for offline support
//! - `config/`: Configuration management
//! - `credentials/`: Secure credential storage
//! - `autodiscovery/`: Email provider auto-configuration

mod autodiscovery;
mod backend;
mod commands;
mod config;
mod credentials;
mod services;
mod state;
mod sync;
mod types;

use state::SyncManager;
use tauri::Manager;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize rustls crypto provider before any TLS operations
    // This is required for rustls 0.23+ which doesn't auto-select a provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize tracing for logging
    // In debug builds, default to debug level for our crate
    // Can be overridden with RUST_LOG environment variable
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            // Debug build: show debug logs for our crate, info for others
            EnvFilter::new("eddie_chat_lib=debug,info")
        } else {
            // Release build: show info and above
            EnvFilter::new("info")
        }
    });

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting eddie ...");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .manage(SyncManager::new())
        .setup(|app| {
            // Try to initialize config on startup
            if let Err(e) = config::init_config() {
                tracing::warn!("Could not load config on startup: {}", e);
            }

            // Initialize the config database
            if let Err(e) = sync::db::init_config_db() {
                tracing::warn!("Could not initialize config database on startup: {}", e);
            }

            // Set app handle on sync manager for event emission
            let sync_manager = app.state::<SyncManager>();
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async {
                sync_manager.set_app_handle(handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Config commands
            commands::init_config,
            commands::init_config_from_paths,
            commands::is_config_initialized,
            commands::get_config_paths,
            commands::save_account,
            // Account database commands
            commands::init_config_database,
            commands::get_accounts,
            commands::get_active_account,
            commands::switch_account,
            commands::delete_account,
            // Account commands
            commands::list_accounts,
            commands::get_default_account,
            commands::account_exists,
            commands::remove_account,
            commands::get_account_details,
            // Autodiscovery commands
            commands::discover_email_config,
            commands::test_email_connection,
            // Credential commands
            commands::store_password,
            commands::store_app_password,
            commands::delete_credentials,
            commands::has_credentials,
            // Combined account setup
            commands::save_discovered_account,
            // Folder commands
            commands::list_folders,
            commands::create_folder,
            commands::delete_folder,
            commands::expunge_folder,
            // Envelope commands
            commands::list_envelopes,
            commands::thread_envelopes,
            // Message commands
            commands::read_message,
            commands::delete_messages,
            commands::copy_messages,
            commands::move_messages,
            commands::send_message,
            commands::send_message_with_attachments,
            commands::save_message,
            commands::get_message_attachments,
            commands::download_attachment,
            commands::download_attachments,
            // Flag commands
            commands::add_flags,
            commands::remove_flags,
            commands::set_flags,
            commands::mark_as_read,
            commands::mark_as_unread,
            commands::toggle_flagged,
            // Conversation commands
            commands::list_conversations,
            commands::get_conversation_messages,
            // Sync commands
            commands::init_sync_engine,
            commands::get_sync_status,
            commands::sync_folder,
            commands::initial_sync,
            commands::get_cached_conversations,
            commands::get_cached_conversation_messages,
            commands::fetch_message_body,
            commands::queue_sync_action,
            commands::set_sync_online,
            commands::has_pending_sync_actions,
            commands::start_monitoring,
            commands::stop_monitoring,
            commands::shutdown_sync_engine,
            commands::mark_conversation_read,
            commands::search_entities,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
