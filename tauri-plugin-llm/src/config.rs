use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Initial Ollama URL. Default: "http://localhost:11434".
    /// Set to null in config to start with Ollama disabled.
    #[serde(default = "default_ollama_url")]
    pub ollama_url: Option<String>,

    /// Initial API key. Default: None (no auth).
    #[serde(default)]
    pub ollama_api_key: Option<String>,

    /// HTTP timeout in seconds. Default: 120.
    #[serde(default = "default_timeout")]
    pub ollama_timeout_secs: u64,
}

fn default_ollama_url() -> Option<String> {
    Some("http://localhost:11434".into())
}

fn default_timeout() -> u64 {
    120
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            ollama_url: default_ollama_url(),
            ollama_api_key: None,
            ollama_timeout_secs: default_timeout(),
        }
    }
}
