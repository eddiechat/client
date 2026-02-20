use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Model unavailable: {0}")]
    ModelUnavailable(String),

    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    #[error("Ollama error: {0}")]
    OllamaError(String),

    #[error("Ollama is not configured")]
    OllamaNotConfigured,

    #[error("Platform not supported for native inference")]
    Unsupported,

    #[error(transparent)]
    Tauri(#[from] tauri::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Ffi(#[from] std::ffi::NulError),
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
