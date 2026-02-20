use crate::{models::*, LlmExt, OllamaBackend, Result};
use tauri::{command, AppHandle, Runtime};

/// List all available models from every backend.
///
/// Returns an array merging:
/// - 0 or 1 OS-native models (depending on platform and hardware)
/// - 0 to N Ollama models (if configured and reachable)
///
/// Never errors - backend failures are logged and that backend contributes 0 models.
#[command]
pub(crate) async fn list_models<R: Runtime>(app: AppHandle<R>) -> Result<Vec<ModelInfo>> {
    let state = app.llm();

    // 1. OS-native models (synchronous, platform-specific)
    let mut models = state.native().list_models();

    // 2. Ollama models (async HTTP)
    {
        let guard = state.ollama_lock().read().await;
        if let Some(ollama) = guard.as_ref() {
            if ollama.is_reachable().await {
                match ollama.list_models().await {
                    Ok(ollama_models) => models.extend(ollama_models),
                    Err(e) => log::warn!("Ollama list_models failed: {}", e),
                }
            }
        }
    } // read lock released here

    Ok(models)
}

/// Generate a non-streaming completion.
///
/// Routes to the correct backend based on the model ID:
/// - IDs starting with "ollama:" -> Ollama HTTP API
/// - All others -> OS-native backend
#[command]
pub(crate) async fn generate<R: Runtime>(
    app: AppHandle<R>,
    payload: GenerateRequest,
) -> Result<GenerateResponse> {
    let state = app.llm();

    if payload.model.starts_with("ollama:") {
        let guard = state.ollama_lock().read().await;
        let ollama = guard.as_ref().ok_or(crate::Error::OllamaNotConfigured)?;
        ollama.generate(&payload).await
    } else {
        state.native().generate(payload)
    }
}

/// Hot-swap the Ollama connection at runtime.
///
/// Pass url: null to disable Ollama entirely.
/// Pass url: "..." to enable/reconfigure. api_key and timeout_secs are optional.
///
/// The write lock is held only for the instant it takes to swap the Option<OllamaBackend>.
#[command]
pub(crate) async fn configure_ollama<R: Runtime>(
    app: AppHandle<R>,
    settings: OllamaSettings,
) -> Result<()> {
    let state = app.llm();

    let new_backend = OllamaBackend::from_settings(&settings);

    // Write lock: exclusive, but the critical section is just a pointer swap
    *state.ollama_lock().write().await = new_backend;

    log::info!(
        "Ollama reconfigured: {}",
        match &settings.url {
            Some(url) => format!("url={}, has_key={}", url, settings.api_key.is_some()),
            None => "disabled".into(),
        }
    );

    Ok(())
}
