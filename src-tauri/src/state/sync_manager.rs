//! Sync engine manager state
//!
//! Manages multiple sync engine instances, one per email account.
//! This module is Tauri-aware only for event emission (app handle).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::{EmailAccountConfig, ImapConfig, SmtpConfig};
use crate::sync::db::{get_connection_config, init_config_db};
use crate::sync::engine::{SyncConfig, SyncEngine};
use crate::sync::idle::MonitorConfig;
use crate::types::error::EddieError;

/// Sync engine manager state - manages engines for multiple accounts
pub struct SyncManager {
    engines: RwLock<HashMap<String, Arc<RwLock<SyncEngine>>>>,
    default_db_dir: PathBuf,
    app_handle: RwLock<Option<tauri::AppHandle>>,
}

impl SyncManager {
    pub fn new() -> Self {
        let db_dir = Self::determine_db_directory();

        // Ensure directory exists
        let _ = std::fs::create_dir_all(&db_dir);

        info!("Sync database directory: {:?}", db_dir);

        Self {
            engines: RwLock::new(HashMap::new()),
            default_db_dir: db_dir,
            app_handle: RwLock::new(None),
        }
    }

    /// Determine the database directory based on platform and build mode
    fn determine_db_directory() -> PathBuf {
        // On mobile platforms (iOS/Android), always use data_dir() even in debug mode
        // because the current directory is read-only
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            dirs::data_dir()
                .expect("Failed to determine data directory for iOS/Android")
                .join("eddie.chat")
                .join("sync")
        }

        // On desktop, use ../.sqlite in debug mode for easier debugging
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

    /// Get the database directory path
    pub fn db_directory(&self) -> &PathBuf {
        &self.default_db_dir
    }

    /// Set the Tauri app handle for event emission
    pub async fn set_app_handle(&self, handle: tauri::AppHandle) {
        let mut app_handle = self.app_handle.write().await;
        *app_handle = Some(handle);
    }

    /// Get or create sync engine for an account
    pub async fn get_or_create(
        &self,
        account_id: &str,
    ) -> Result<Arc<RwLock<SyncEngine>>, EddieError> {
        // Check if engine exists
        {
            let engines = self.engines.read().await;
            if let Some(engine) = engines.get(account_id) {
                return Ok(engine.clone());
            }
        }

        // Create new engine
        info!("Creating sync engine for account: {}", account_id);

        let engine = self.create_engine(account_id).await?;
        let engine = Arc::new(RwLock::new(engine));

        // Store engine
        {
            let mut engines = self.engines.write().await;
            engines.insert(account_id.to_string(), engine.clone());
        }

        // Configure Ollama classifier if enabled in settings
        self.configure_ollama_for_engine(&engine).await;

        Ok(engine)
    }

    /// Configure Ollama classifier on an engine based on app_settings
    async fn configure_ollama_for_engine(&self, engine: &Arc<RwLock<SyncEngine>>) {
        use crate::sync::db::get_app_setting;
        use crate::sync::ollama_classifier::{OllamaClassifier, OllamaConfig};

        let enabled = get_app_setting("ollama_enabled")
            .map(|s| s == "true")
            .unwrap_or(false);

        if enabled {
            let url = get_app_setting("ollama_url")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            let model = get_app_setting("ollama_model")
                .unwrap_or_else(|_| "mistral:latest".to_string());

            let config = OllamaConfig {
                url,
                model,
                enabled: true,
            };
            let engine_guard = engine.read().await;
            engine_guard
                .classifier()
                .set_ollama(Some(OllamaClassifier::new(config)))
                .await;
            info!("Ollama classifier configured for engine");
        }
    }

    /// Create a new sync engine for an account
    async fn create_engine(&self, account_id: &str) -> Result<SyncEngine, EddieError> {
        init_config_db()?;
        let db_config = get_connection_config(account_id)?
            .ok_or_else(|| EddieError::AccountNotFound(account_id.to_string()))?;

        // Deserialize IMAP and SMTP configs from JSON
        let imap_config = db_config
            .imap_config
            .and_then(|json| serde_json::from_str::<ImapConfig>(&json).ok());

        let smtp_config = db_config
            .smtp_config
            .and_then(|json| serde_json::from_str::<SmtpConfig>(&json).ok());

        let account = EmailAccountConfig {
            name: db_config.display_name.clone(),
            default: db_config.active,
            email: db_config.email.clone(),
            display_name: db_config.display_name.clone(),
            imap: imap_config,
            smtp: smtp_config,
        };

        // Use account ID as-is for database filename (preserving @ and . characters)
        let db_path = self.default_db_dir.join(format!("{}.db", db_config.account_id));
        info!("Database path: {:?}", db_path);

        let sync_config = SyncConfig {
            db_path,
            initial_sync_days: 365,
            max_cache_age_days: 365,
            auto_classify: true,
            sync_folders: vec![],
            fetch_page_size: 1000,
            enable_monitoring: true,
            monitor_config: MonitorConfig {
                prefer_idle: true,
                idle_timeout_minutes: 20,
                poll_interval_seconds: 60,
                use_quick_check: true,
            },
        };

        // Get app handle for event emission
        let app_handle = self.app_handle.read().await.clone();

        // Parse aliases from comma-separated string
        let user_aliases = db_config
            .aliases
            .as_ref()
            .map(|s| {
                s.split(',')
                    .map(|a| a.trim().to_lowercase())
                    .filter(|a| !a.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        SyncEngine::new(
            db_config.account_id.clone(),
            account.email.clone(),
            user_aliases,
            account,
            sync_config,
            app_handle,
        )
    }

    /// Get sync engine for account (if exists)
    pub async fn get(&self, account_id: &str) -> Option<Arc<RwLock<SyncEngine>>> {
        let engines = self.engines.read().await;
        engines.get(account_id).cloned()
    }

    /// Remove sync engine for account
    pub async fn remove(&self, account_id: &str) {
        let mut engines = self.engines.write().await;
        if let Some(engine) = engines.remove(account_id) {
            engine.read().await.shutdown();
        }
    }

    /// Get all engine account IDs
    #[allow(dead_code)]
    pub async fn get_account_ids(&self) -> Vec<String> {
        let engines = self.engines.read().await;
        engines.keys().cloned().collect()
    }

    /// Get all active engines
    pub async fn get_all_engines(&self) -> Vec<Arc<RwLock<SyncEngine>>> {
        let engines = self.engines.read().await;
        engines.values().cloned().collect()
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}
