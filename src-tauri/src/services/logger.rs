use std::sync::{OnceLock, RwLock};

use crate::adapters::sqlite::sync::DbPool;

static LOGGER: OnceLock<Logger> = OnceLock::new();

struct Logger {
    log_source: RwLock<String>,
    host: RwLock<String>,
    environment: String,
}

pub fn init(pool: &DbPool) {
    let emails = crate::adapters::sqlite::sync::accounts::list_account_emails(pool)
        .unwrap_or_default();

    let log_source = if emails.is_empty() {
        "unknown".into()
    } else {
        emails.join(", ")
    };

    let environment = if cfg!(debug_assertions) {
        "development"
    } else {
        "test"
    };

    let host = crate::adapters::sqlite::sync::accounts::get_first_imap_host(pool)
        .unwrap_or_default()
        .unwrap_or_else(|| "unknown".into());

    let _ = LOGGER.set(Logger {
        log_source: RwLock::new(log_source),
        host: RwLock::new(host),
        environment: environment.into(),
    });
}

pub fn set_source(email: &str) {
    if let Some(l) = LOGGER.get() {
        if let Ok(mut source) = l.log_source.write() {
            *source = email.to_string();
        }
    }
}

pub fn set_host(hostname: &str) {
    if let Some(l) = LOGGER.get() {
        if let Ok(mut h) = l.host.write() {
            *h = hostname.to_string();
        }
    }
}

fn get() -> &'static Logger {
    LOGGER.get().expect("Logger not initialized — call logger::init() first")
}

pub fn fmt_ms(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms == 0 { "<1ms".into() } else { format!("{}ms", ms) }
}

pub fn debug(message: &str) {
    let l = get();
    let source = l.log_source.read().unwrap();
    let host = l.host.read().unwrap();
    sentry::logger_debug!(
        log.source = source.as_str(),
        host = host.as_str(),
        environment = l.environment.as_str(),
        "{}", message
    );
    println!("DEBUG {}", message);
}

pub fn info(message: &str) {
    let l = get();
    let source = l.log_source.read().unwrap();
    let host = l.host.read().unwrap();
    sentry::logger_info!(
        log.source = source.as_str(),
        host = host.as_str(),
        environment = l.environment.as_str(),
        "{}", message
    );
    println!("INFO {}", message);
}

pub fn warn(message: &str) {
    let l = get();
    let source = l.log_source.read().unwrap();
    let host = l.host.read().unwrap();
    sentry::logger_warn!(
        log.source = source.as_str(),
        host = host.as_str(),
        environment = l.environment.as_str(),
        "{}", message
    );
    println!("WARN {}", message);
}

pub fn error(message: &str) {
    let l = get();
    let source = l.log_source.read().unwrap();
    let host = l.host.read().unwrap();
    sentry::logger_error!(
        log.source = source.as_str(),
        host = host.as_str(),
        environment = l.environment.as_str(),
        "{}", message
    );
    println!("ERROR {}", message);
}
