mod adapters;
mod autodiscovery;
mod services;
mod commands;
pub mod error;

use adapters::sqlite::sync;
use services::sync::helpers::message_classification::ClassifierState;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::mpsc;
use tracing_subscriber::{prelude::*, Layer};

const SYNC_WORKER_TICK_FREQ: u64 = 15; // seconds

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install rustls crypto provider first — needed by sentry's reqwest transport (rustls).
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Sentry guard must live for the duration of run() — if dropped, Sentry shuts down.
    let _sentry_guard = sentry::init((
        "https://52c142f86a5adb01226a7aec943c63bc@o4506308159340544.ingest.us.sentry.io/4510925988036608",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            debug: cfg!(debug_assertions),
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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let pool = sync::db::initialize(app.handle())
                .expect("Failed to initialize sync database");

            services::logger::init(&pool);
            services::logger::info("App initialized");

            // Load ONNX classifier model once at startup.
            // In production, resources are bundled via tauri.conf.json.
            // In dev mode, fall back to src-tauri/resources/.
            let resource_dir = app.path().resource_dir()
                .expect("Failed to resolve resource directory");
            let bundled = resource_dir.join("resources/model_int8.onnx");
            let dev_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");

            let (model_path, tokenizer_path) = if bundled.exists() {
                (bundled, resource_dir.join("resources/tokenizer.json"))
            } else {
                (dev_dir.join("model_int8.onnx"), dev_dir.join("tokenizer.json"))
            };

            let classifier = Arc::new(
                ClassifierState::load(&model_path, &tokenizer_path)
                    .expect("Failed to load ONNX classifier model")
            );
            services::logger::info("ONNX classifier loaded");

            let engine_pool = pool.clone();
            let engine_app = app.handle().clone();
            let engine_classifier = classifier.clone();

            let (wake_tx, mut wake_rx) = mpsc::channel::<()>(1);
            app.manage(wake_tx);

            tauri::async_runtime::spawn(async move {
                loop {
                    match services::sync::worker::tick(&engine_app, &engine_pool, &engine_classifier).await {
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

            // Make the pool and classifier available to all Tauri commands via State
            app.manage(pool);
            app.manage(classifier);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::account::connect_account,
            commands::account::get_existing_account,
            commands::conversations::fetch_conversations,
            commands::conversations::fetch_conversation_messages,
            commands::classify::reclassify,
            commands::sync::sync_now,
            commands::sync::get_onboarding_status,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::conversations::move_to_requests,
            commands::conversations::move_to_points,
            commands::conversations::block_entities,
            commands::conversations::fetch_recent_messages,
            commands::conversations::fetch_message_html,
            commands::discovery::discover_email_config,
            commands::app::get_app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
