//! Eddie Chat - Email client application
//!
//! This module provides the main Tauri application setup and configuration.
//!
//! ## Module Organization
//!
//! - `commands/`: Tauri command handlers (thin wrappers)
//! - `adapters/`: IMAP and SQLite adapter layers
//! - `engine/`: Background sync engine (tick-based worker)
//! - `services/`: Business logic (Tauri-agnostic)
//! - `types/`: Data structures and types
//! - `sync/`: Config database (account credentials, settings)
//! - `config/`: Configuration management
//! - `encryption/`: Secure credential storage
//! - `autodiscovery/`: Email provider auto-configuration

mod adapters;
mod autodiscovery;
mod commands;
mod config;
mod encryption;
mod engine;
mod services;
mod sync;
mod types;

use adapters::sqlite;
use tauri::Manager;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use std::path::{Path, PathBuf};

/// Get the sync database directory path
fn get_sync_db_dir() -> PathBuf {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        dirs::data_dir()
            .expect("Failed to determine data directory for iOS/Android")
            .join("eddie.chat")
            .join("sync")
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        if cfg!(debug_assertions) {
            PathBuf::from("../.sqlite")
        } else {
            dirs::data_local_dir()
                .expect("Failed to determine data directory for desktop")
                .join("eddie.chat")
                .join("sync")
        }
    }
}

/// Get the sync database file path
fn get_sync_db_path() -> PathBuf {
    get_sync_db_dir().join("sync.db")
}

/// Remove old per-account `.db` files from the sync DB directory.
/// These are leftover from the previous architecture where each account had its own DB file.
fn cleanup_old_db_files(dir: &Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Could not read sync DB directory for cleanup: {}", e);
            return;
        }
    };

    // In debug mode, config.db and sync.db share the same directory
    let keep_stems: &[&str] = if cfg!(debug_assertions) {
        &["sync", "config"]
    } else {
        &["sync"]
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Match .db, .db-shm, and .db-wal files (SQLite WAL mode companions)
        let is_db_file = filename.ends_with(".db")
            || filename.ends_with(".db-shm")
            || filename.ends_with(".db-wal");

        if is_db_file {
            // Extract the stem (e.g., "foo" from "foo.db" or "foo.db-shm")
            let stem = filename.strip_suffix(".db-wal")
                .or_else(|| filename.strip_suffix(".db-shm"))
                .or_else(|| filename.strip_suffix(".db"))
                .unwrap_or(filename);

            if !keep_stems.contains(&stem) {
                info!("Removing old database file: {}", path.display());
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Failed to remove old database file {}: {}", path.display(), e);
                }
            }
        }
    }
}

/// Seed sync DB with account rows and onboarding tasks for all accounts in Config DB.
/// All operations are idempotent (INSERT OR IGNORE).
fn seed_accounts_from_config(pool: &sqlite::DbPool) {
    let configs = match sync::db::get_all_connection_configs() {
        Ok(configs) => configs,
        Err(e) => {
            warn!("Could not read accounts from config DB for seeding: {}", e);
            return;
        }
    };

    for config in &configs {
        let account_id = &config.account_id;

        if let Err(e) = sqlite::accounts::ensure_account(pool, account_id) {
            warn!("Failed to seed account {}: {}", account_id, e);
            continue;
        }

        if let Err(e) = sqlite::onboarding_tasks::seed_tasks(pool, account_id) {
            warn!("Failed to seed onboarding tasks for {}: {}", account_id, e);
        }

        if let Err(e) = sqlite::entities::insert_entity(pool, account_id, account_id, "account", "user") {
            warn!("Failed to seed user entity for {}: {}", account_id, e);
        }
    }

    if !configs.is_empty() {
        info!("Seeded {} account(s) from config DB", configs.len());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize rustls crypto provider before any TLS operations
    // This is required for rustls 0.23+ which doesn't auto-select a provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize tracing for logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            EnvFilter::new("eddie_chat_lib=debug,info")
        } else {
            EnvFilter::new("info")
        }
    });

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting eddie ...");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            // Try to initialize config on startup
            if let Err(e) = config::init_config() {
                tracing::warn!("Could not load config on startup: {}", e);
            }

            // Initialize the config database (account credentials, settings)
            if let Err(e) = sync::db::init_config_db() {
                tracing::warn!("Could not initialize config database on startup: {}", e);
            }

            // Initialize the sync database (email cache)
            let db_dir = get_sync_db_dir();
            std::fs::create_dir_all(&db_dir)?;
            let db_path = get_sync_db_path();

            let pool = sqlite::pool::create_pool(&db_path)
                .expect("Failed to create sync database pool");

            let conn = pool.get().expect("Failed to get sync database connection");
            sqlite::schema::initialize_schema(&conn)
                .expect("Failed to initialize sync schema");
            drop(conn);

            // Clean up old per-account .db files from previous architecture
            cleanup_old_db_files(&db_dir);

            // Seed sync DB with accounts from Config DB (idempotent)
            seed_accounts_from_config(&pool);

            // Create wake channel for triggering sync
            let (wake_tx, mut wake_rx) = mpsc::channel::<()>(1);
            app.manage(wake_tx);
            app.manage(pool.clone());

            // Spawn background sync worker
            let engine_pool = pool;
            let engine_app = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    match engine::worker::tick(&engine_app, &engine_pool).await {
                        Ok(did_work) => {
                            if did_work {
                                continue;
                            }
                        }
                        Err(e) => {
                            error!("Engine error: {}", e);
                        }
                    }
                    // Sleep until woken or timeout
                    tokio::select! {
                        _ = wake_rx.recv() => {},
                        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {},
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // App commands
            commands::get_app_version,
            // Config commands
            commands::init_config,
            commands::init_config_from_paths,
            commands::is_config_initialized,
            commands::get_config_paths,
            commands::save_account,
            commands::get_read_only_mode,
            commands::set_read_only_mode,
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
            // Sync commands
            commands::init_sync_engine,
            commands::get_sync_status,
            commands::sync_now,
            commands::get_cached_conversations,
            commands::get_cached_conversation_messages,
            commands::fetch_message_body,
            commands::rebuild_conversations,
            commands::reclassify,
            commands::drop_and_resync,
            commands::mark_conversation_read,
            commands::search_entities,
            commands::shutdown_sync_engine,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
