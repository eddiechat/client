//! Unified error types for the application
//!
//! This module defines error types that:
//! - Are serializable for frontend consumption
//! - Provide actionable error messages
//! - Map internal errors to user-friendly variants

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Application error type for commands and services
///
/// All errors are serializable so they can be sent to the frontend.
/// Error messages should be user-friendly and actionable.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum EddieError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("No active account configured")]
    NoActiveAccount,

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

    #[error("Database error: {0}")]
    Database(String),

    #[error("Credential error: {0}")]
    Credential(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Operation not supported: {0}")]
    NotSupported(String),

    #[error("Operation blocked: Eddie is in read-only mode. Enable write access in Settings to perform this action.")]
    ReadOnlyMode,

    #[error("{0}")]
    Other(String),
}

// Implement From for common error types

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

impl From<serde_json::Error> for EddieError {
    fn from(err: serde_json::Error) -> Self {
        EddieError::Parse(err.to_string())
    }
}

impl From<String> for EddieError {
    fn from(err: String) -> Self {
        EddieError::Other(err)
    }
}

impl From<&str> for EddieError {
    fn from(err: &str) -> Self {
        EddieError::Other(err.to_string())
    }
}

/// Result type alias using EddieError
pub type Result<T> = std::result::Result<T, EddieError>;
