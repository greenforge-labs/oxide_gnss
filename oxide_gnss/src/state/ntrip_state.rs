//! NTRIP client state machine.

use std::fmt;
use std::time::Instant;

use super::DiagnosticLevel;

/// State of the NTRIP client connection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NtripState {
    /// NTRIP is not configured/disabled.
    #[default]
    Disabled,

    /// Attempting to connect to the NTRIP caster.
    Connecting,

    /// Connected and receiving RTCM correction stream.
    Streaming,

    /// Connection failed, waiting before retry.
    Backoff {
        /// Number of retry attempts so far
        attempt: u32,
        /// Delay in seconds before next retry
        delay_secs: u32,
        /// Reason for backoff
        reason: String,
    },

    /// Shutting down gracefully.
    ShuttingDown,
}

impl NtripState {
    /// Create a new Backoff state.
    pub fn backoff(attempt: u32, delay_secs: u32, reason: impl Into<String>) -> Self {
        Self::Backoff {
            attempt,
            delay_secs,
            reason: reason.into(),
        }
    }

    /// Check if NTRIP is actively providing corrections.
    pub fn is_streaming(&self) -> bool {
        matches!(self, Self::Streaming)
    }

    /// Check if NTRIP is enabled (not disabled).
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// Check if attempting to connect.
    pub fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting | Self::Backoff { .. })
    }

    /// Get the diagnostic level for this state.
    pub fn diagnostic_level(&self) -> DiagnosticLevel {
        match self {
            Self::Disabled | Self::Streaming => DiagnosticLevel::Ok,
            Self::Connecting | Self::Backoff { .. } => DiagnosticLevel::Warn,
            Self::ShuttingDown => DiagnosticLevel::Stale,
        }
    }

    /// Get a short name for the state (for logging/diagnostics).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Connecting => "Connecting",
            Self::Streaming => "Streaming",
            Self::Backoff { .. } => "Backoff",
            Self::ShuttingDown => "ShuttingDown",
        }
    }
}

impl fmt::Display for NtripState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => write!(f, "Disabled"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Streaming => write!(f, "Streaming"),
            Self::Backoff {
                attempt,
                delay_secs,
                reason,
            } => write!(
                f,
                "Backoff (attempt {}, {}s delay, {})",
                attempt, delay_secs, reason
            ),
            Self::ShuttingDown => write!(f, "Shutting Down"),
        }
    }
}

/// Statistics for NTRIP connection.
#[allow(dead_code)] // Will be used when NTRIP client is implemented
#[derive(Debug, Clone, Default)]
pub struct NtripStats {
    /// Total bytes received from NTRIP stream
    pub bytes_received: u64,
    /// Number of RTCM messages received
    pub messages_received: u64,
    /// Time of last received message
    pub last_message_time: Option<Instant>,
    /// Number of successful connections
    pub connection_count: u32,
    /// Number of connection failures
    pub failure_count: u32,
}

#[allow(dead_code)] // Will be used when NTRIP client is implemented
impl NtripStats {
    /// Create new empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record bytes received.
    pub fn record_bytes(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.last_message_time = Some(Instant::now());
    }

    /// Record a message received.
    pub fn record_message(&mut self) {
        self.messages_received += 1;
        self.last_message_time = Some(Instant::now());
    }

    /// Record a successful connection.
    pub fn record_connection(&mut self) {
        self.connection_count += 1;
    }

    /// Record a connection failure.
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
    }

    /// Get age of last message in seconds, if any.
    pub fn message_age_secs(&self) -> Option<f64> {
        self.last_message_time.map(|t| t.elapsed().as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = NtripState::default();
        assert!(matches!(state, NtripState::Disabled));
    }

    #[test]
    fn test_is_streaming() {
        assert!(NtripState::Streaming.is_streaming());
        assert!(!NtripState::Connecting.is_streaming());
        assert!(!NtripState::Disabled.is_streaming());
    }

    #[test]
    fn test_diagnostic_levels() {
        assert_eq!(NtripState::Disabled.diagnostic_level(), DiagnosticLevel::Ok);
        assert_eq!(
            NtripState::Streaming.diagnostic_level(),
            DiagnosticLevel::Ok
        );
        assert_eq!(
            NtripState::Connecting.diagnostic_level(),
            DiagnosticLevel::Warn
        );
    }

    #[test]
    fn test_display() {
        let state = NtripState::backoff(2, 30, "connection timeout");
        assert!(state.to_string().contains("attempt 2"));
        assert!(state.to_string().contains("30s"));
    }

    #[test]
    fn test_stats() {
        let mut stats = NtripStats::new();
        stats.record_bytes(1024);
        stats.record_message();
        assert_eq!(stats.bytes_received, 1024);
        assert_eq!(stats.messages_received, 1);
        assert!(stats.message_age_secs().is_some());
    }
}
