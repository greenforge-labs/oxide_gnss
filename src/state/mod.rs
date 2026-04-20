//! State machine and supervisor infrastructure for oxide_gnss.
//!
//! This module implements the parallel supervisor pattern where Device and NTRIP
//! subsystems run as independent async tasks, coordinated by a central supervisor.

mod device_state;
mod integrity;
mod ntrip_state;
mod supervisor;

pub use device_state::DeviceState;
pub use integrity::{
    AntennaStatus, GnssIntegrity, IntegrityAggregator, IntegrityLevel, IntegrityThresholds,
};
pub use ntrip_state::NtripState;
pub use supervisor::{GgaData, GnssMessage, PvtData, Supervisor, SupervisorHandle};

use std::fmt;

/// Diagnostic level for ROS2 diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticLevel {
    /// Everything is operating normally
    Ok = 0,
    /// Non-critical issue, operation continues
    Warn = 1,
    /// Critical issue, functionality degraded
    Error = 2,
    /// System is in an unknown or stale state
    Stale = 3,
}

impl fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
            Self::Stale => write!(f, "STALE"),
        }
    }
}

/// GNSS fix type from the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixType {
    /// No fix available
    #[default]
    NoFix,
    /// Dead reckoning only
    DeadReckoning,
    /// 2D fix (no altitude)
    Fix2D,
    /// 3D fix
    Fix3D,
    /// 3D fix with GNSS + dead reckoning
    GnssDr,
    /// Time-only fix
    TimeOnly,
    /// RTK float solution
    RtkFloat,
    /// RTK fixed solution (highest accuracy)
    RtkFixed,
}

impl FixType {
    /// Get the diagnostic level for this fix type.
    pub fn diagnostic_level(&self) -> DiagnosticLevel {
        match self {
            Self::RtkFixed => DiagnosticLevel::Ok,
            Self::RtkFloat | Self::Fix3D | Self::GnssDr => DiagnosticLevel::Warn,
            Self::Fix2D | Self::DeadReckoning | Self::TimeOnly => DiagnosticLevel::Warn,
            Self::NoFix => DiagnosticLevel::Error,
        }
    }
}

impl fmt::Display for FixType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFix => write!(f, "No Fix"),
            Self::DeadReckoning => write!(f, "Dead Reckoning"),
            Self::Fix2D => write!(f, "2D Fix"),
            Self::Fix3D => write!(f, "3D Fix"),
            Self::GnssDr => write!(f, "GNSS+DR"),
            Self::TimeOnly => write!(f, "Time Only"),
            Self::RtkFloat => write!(f, "RTK Float"),
            Self::RtkFixed => write!(f, "RTK Fixed"),
        }
    }
}

/// Calculate aggregate diagnostic level from subsystem states.
pub fn aggregate_diagnostic_level(
    device: &DeviceState,
    ntrip: &NtripState,
    fix: FixType,
) -> DiagnosticLevel {
    let device_level = device.diagnostic_level();
    let ntrip_level = ntrip.diagnostic_level();
    let fix_level = fix.diagnostic_level();

    // Return the worst (highest) level
    device_level.max(ntrip_level).max(fix_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_level_ordering() {
        assert!(DiagnosticLevel::Ok < DiagnosticLevel::Warn);
        assert!(DiagnosticLevel::Warn < DiagnosticLevel::Error);
        assert!(DiagnosticLevel::Error < DiagnosticLevel::Stale);
    }

    #[test]
    fn test_aggregate_worst_of() {
        let device = DeviceState::Active;
        let ntrip = NtripState::Streaming;
        let fix = FixType::RtkFixed;

        assert_eq!(
            aggregate_diagnostic_level(&device, &ntrip, fix),
            DiagnosticLevel::Ok
        );

        let device_err = DeviceState::Waiting { reason: None };
        assert_eq!(
            aggregate_diagnostic_level(&device_err, &ntrip, fix),
            DiagnosticLevel::Error
        );
    }

    #[test]
    fn test_fix_type_levels() {
        assert_eq!(FixType::RtkFixed.diagnostic_level(), DiagnosticLevel::Ok);
        assert_eq!(FixType::RtkFloat.diagnostic_level(), DiagnosticLevel::Warn);
        assert_eq!(FixType::NoFix.diagnostic_level(), DiagnosticLevel::Error);
    }
}
