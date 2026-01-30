//! Business logic services
//!
//! This module contains Tauri-agnostic business logic that can be reused
//! across different contexts (CLI, tests, etc.).
//!
//! Services should:
//! - Not depend on Tauri types where possible
//! - Use EddieError for error handling
//! - Be easily testable in isolation

mod account_service;
mod helpers;
mod message_service;

pub use account_service::*;
pub use helpers::*;
pub use message_service::*;
