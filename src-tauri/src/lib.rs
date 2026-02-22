mod adapters;
mod autodiscovery;
mod services;
mod commands;
pub mod error;

use adapters::sqlite::{sync};
use services::ollama::OllamaState;
use tauri::Manager;
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{prelude::*, Layer};

const SYNC_WORKER_TICK_FREQ: u64 = 15; // seconds

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let _sentry_guard = sentry::init((
                "https://52c142f86a5adb01226a7aec943c63bc@o4506308159340544.ingest.us.sentry.io/4510925988036608",
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    enable_logs: true,
                    ..Default::default()
                }
            ));

            let fmt_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "eddie_chat_lib=info,error".into());
            let sentry_filter = tracing_subscriber::filter::Targets::new()
                .with_target("eddie_chat_lib", tracing::Level::INFO)
                .with_default(tracing::Level::ERROR);

            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
                .with(sentry::integrations::tracing::layer().with_filter(sentry_filter))
                .init();

            rustls::crypto::aws_lc_rs::default_provider()
                .install_default()
                .expect("Failed to install rustls crypto provider");
            
            let pool = sync::db::initialize(app.handle())
                .expect("Failed to initialize sync database");

            services::logger::init(&pool);
            services::logger::info("App initialized");

            let engine_pool = pool.clone();
            let engine_app = app.handle().clone();

            let (wake_tx, mut wake_rx) = mpsc::channel::<()>(1);
            app.manage(wake_tx);

            tauri::async_runtime::spawn(async move {
                loop {
                    match services::sync::worker::tick(&engine_app, &engine_pool).await {
                        Ok(did_work) => {
                            if did_work {
                                continue;
                            }
                        }
                        Err(e) => {
                            services::logger::error(&format!("Engine error: {}", e));
                        }
                    }
                    // Sleep until woken or timeout
                    tokio::select! {
                        _ = wake_rx.recv() => {},
                        _ = tokio::time::sleep(std::time::Duration::from_secs(SYNC_WORKER_TICK_FREQ)) => {},
                    }
                }
            });

            // Ollama model discovery (non-blocking)
            let ollama_state: OllamaState = Arc::new(RwLock::new(HashMap::new()));
            app.manage(ollama_state.clone());
            let ollama_pool = pool.clone();
            tauri::async_runtime::spawn(async move {
                services::ollama::populate(&ollama_pool, &ollama_state).await;
            });

            // Make the pool available to all Tauri commands via State
            app.manage(pool);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::account::connect_account,
            commands::account::get_existing_account,
            commands::conversations::fetch_conversations,
            commands::conversations::fetch_conversation_messages,
            commands::conversations::fetch_clusters,
            commands::conversations::fetch_cluster_messages,
            commands::conversations::fetch_cluster_threads,
            commands::conversations::fetch_thread_messages,
            commands::conversations::group_domains,
            commands::conversations::ungroup_domains,
            commands::classify::reclassify,
            commands::sync::sync_now,
            commands::sync::get_onboarding_status,
            commands::skills::list_skills,
            commands::skills::get_skill,
            commands::skills::create_skill,
            commands::skills::update_skill,
            commands::skills::toggle_skill,
            commands::skills::delete_skill,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::get_ollama_models,
            commands::conversations::move_to_lines,
            commands::conversations::fetch_recent_messages,
            commands::ollama::ollama_complete,
            commands::discovery::discover_email_config,
            commands::app::get_app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
