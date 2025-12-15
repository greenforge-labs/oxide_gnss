//! Device state machine for GNSS receiver communication.

use std::fmt;

use super::DiagnosticLevel;

/// State of the GNSS device connection and operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceState {
    /// Waiting for device to become available.
    Waiting {
        /// Optional reason for waiting (e.g., "port not found")
        reason: Option<String>,
    },

    /// Attempting to open and connect to the serial port.
    Connecting,

    /// Sending configuration commands to the device.
    Configuring {
        /// Current configuration step (for progress tracking)
        step: u8,
        /// Total configuration steps
        total_steps: u8,
    },

    /// Device is active and receiving data normally.
    Active,

    /// Lost connection, attempting to reconnect.
    Reconnecting {
        /// Number of reconnection attempts so far
        attempt: u32,
        /// Maximum attempts (0 = unlimited)
        max_attempts: u32,
        /// Reason for disconnection
        reason: String,
    },

    /// Shutting down gracefully.
    ShuttingDown,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self::Waiting { reason: None }
    }
}

impl DeviceState {
    /// Create a new Waiting state with a reason.
    pub fn waiting(reason: impl Into<String>) -> Self {
        Self::Waiting {
            reason: Some(reason.into()),
        }
    }

    /// Create a new Configuring state.
    pub fn configuring(step: u8, total_steps: u8) -> Self {
        Self::Configuring { step, total_steps }
    }

    /// Create a new Reconnecting state.
    pub fn reconnecting(attempt: u32, max_attempts: u32, reason: impl Into<String>) -> Self {
        Self::Reconnecting {
            attempt,
            max_attempts,
            reason: reason.into(),
        }
    }

    /// Check if the device is in an operational state.
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Active | Self::Configuring { .. })
    }

    /// Check if the device is trying to connect.
    pub fn is_connecting(&self) -> bool {
        matches!(
            self,
            Self::Connecting | Self::Reconnecting { .. } | Self::Waiting { .. }
        )
    }

    /// Get the diagnostic level for this state.
    pub fn diagnostic_level(&self) -> DiagnosticLevel {
        match self {
            Self::Active => DiagnosticLevel::Ok,
            Self::Configuring { .. } | Self::Connecting | Self::Reconnecting { .. } => {
                DiagnosticLevel::Warn
            }
            Self::Waiting { .. } => DiagnosticLevel::Error,
            Self::ShuttingDown => DiagnosticLevel::Stale,
        }
    }

    /// Get a short name for the state (for logging/diagnostics).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Waiting { .. } => "Waiting",
            Self::Connecting => "Connecting",
            Self::Configuring { .. } => "Configuring",
            Self::Active => "Active",
            Self::Reconnecting { .. } => "Reconnecting",
            Self::ShuttingDown => "ShuttingDown",
        }
    }
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Waiting { reason: Some(r) } => write!(f, "Waiting ({})", r),
            Self::Waiting { reason: None } => write!(f, "Waiting"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Configuring { step, total_steps } => {
                write!(f, "Configuring ({}/{})", step, total_steps)
            }
            Self::Active => write!(f, "Active"),
            Self::Reconnecting {
                attempt,
                max_attempts,
                reason,
            } => {
                if *max_attempts == 0 {
                    write!(f, "Reconnecting (attempt {}, {})", attempt, reason)
                } else {
                    write!(
                        f,
                        "Reconnecting (attempt {}/{}, {})",
                        attempt, max_attempts, reason
                    )
                }
            }
            Self::ShuttingDown => write!(f, "Shutting Down"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = DeviceState::default();
        assert!(matches!(state, DeviceState::Waiting { reason: None }));
    }

    #[test]
    fn test_is_operational() {
        assert!(DeviceState::Active.is_operational());
        assert!(DeviceState::configuring(1, 5).is_operational());
        assert!(!DeviceState::Connecting.is_operational());
    }

    #[test]
    fn test_diagnostic_levels() {
        assert_eq!(DeviceState::Active.diagnostic_level(), DiagnosticLevel::Ok);
        assert_eq!(
            DeviceState::Connecting.diagnostic_level(),
            DiagnosticLevel::Warn
        );
        assert_eq!(
            DeviceState::Waiting { reason: None }.diagnostic_level(),
            DiagnosticLevel::Error
        );
    }

    #[test]
    fn test_display() {
        let state = DeviceState::reconnecting(3, 10, "USB disconnected");
        assert!(state.to_string().contains("3/10"));
        assert!(state.to_string().contains("USB disconnected"));
    }
}
