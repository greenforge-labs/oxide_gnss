//! Logging infrastructure for oxide_gnss.
//!
//! Uses the `tracing` ecosystem for structured, leveled logging.
//! Dual-layer output: stderr (for terminal) and file (for persistence).

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
    Layer,
};

/// Log categories for different subsystems.
pub mod category {
    /// Device communication logging
    pub const DEVICE: &str = "oxide_gnss::device";
    /// NTRIP client logging
    pub const NTRIP: &str = "oxide_gnss::ntrip";
    /// Protocol parsing logging
    pub const PROTOCOL: &str = "oxide_gnss::protocol";
    /// ROS2 interface logging
    pub const ROS: &str = "oxide_gnss::ros";
    /// Configuration logging
    pub const CONFIG: &str = "oxide_gnss::config";
    /// State machine logging
    pub const STATE: &str = "oxide_gnss::state";
}

/// Initialize the logging system with default configuration.
///
/// Returns a `WorkerGuard` that must be held alive for the duration of the program.
/// When the guard is dropped, the file writer flushes remaining buffered logs.
///
/// # Default Behavior
/// - Stderr: INFO level, compact format (override with RUST_LOG)
/// - File: INFO globally, daily rotation (override with OXIDE_GNSS_FILE_LOG)
/// - Log directory: OXIDE_GNSS_LOG_DIR env → ~/.ros/log/oxide_gnss/ → /tmp/oxide_gnss/
///
/// # Environment Variables
/// - `RUST_LOG`: Set stderr log level (e.g., "debug", "oxide_gnss=trace")
/// - `OXIDE_GNSS_LOG_STYLE`: Set stderr format ("compact", "pretty")
/// - `OXIDE_GNSS_LOG_DIR`: Override log file directory
/// - `OXIDE_GNSS_FILE_LOG`: Set file log level (e.g., "oxide_gnss=trace,info")
///
/// # Example
/// ```no_run
/// let _guard = oxide_gnss::logging::init();
/// tracing::info!("GNSS driver started");
/// ```
pub fn init() -> WorkerGuard {
    init_with_level(Level::INFO)
}

/// Initialize the logging system with a specific default level.
///
/// The level can still be overridden by RUST_LOG environment variable.
pub fn init_with_level(default_level: Level) -> WorkerGuard {
    // Stderr filter (existing behavior)
    let stderr_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "oxide_gnss={},warn",
            default_level.as_str().to_lowercase()
        ))
    });

    // File filter defaults to INFO; set OXIDE_GNSS_FILE_LOG=oxide_gnss=debug to re-enable
    // debug-level events. Per-epoch debug logs on the device/ROS/NTRIP hot paths add up
    // through tracing dispatch, message format, the crossbeam channel to the non-blocking
    // appender, and the futex_wake to its thread; keep them off by default.
    // See ferrous_gnss commit 95a2fd9 / docs/PROFILING.md for the investigation.
    let file_filter =
        EnvFilter::try_from_env("OXIDE_GNSS_FILE_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    // Resolve log directory
    let log_dir = resolve_log_dir();

    // Create rolling daily file appender
    let file_appender = tracing_appender::rolling::daily(&log_dir, "oxide_gnss.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // File layer: full format, no ANSI, thread names
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_thread_names(true)
        .with_writer(non_blocking)
        .with_filter(file_filter);

    // Stderr layer: compact or pretty based on env
    let log_style = std::env::var("OXIDE_GNSS_LOG_STYLE").unwrap_or_default();
    let stderr_layer = match log_style.as_str() {
        "pretty" => fmt::layer()
            .pretty()
            .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
            .with_filter(stderr_filter)
            .boxed(),
        _ => fmt::layer().compact().with_filter(stderr_filter).boxed(),
    };

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();

    tracing::info!("Log files: {}", log_dir.display());

    guard
}

/// Resolve the log file directory, creating it if necessary.
fn resolve_log_dir() -> std::path::PathBuf {
    // 1. Check env var
    if let Ok(dir) = std::env::var("OXIDE_GNSS_LOG_DIR") {
        let path = std::path::PathBuf::from(dir);
        if ensure_dir(&path) {
            return path;
        }
    }

    // 2. Try ~/.ros/log/oxide_gnss/
    if let Some(home) = std::env::var_os("HOME") {
        let path = std::path::PathBuf::from(home)
            .join(".ros")
            .join("log")
            .join("oxide_gnss");
        if ensure_dir(&path) {
            return path;
        }
    }

    // 3. Fallback to /tmp/oxide_gnss/
    let path = std::path::PathBuf::from("/tmp/oxide_gnss");
    let _ = std::fs::create_dir_all(&path);
    path
}

/// Attempt to create a directory, return true if it exists or was created.
fn ensure_dir(path: &std::path::Path) -> bool {
    std::fs::create_dir_all(path).is_ok()
}

/// Log a state transition for the device state machine.
#[macro_export]
macro_rules! log_state_transition {
    ($from:expr, $to:expr) => {
        tracing::info!(
            target: $crate::logging::category::STATE,
            from = %$from,
            to = %$to,
            "State transition"
        );
    };
    ($from:expr, $to:expr, $reason:expr) => {
        tracing::info!(
            target: $crate::logging::category::STATE,
            from = %$from,
            to = %$to,
            reason = %$reason,
            "State transition"
        );
    };
}

/// Log a device event with structured fields.
#[macro_export]
macro_rules! log_device_event {
    ($event:expr, $($field:tt)*) => {
        tracing::info!(
            target: $crate::logging::category::DEVICE,
            event = $event,
            $($field)*
        );
    };
}

/// Log an NTRIP event with structured fields.
#[macro_export]
macro_rules! log_ntrip_event {
    ($event:expr, $($field:tt)*) => {
        tracing::info!(
            target: $crate::logging::category::NTRIP,
            event = $event,
            $($field)*
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_constants() {
        assert!(category::DEVICE.starts_with("oxide_gnss::"));
        assert!(category::NTRIP.starts_with("oxide_gnss::"));
        assert!(category::PROTOCOL.starts_with("oxide_gnss::"));
    }
}
