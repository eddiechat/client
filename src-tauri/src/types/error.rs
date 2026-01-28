use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum EddieError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Folder not found: {0}")]
    FolderNotFound(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for EddieError {
    fn from(err: std::io::Error) -> Self {
        EddieError::Io(err.to_string())
    }
}

impl From<toml::de::Error> for EddieError {
    fn from(err: toml::de::Error) -> Self {
        EddieError::Config(err.to_string())
    }
}
