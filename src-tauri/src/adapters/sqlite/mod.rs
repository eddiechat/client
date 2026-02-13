pub mod pool;
pub mod schema;
pub mod messages;
pub mod accounts;
pub mod entities;
pub mod conversations;
pub mod onboarding_tasks;
pub mod folder_sync;

// Re-export the pool type so callers can do `use crate::db::DbPool`
// instead of `use crate::db::pool::DbPool`
pub use pool::DbPool;