use std::sync::Arc;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};
use tokio::sync::RwLock;

mod commands;
mod config;
mod error;
mod models;
mod ollama;

pub use config::LlmConfig;
pub use error::{Error, Result};
pub use models::*;
pub use ollama::OllamaBackend;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
use desktop::NativeBackend;
#[cfg(mobile)]
use mobile::NativeBackend;

// Android plugin identifier (Java package name)
#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.plugin.llm";

// iOS plugin binding symbol
#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_llm);

/// The plugin's managed state, accessible from commands via AppHandle.
pub struct LlmState<R: Runtime> {
    /// Platform-specific backend (immutable after init)
    native: NativeBackend<R>,

    /// Ollama backend (hot-swappable at runtime)
    ///
    /// - Readers (list_models, generate) acquire a read lock - many can run concurrently.
    /// - Writers (configure_ollama) acquire a write lock - exclusive, but only held
    ///   for the instant it takes to swap the Option.
    /// - The OllamaBackend inside is immutable; reconfiguration replaces the entire value.
    ollama: Arc<RwLock<Option<OllamaBackend>>>,
}

impl<R: Runtime> LlmState<R> {
    pub fn ollama_lock(&self) -> &Arc<RwLock<Option<OllamaBackend>>> {
        &self.ollama
    }

    pub fn native(&self) -> &NativeBackend<R> {
        &self.native
    }
}

/// Extension trait: lets any Manager (AppHandle, Window, etc.) access the plugin state.
pub trait LlmExt<R: Runtime> {
    fn llm(&self) -> &LlmState<R>;
}

impl<R: Runtime, T: Manager<R>> LlmExt<R> for T {
    fn llm(&self) -> &LlmState<R> {
        self.state::<LlmState<R>>().inner()
    }
}

/// Initialize the plugin. Call from your app's main.rs:
///
/// ```rust,ignore
/// tauri::Builder::default()
///     .plugin(tauri_plugin_llm::init())
/// ```
pub fn init<R: Runtime>() -> TauriPlugin<R, Option<LlmConfig>> {
    Builder::<R, Option<LlmConfig>>::new("llm")
        .invoke_handler(tauri::generate_handler![
            commands::list_models,
            commands::generate,
            commands::configure_ollama,
        ])
        .setup(|app, api| {
            // Parse config from tauri.conf.json > plugins > llm (or use defaults)
            let config = api.config().clone().unwrap_or_default();

            // Build the initial Ollama backend from static config
            let ollama = OllamaBackend::from_config(&config);

            // Build the native backend (platform-specific)
            #[cfg(mobile)]
            let native = mobile::init(app, api)?;
            #[cfg(desktop)]
            let native = desktop::init(app, api)?;

            // Manage the combined state
            app.manage(LlmState {
                native,
                ollama: Arc::new(RwLock::new(ollama)),
            });

            Ok(())
        })
        .build()
}
