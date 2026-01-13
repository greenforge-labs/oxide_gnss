//! NTRIP client state machine.

use std::fmt;

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

    /// Read timeout - no data received within configured period.
    TimedOut,

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
            Self::Connecting | Self::Backoff { .. } | Self::TimedOut => DiagnosticLevel::Warn,
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
            Self::TimedOut => "TimedOut",
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
            Self::TimedOut => write!(f, "Timed Out"),
            Self::ShuttingDown => write!(f, "Shutting Down"),
        }
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
}
