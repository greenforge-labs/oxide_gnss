//! # oxide_gnss
//!
//! A Rust-based ROS2 GNSS driver for u-blox ZED-F9P receivers with integrated NTRIP client.
//!
//! ## Overview
//!
//! oxide_gnss provides a reliable, safety-focused GNSS driver for ROS2 applications.
//! It connects to u-blox ZED-F9P GNSS receivers via serial port, parses UBX protocol
//! messages, and publishes position, velocity, and diagnostic information to ROS2 topics.
//!
//! ## Key Features
//!
//! - **Integrated NTRIP client** for RTK corrections with TLS/HTTPS support
//! - **Safety integrity monitoring** with configurable thresholds and go/no-go signal
//! - **Mode-based configuration** for common scenarios (rover, base, moving base)
//! - **Comprehensive diagnostics** including jamming/spoofing detection
//! - **Multi-device support** via configurable namespaces
//! - **ENU/NED coordinate frames** for velocity output
//!
//! ## Architecture
//!
//! The driver uses an async task model coordinated by a [`state::Supervisor`]:
//!
//! ```text
//! DeviceTask (serial) ──→ Supervisor (routing) ──→ RosTask (publish)
//!                               ↑
//!                         NtripTask (corrections)
//! ```
//!
//! - **DeviceTask** reads UBX packets from the serial port and parses them
//! - **NtripTask** connects to NTRIP casters for RTK corrections
//! - **RosTask** converts messages to ROS types and publishes
//! - **Supervisor** manages channels, shared state, and shutdown coordination
//!
//! ## Modules
//!
//! - [`config`] - YAML configuration loading and validation
//! - [`device`] - Serial communication and UBX protocol parsing
//! - [`error`] - Error types for all subsystems
//! - [`ntrip`] - NTRIP client for RTK corrections
//! - [`state`] - Supervisor, state machines, and integrity monitoring
//! - [`transform`] - Coordinate frame transformations
//! - [`ros`] - ROS2 publishers and node (requires `ros2` feature)
//!
//! ## Configuration
//!
//! See the [configuration documentation](https://github.com/gsokoll/oxide_gnss/blob/master/docs/CONFIGURATION.md)
//! for full details on YAML configuration options.
//!
//! ## ROS2 Topics
//!
//! Core topics (always published):
//! - `~/fix` ([`sensor_msgs/NavSatFix`]) - Position with covariance
//! - `~/velocity` ([`geometry_msgs/TwistWithCovarianceStamped`]) - 3D velocity
//! - `~/time_reference` ([`sensor_msgs/TimeReference`]) - GPS timestamp
//! - `/diagnostics` ([`diagnostic_msgs/DiagnosticArray`]) - System health
//!
//! Optional topics (based on mode/features):
//! - `~/integrity` - Safety integrity status
//! - `~/operational` - Go/no-go signal
//! - `~/satellites` - Per-satellite visibility

/// Configuration loading and validation.
///
/// This module handles:
/// - YAML file parsing with environment variable substitution
/// - Mode-based configuration (rover_ntrip, standalone, moving_base, etc.)
/// - Feature flags (high_precision, integrity, satellites)
/// - Validation of configuration consistency
pub mod config;

/// GNSS device communication and UBX protocol.
///
/// This module provides:
/// - Serial port communication with the GNSS receiver
/// - UBX protocol message parsing ([`device::UbxHandler`])
/// - Device configuration via UBX-CFG-VALSET commands
/// - Parsed data structures for position, velocity, satellites, etc.
pub mod device;

/// Error types for all subsystems.
///
/// Provides a hierarchy of error types:
/// - [`error::DeviceError`] - Serial port and device communication
/// - [`error::ProtocolError`] - UBX/RTCM protocol parsing
/// - [`error::NtripError`] - NTRIP client operations
pub mod error;

/// Structured logging categories and utilities.
pub mod logging;

/// NTRIP client for RTK corrections.
///
/// Features:
/// - Automatic connection with exponential backoff
/// - TLS/HTTPS support for secure connections
/// - GGA position reporting for VRS mountpoints
/// - Configurable reconnection behavior
pub mod ntrip;

/// State management and coordination.
///
/// Provides:
/// - [`state::Supervisor`] for coordinating async tasks
/// - [`state::GnssIntegrity`] for safety integrity computation
/// - Device and NTRIP state machines
pub mod state;

/// Coordinate frame transformations (NED ↔ ENU).
pub mod transform;

/// Shared utility functions.
pub mod util;

/// ROS2 interface (publishers, subscribers, node).
///
/// Only available when compiled with the `ros2` feature.
#[cfg(feature = "ros2")]
pub mod ros;

/// Library version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
