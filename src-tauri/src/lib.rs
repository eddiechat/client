mod backend;
mod commands;
mod config;
mod types;

use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    info!("Starting eddie ...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| {
            // Try to initialize config on startup
            if let Err(e) = config::init_config() {
                tracing::warn!("Could not load config on startup: {}", e);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Config commands
            commands::init_config,
            commands::init_config_from_paths,
            commands::is_config_initialized,
            commands::get_config_paths,
            commands::save_account,
            // Account commands
            commands::list_accounts,
            commands::get_default_account,
            commands::account_exists,
            commands::remove_account,
            commands::get_account_details,
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
            commands::save_message,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
