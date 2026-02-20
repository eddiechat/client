use serde::{Deserialize, Serialize};

/// A model available for inference.
/// Returned by list_models. The `id` field is what the caller passes to `generate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique identifier. Convention:
    ///   OS-native:  "apple-foundation-model", "gemini-nano", "phi-silica"
    ///   Ollama:     "ollama:<model-name>" e.g. "ollama:llama3.2:latest"
    pub id: String,

    /// Human-readable display name
    pub name: String,

    /// true if the model is ready for inference right now
    pub available: bool,

    /// If unavailable, a human-readable reason
    /// e.g. "Apple Intelligence not enabled", "Model needs download"
    pub reason: Option<String>,

    /// Backend that provides this model: "apple" | "android" | "windows" | "ollama"
    pub provider: String,

    /// Backend-specific metadata. Ollama includes family, parameter_size,
    /// quantization_level, size_bytes. OS-native backends may include version info.
    pub metadata: Option<serde_json::Value>,
}

/// Input to the `generate` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Model ID from list_models
    pub model: String,

    /// The prompt text
    pub prompt: String,

    /// Sampling temperature (0.0-2.0). Clamped by each backend to its supported range.
    pub temperature: f64,

    /// Max tokens to generate. Optional; each backend uses its own default if omitted.
    pub max_tokens: Option<u32>,
}

/// Output from the `generate` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    /// The generated text
    pub text: String,

    /// The model ID that produced it (echoed back, may be normalized)
    pub model: String,

    /// Backend that served it: "apple" | "android" | "windows" | "ollama"
    pub provider: String,
}

/// Input to the `configure_ollama` command.
/// Every field is optional - omitted fields keep their current value.
/// To disable Ollama entirely, pass url: null.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaSettings {
    /// Base URL of the Ollama server, e.g. "http://localhost:11434"
    /// or "https://my-proxy.example.com". Pass null to disable.
    pub url: Option<String>,

    /// API key sent as `Authorization: Bearer <key>` on every request.
    /// Supports reverse proxies (LiteLLM, OpenWebUI, nginx auth, etc.).
    /// Pass null or omit to send no auth header.
    pub api_key: Option<String>,

    /// HTTP timeout in seconds. Defaults to 120 if omitted.
    pub timeout_secs: Option<u64>,
}
