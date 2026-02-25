use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum EddieError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("No active account")]
    NoActiveAccount,
}

// Tauri requires Serialize for command error types.
// Serialize as a plain string to maintain frontend compatibility.
impl Serialize for EddieError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<r2d2::Error> for EddieError {
    fn from(e: r2d2::Error) -> Self {
        EddieError::Database(e.to_string())
    }
}

impl From<rusqlite::Error> for EddieError {
    fn from(e: rusqlite::Error) -> Self {
        EddieError::Database(e.to_string())
    }
}
