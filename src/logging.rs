use std::{io, path::PathBuf, sync::OnceLock};

use tracing::Level;
use tracing_subscriber::fmt;

use crate::ui::state::AppToast;

pub const APP_LOG_HANDLE: &str = "h5v";

static LOGGER_INIT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
static SESSION_START_OFFSET: OnceLock<u64> = OnceLock::new();

pub fn initialize() -> Result<&'static PathBuf, String> {
    LOGGER_INIT
        .get_or_init(|| init_inner().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(Clone::clone)
}

pub fn log_path() -> Option<&'static PathBuf> {
    initialize().ok()
}

pub fn read_log_text(session_only: bool) -> Result<String, String> {
    let path = initialize()?;
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let start = if session_only {
        SESSION_START_OFFSET.get().copied().unwrap_or(0) as usize
    } else {
        0
    };
    Ok(String::from_utf8_lossy(&bytes[start.min(bytes.len())..]).into_owned())
}

pub fn log_error(kind: &'static str, message: impl AsRef<str>) {
    let message = message.as_ref();
    if initialize().is_ok() {
        tracing::error!(kind, handle = APP_LOG_HANDLE, message);
    }
}

pub fn log_toast(toast: &AppToast) {
    if initialize().is_err() {
        return;
    }
    match toast {
        AppToast::Empty => {}
        AppToast::Info(message) => {
            tracing::info!(
                kind = "toast",
                handle = APP_LOG_HANDLE,
                level = "info",
                message
            )
        }
        AppToast::Warning(message) => {
            tracing::warn!(
                kind = "toast",
                handle = APP_LOG_HANDLE,
                level = "warning",
                message
            )
        }
        AppToast::Error(message) => {
            tracing::error!(
                kind = "toast",
                handle = APP_LOG_HANDLE,
                level = "error",
                message
            )
        }
    }
}

pub fn log_lua(level: Level, message: impl AsRef<str>) {
    log_lua_with_handle(level, APP_LOG_HANDLE, message);
}

pub fn log_lua_with_handle(level: Level, handle: &str, message: impl AsRef<str>) {
    let message = message.as_ref();
    if initialize().is_err() {
        return;
    }
    match level {
        Level::ERROR => tracing::error!(kind = "lua", handle, message),
        Level::WARN => tracing::warn!(kind = "lua", handle, message),
        Level::INFO => tracing::info!(kind = "lua", handle, message),
        Level::DEBUG => tracing::debug!(kind = "lua", handle, message),
        Level::TRACE => tracing::trace!(kind = "lua", handle, message),
    }
}

fn init_inner() -> io::Result<PathBuf> {
    let cache_dir = dirs::cache_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Failed to resolve the h5v cache directory for logging",
        )
    })?;
    let log_dir = cache_dir.join("h5v");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("log.jsonl");
    let start_offset = std::fs::metadata(&log_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let _ = SESSION_START_OFFSET.set(start_offset);
    let appender = tracing_appender::rolling::never(&log_dir, "log.jsonl");
    let subscriber = fmt()
        .with_ansi(false)
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(appender)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|error| io::Error::other(format!("Failed to install logger: {error}")))?;
    tracing::info!(kind = "logging", message = "initialized", log_path = %log_path.display());
    Ok(log_path)
}
