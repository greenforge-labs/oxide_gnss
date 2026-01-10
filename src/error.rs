//! Error types for oxide_gnss.
//!
//! This module defines a hierarchy of error types for different subsystems:
//! - [`DeviceError`] - Serial port and device communication errors
//! - [`ProtocolError`] - UBX and RTCM protocol parsing errors
//! - [`NtripError`] - NTRIP client connection and streaming errors
//!
//! The [`Error`] enum provides a unified error type for the library.

use thiserror::Error;

/// Unified error type for the oxide_gnss library.
#[derive(Debug, Error)]
pub enum Error {
    /// Configuration error
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),

    /// Device communication error
    #[error(transparent)]
    Device(#[from] DeviceError),

    /// Protocol parsing error
    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    /// NTRIP client error
    #[error(transparent)]
    Ntrip(#[from] NtripError),
}

/// Result type alias using the library's unified error type.
pub type Result<T> = std::result::Result<T, Error>;

// ============================================================================
// Device Errors
// ============================================================================

/// Errors related to GNSS device communication.
#[derive(Debug, Error)]
pub enum DeviceError {
    /// Serial port not found or inaccessible
    #[error("Serial port '{port}' not found or inaccessible: {source}")]
    PortNotFound {
        port: String,
        #[source]
        source: std::io::Error,
    },

    /// Failed to open serial port
    #[error("Failed to open serial port '{port}': {source}")]
    OpenFailed {
        port: String,
        #[source]
        source: std::io::Error,
    },

    /// Failed to configure serial port settings (baud rate, etc.)
    #[error("Failed to configure serial port '{port}': {source}")]
    SerialConfigFailed {
        port: String,
        #[source]
        source: std::io::Error,
    },

    /// Serial port read error
    #[error("Read error on '{port}': {source}")]
    ReadError {
        port: String,
        #[source]
        source: std::io::Error,
    },

    /// Serial port write error
    #[error("Write error on '{port}': {source}")]
    WriteError {
        port: String,
        #[source]
        source: std::io::Error,
    },

    /// Device disconnected unexpectedly
    #[error("Device disconnected: {port}")]
    Disconnected { port: String },

    /// Timeout waiting for device response
    #[error("Timeout waiting {timeout_ms}ms for {operation}")]
    Timeout {
        /// The operation that timed out
        operation: String,
        /// Timeout duration in milliseconds
        timeout_ms: u64,
    },

    /// Device configuration failed (UBX config not acknowledged)
    #[error("Device rejected configuration for {config_key}: {reason}")]
    ConfigRejected {
        /// The configuration key that was rejected (e.g., "CFG-RATE-MEAS")
        config_key: String,
        /// Reason for rejection
        reason: String,
    },

    /// Device returned unexpected response
    #[error("Unexpected device response: {message}")]
    UnexpectedResponse { message: String },

    /// Configuration step failed after retries
    #[error("Configuration failed at step '{step}' after retries")]
    ConfigurationFailed { step: String },
}

// ============================================================================
// Protocol Errors
// ============================================================================

/// Errors related to GNSS protocol parsing (UBX, RTCM, NMEA).
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Invalid UBX message header
    #[error("Invalid UBX header: expected 0xB5 0x62, got {got:02X?}")]
    InvalidUbxHeader { got: [u8; 2] },

    /// UBX checksum mismatch
    #[error(
        "UBX checksum mismatch for {class:#04X}/{id:#04X}: expected {expected:04X}, got {got:04X}"
    )]
    UbxChecksumMismatch {
        class: u8,
        id: u8,
        expected: u16,
        got: u16,
    },

    /// UBX message too short
    #[error("UBX message too short: expected {expected} bytes, got {got}")]
    UbxMessageTooShort { expected: usize, got: usize },

    /// Unknown UBX message class/id
    #[error("Unknown UBX message: class={class:#04X}, id={id:#04X}")]
    UnknownUbxMessage { class: u8, id: u8 },

    /// Failed to parse UBX message payload
    #[error("Failed to parse UBX {class:#04X}/{id:#04X} payload: {message}")]
    UbxPayloadError { class: u8, id: u8, message: String },

    /// Invalid RTCM frame
    #[error("Invalid RTCM frame: {message}")]
    InvalidRtcmFrame { message: String },

    /// RTCM CRC mismatch
    #[error("RTCM CRC mismatch for message {msg_type}: expected {expected:06X}, got {got:06X}")]
    RtcmCrcMismatch {
        msg_type: u16,
        expected: u32,
        got: u32,
    },

    /// Unknown RTCM message type
    #[error("Unknown RTCM message type: {msg_type}")]
    UnknownRtcmMessage { msg_type: u16 },

    /// Invalid NMEA sentence
    #[error("Invalid NMEA sentence: {message}")]
    InvalidNmea { message: String },

    /// Buffer overflow (message too large)
    #[error("Protocol buffer overflow: message size {size} exceeds maximum {max}")]
    BufferOverflow { size: usize, max: usize },

    /// Incomplete message in buffer
    #[error("Incomplete message: need {needed} more bytes")]
    IncompleteMessage { needed: usize },
}

// ============================================================================
// NTRIP Errors
// ============================================================================

/// Errors related to NTRIP client operations.
#[derive(Debug, Error)]
pub enum NtripError {
    /// Failed to connect to NTRIP caster
    #[error("Failed to connect to NTRIP caster at {host}:{port}: {source}")]
    ConnectionFailed {
        host: String,
        port: u16,
        #[source]
        source: std::io::Error,
    },

    /// Authentication failed
    #[error("NTRIP authentication failed for user '{username}' at {host}")]
    AuthenticationFailed { host: String, username: String },

    /// Mountpoint not found
    #[error("Mountpoint '{mountpoint}' not found on caster {host}")]
    MountpointNotFound { host: String, mountpoint: String },

    /// HTTP error response from caster
    #[error("NTRIP caster returned HTTP {status}: {message}")]
    HttpError { status: u16, message: String },

    /// Connection timed out
    #[error("NTRIP connection timed out after {timeout_secs} seconds")]
    Timeout { timeout_secs: u32 },

    /// Read timed out - no data received within configured period
    #[error("NTRIP read timed out after {timeout_secs} seconds - no data received")]
    ReadTimeout { timeout_secs: u32 },

    /// Stream disconnected
    #[error("NTRIP stream disconnected: {reason}")]
    StreamDisconnected { reason: String },

    /// Failed to send GGA position
    #[error("Failed to send GGA position to caster: {source}")]
    GgaSendFailed {
        #[source]
        source: std::io::Error,
    },

    /// Invalid sourcetable response
    #[error("Invalid NTRIP sourcetable: {message}")]
    InvalidSourcetable { message: String },

    /// URL parsing error
    #[error("Invalid NTRIP URL: {url}")]
    InvalidUrl { url: String },

    /// Network I/O error
    #[error("NTRIP network error: {source}")]
    NetworkError {
        #[source]
        source: std::io::Error,
    },

    /// TLS/HTTPS error
    #[error("NTRIP TLS error: {message}")]
    TlsError { message: String },

    /// Invalid configuration
    #[error("NTRIP configuration error: {message}")]
    InvalidConfig { message: String },
}

impl From<ntrip_core::Error> for NtripError {
    fn from(err: ntrip_core::Error) -> Self {
        match err {
            ntrip_core::Error::ConnectionFailed { host, port, source } => {
                Self::ConnectionFailed { host, port, source }
            }
            ntrip_core::Error::Timeout { timeout_secs } => Self::Timeout { timeout_secs },
            ntrip_core::Error::ReadTimeout { timeout_secs } => Self::ReadTimeout { timeout_secs },
            ntrip_core::Error::AuthenticationFailed { host, username } => {
                Self::AuthenticationFailed { host, username }
            }
            ntrip_core::Error::MountpointNotFound { host, mountpoint } => {
                Self::MountpointNotFound { host, mountpoint }
            }
            ntrip_core::Error::HttpError {
                host: _,
                status_code,
                reason,
            } => Self::HttpError {
                status: status_code,
                message: reason,
            },
            ntrip_core::Error::TlsError { message } => Self::TlsError { message },
            ntrip_core::Error::NetworkError { source } => Self::NetworkError { source },
            ntrip_core::Error::StreamDisconnected { reason } => Self::StreamDisconnected { reason },
            ntrip_core::Error::InvalidConfig { message } => Self::InvalidConfig { message },
            ntrip_core::Error::SourcetableParseError { message } => {
                Self::InvalidSourcetable { message }
            }
            // Handle any future variants added to ntrip_core::Error
            #[allow(unreachable_patterns)]
            _ => Self::NetworkError {
                source: std::io::Error::other(format!("Unknown ntrip-core error: {}", err)),
            },
        }
    }
}

// ============================================================================
// Convenience constructors
// ============================================================================

impl DeviceError {
    /// Create a port not found error.
    pub fn port_not_found(port: impl Into<String>, source: std::io::Error) -> Self {
        Self::PortNotFound {
            port: port.into(),
            source,
        }
    }

    /// Create a disconnected error.
    pub fn disconnected(port: impl Into<String>) -> Self {
        Self::Disconnected { port: port.into() }
    }

    /// Create a timeout error.
    pub fn timeout(operation: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            timeout_ms,
        }
    }

    /// Create a config rejected error.
    pub fn config_rejected(config_key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ConfigRejected {
            config_key: config_key.into(),
            reason: reason.into(),
        }
    }

    /// Create a configuration failed error.
    pub fn configuration_failed(step: impl Into<String>) -> Self {
        Self::ConfigurationFailed { step: step.into() }
    }
}

impl NtripError {
    /// Create a connection failed error.
    pub fn connection_failed(host: impl Into<String>, port: u16, source: std::io::Error) -> Self {
        Self::ConnectionFailed {
            host: host.into(),
            port,
            source,
        }
    }

    /// Create an authentication failed error.
    pub fn auth_failed(host: impl Into<String>, username: impl Into<String>) -> Self {
        Self::AuthenticationFailed {
            host: host.into(),
            username: username.into(),
        }
    }

    /// Create a mountpoint not found error.
    pub fn mountpoint_not_found(host: impl Into<String>, mountpoint: impl Into<String>) -> Self {
        Self::MountpointNotFound {
            host: host.into(),
            mountpoint: mountpoint.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_error_display() {
        let err = DeviceError::disconnected("/dev/ttyACM0");
        assert!(err.to_string().contains("/dev/ttyACM0"));
    }

    #[test]
    fn test_protocol_error_display() {
        let err = ProtocolError::UbxChecksumMismatch {
            class: 0x01,
            id: 0x07,
            expected: 0x1234,
            got: 0x5678,
        };
        assert!(err.to_string().contains("0x01"));
        assert!(err.to_string().contains("0x07"));
    }

    #[test]
    fn test_error_conversion() {
        let device_err = DeviceError::disconnected("/dev/ttyACM0");
        let unified: Error = device_err.into();
        assert!(matches!(unified, Error::Device(_)));
    }

    #[test]
    fn test_timeout_error_with_context() {
        let err = DeviceError::timeout("waiting for ACK", 500);
        let msg = err.to_string();
        assert!(msg.contains("500"));
        assert!(msg.contains("waiting for ACK"));
    }

    #[test]
    fn test_config_rejected_error_with_context() {
        let err = DeviceError::config_rejected("CFG-RATE-MEAS", "value out of range");
        let msg = err.to_string();
        assert!(msg.contains("CFG-RATE-MEAS"));
        assert!(msg.contains("value out of range"));
    }

    #[test]
    fn test_ntrip_error_constructors() {
        let err = NtripError::auth_failed("caster.example.com", "testuser");
        let msg = err.to_string();
        assert!(msg.contains("caster.example.com"));
        assert!(msg.contains("testuser"));

        let err = NtripError::mountpoint_not_found("caster.example.com", "TESTMOUNT");
        let msg = err.to_string();
        assert!(msg.contains("TESTMOUNT"));
    }

    #[test]
    fn test_protocol_error_variants() {
        let err = ProtocolError::InvalidUbxHeader { got: [0x00, 0x00] };
        assert!(err.to_string().contains("Invalid UBX header"));

        let err = ProtocolError::UbxMessageTooShort {
            expected: 100,
            got: 50,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));

        let err = ProtocolError::BufferOverflow {
            size: 10000,
            max: 8192,
        };
        assert!(err.to_string().contains("10000"));
        assert!(err.to_string().contains("8192"));
    }

    #[test]
    fn test_ntrip_error_variants() {
        let err = NtripError::Timeout { timeout_secs: 30 };
        assert!(err.to_string().contains("30"));

        let err = NtripError::ReadTimeout { timeout_secs: 60 };
        assert!(err.to_string().contains("60"));

        let err = NtripError::HttpError {
            status: 503,
            message: "Service Unavailable".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("503"));
        assert!(msg.contains("Service Unavailable"));
    }
}
