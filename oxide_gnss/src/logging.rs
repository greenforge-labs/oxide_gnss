//! Logging infrastructure for oxide_gnss.
//!
//! Uses the `tracing` ecosystem for structured, leveled logging.
//! Integrates with ROS2 logging conventions when running as a ROS2 node.

use tracing::Level;
use tracing_subscriber::{
    filter::EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
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
/// # Default Behavior
/// - Log level: INFO (override with RUST_LOG env var)
/// - Format: Compact with timestamps
/// - Output: stderr
///
/// # Environment Variables
/// - `RUST_LOG`: Set log level (e.g., "debug", "oxide_gnss=trace")
/// - `FERROUS_GNSS_LOG_STYLE`: Set format ("compact", "pretty")
///
/// # Example
/// ```no_run
/// oxide_gnss::logging::init();
/// tracing::info!("GNSS driver started");
/// ```
pub fn init() {
    init_with_level(Level::INFO);
}

/// Initialize the logging system with a specific default level.
///
/// The level can still be overridden by RUST_LOG environment variable.
pub fn init_with_level(default_level: Level) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "oxide_gnss={},warn",
            default_level.as_str().to_lowercase()
        ))
    });

    let log_style = std::env::var("FERROUS_GNSS_LOG_STYLE").unwrap_or_default();

    match log_style.as_str() {
        "pretty" => {
            // Pretty format for development
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .pretty()
                        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT),
                )
                .init();
        }
        _ => {
            // Compact format (default)
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().compact())
                .init();
        }
    }
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
