//! Shared helper functions
//!
//! Common utilities used across services and commands.

use crate::sync::db::{get_active_connection_config, init_config_db};
use crate::types::error::{EddieError, Result};

/// Resolve account ID from optional parameter or active account
///
/// If an account ID is provided, returns it directly.
/// Otherwise, returns the currently active account ID.
pub fn resolve_account_id(account: Option<&str>) -> Result<String> {
    if let Some(id) = account {
        Ok(id.to_string())
    } else {
        init_config_db()?;
        let active_config = get_active_connection_config()?;
        active_config
            .map(|c| c.account_id)
            .ok_or(EddieError::NoActiveAccount)
    }
}

/// Resolve account ID from optional String parameter
pub fn resolve_account_id_string(account: Option<String>) -> Result<String> {
    resolve_account_id(account.as_deref())
}
