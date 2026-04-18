//! Device configuration for GNSS receivers.

use serde::Deserialize;

use super::ublox::UbloxConfig;
use super::ConfigError;

/// Coordinate frame for velocity output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CoordinateFrame {
    /// East-North-Up (ROS convention)
    #[default]
    ENU,
    /// North-East-Down (aviation/marine convention)
    NED,
}

/// GNSS device configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceConfig {
    /// Serial port path (e.g., "/dev/ttyACM0" or "COM3")
    pub port: String,

    /// Baud rate for serial communication
    #[serde(default = "default_baud_rate")]
    pub baud_rate: u32,

    /// Coordinate frame for velocity output
    #[serde(default)]
    pub frame: CoordinateFrame,

    /// Navigation settings
    #[serde(default)]
    pub navigation: NavigationConfig,

    /// Reconnection settings
    #[serde(default)]
    pub reconnect: ReconnectConfig,

    /// u-blox specific configuration (optional)
    /// Uses CFG_* key names matching u-blox documentation.
    #[serde(default)]
    pub ublox: Option<UbloxConfig>,

    /// Serial read watchdog timeout in seconds.
    /// If no data is received within this period, the device is considered
    /// disconnected and reconnection logic is triggered.
    #[serde(default = "default_watchdog_timeout")]
    pub watchdog_timeout_secs: f64,

    /// Packet-level watchdog in seconds.
    /// If no non-empty serial read completes within this period while in Active,
    /// mark the device disconnected and trigger reconnect. Complements
    /// watchdog_timeout_secs, which only catches "read never returns" — this one
    /// catches "read keeps returning zero bytes" (stuck firmware, silenced
    /// message output, or USB-CDC EOF from a newer kernel).
    #[serde(default = "default_packet_watchdog_secs")]
    pub packet_watchdog_secs: f64,
}

/// Navigation update configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct NavigationConfig {
    /// Navigation solution rate in Hz (1-25 for ZED-F9P)
    #[serde(default = "default_nav_rate")]
    pub rate_hz: u8,

    /// Minimum satellites required for valid fix
    #[serde(default = "default_min_satellites")]
    pub min_satellites: u8,

    /// Maximum acceptable HDOP
    #[serde(default = "default_max_hdop")]
    pub max_hdop: f32,

    /// Maximum acceptable PDOP
    #[serde(default = "default_max_pdop")]
    pub max_pdop: f32,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self {
            rate_hz: default_nav_rate(),
            min_satellites: default_min_satellites(),
            max_hdop: default_max_hdop(),
            max_pdop: default_max_pdop(),
        }
    }
}

/// Reconnection behavior configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ReconnectConfig {
    /// Enable automatic reconnection
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Initial delay before first reconnection attempt (seconds)
    #[serde(default = "default_initial_delay")]
    pub initial_delay_secs: u32,

    /// Maximum delay between reconnection attempts (seconds)
    #[serde(default = "default_max_delay")]
    pub max_delay_secs: u32,

    /// Maximum number of reconnection attempts (0 = unlimited)
    #[serde(default)]
    pub max_attempts: u32,

    /// Time in seconds of continuous successful operation after which the
    /// reconnect backoff is reset (0 = never reset).
    #[serde(default = "default_backoff_reset_secs")]
    pub backoff_reset_secs: u32,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay_secs: default_initial_delay(),
            max_delay_secs: default_max_delay(),
            max_attempts: 0,
            backoff_reset_secs: default_backoff_reset_secs(),
        }
    }
}

impl DeviceConfig {
    /// Validate the device configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.port.is_empty() {
            return Err(ConfigError::Validation {
                message: "Device port cannot be empty".to_string(),
            });
        }

        if self.baud_rate == 0 {
            return Err(ConfigError::Validation {
                message: "Baud rate must be greater than 0".to_string(),
            });
        }

        if self.navigation.rate_hz == 0 || self.navigation.rate_hz > 25 {
            return Err(ConfigError::Validation {
                message: "Navigation rate must be between 1 and 25 Hz".to_string(),
            });
        }

        if self.navigation.max_hdop <= 0.0 {
            return Err(ConfigError::Validation {
                message: "max_hdop must be greater than 0".to_string(),
            });
        }

        if self.navigation.max_pdop <= 0.0 {
            return Err(ConfigError::Validation {
                message: "max_pdop must be greater than 0".to_string(),
            });
        }

        if self.watchdog_timeout_secs <= 0.0 {
            return Err(ConfigError::Validation {
                message: "watchdog_timeout_secs must be greater than 0".to_string(),
            });
        }

        if self.packet_watchdog_secs <= 0.0 {
            return Err(ConfigError::Validation {
                message: "packet_watchdog_secs must be greater than 0".to_string(),
            });
        }


        Ok(())
    }
}

// Default value functions for serde
fn default_baud_rate() -> u32 {
    460800
}

fn default_nav_rate() -> u8 {
    10
}

fn default_min_satellites() -> u8 {
    4
}

fn default_max_hdop() -> f32 {
    5.0
}

fn default_max_pdop() -> f32 {
    10.0
}

fn default_true() -> bool {
    true
}

fn default_initial_delay() -> u32 {
    1
}

fn default_max_delay() -> u32 {
    30
}

fn default_backoff_reset_secs() -> u32 {
    300 // 5 minutes of stable operation resets backoff
}

fn default_watchdog_timeout() -> f64 {
    5.0
}

fn default_packet_watchdog_secs() -> f64 {
    3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let yaml = r#"port: "/dev/ttyACM0""#;
        let config: DeviceConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.port, "/dev/ttyACM0");
        assert_eq!(config.baud_rate, 460800);
        assert_eq!(config.frame, CoordinateFrame::ENU);
        assert_eq!(config.navigation.rate_hz, 10);
        assert_eq!(config.watchdog_timeout_secs, 5.0);
        assert_eq!(config.packet_watchdog_secs, 3.0);
    }

    #[test]
    fn test_validation_empty_port() {
        let yaml = r#"port: """#;
        let config: DeviceConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_rate() {
        let yaml = r#"
port: "/dev/ttyACM0"
navigation:
  rate_hz: 30
"#;
        let config: DeviceConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_rejects_non_positive_packet_watchdog() {
        let yaml = r#"
port: "/dev/ttyACM0"
packet_watchdog_secs: 0.0
"#;
        let config: DeviceConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_packet_watchdog_override() {
        let yaml = r#"
port: "/dev/ttyACM0"
packet_watchdog_secs: 1.5
"#;
        let config: DeviceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.packet_watchdog_secs, 1.5);
        assert!(config.validate().is_ok());
    }
}
