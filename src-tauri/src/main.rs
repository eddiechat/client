// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing_subscriber::{prelude::*, Layer};

fn main() {
    let _guard = sentry::init((
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

    eddie_chat_lib::run()
}
