//! u-blox device-specific configuration.
//!
//! Provides a structured configuration format for u-blox GNSS receivers,
//! organized into logical sections: rate, protocols, and messages.

use std::collections::HashMap;

use serde::Deserialize;
use tracing::warn;

/// u-blox device-specific configuration.
///
/// # Example
/// ```yaml
/// ublox:
///   family: "F9P"
///   rate:
///     measurement_ms: 100
///     nav_ratio: 1
///   protocols:
///     usb_in: [ubx, rtcm3x]
///     usb_out: [ubx]
///   messages:
///     usb:
///       NAV_PVT: 1
///       NAV_HPPOSLLH: 1
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UbloxConfig {
    /// Device family (F9P, F9R, F9H) - used for validation warnings
    #[serde(default)]
    pub family: Option<String>,

    /// Measurement/navigation rate settings
    #[serde(default)]
    pub rate: RateConfig,

    /// Protocol enable/disable settings per port
    #[serde(default)]
    pub protocols: ProtocolConfig,

    /// UBX message output rates per port
    #[serde(default)]
    pub messages: MessageConfig,
}

/// Measurement and navigation rate configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct RateConfig {
    /// Measurement rate in milliseconds (e.g., 100 = 10 Hz)
    #[serde(default = "default_measurement_ms")]
    pub measurement_ms: u16,

    /// Navigation solutions per measurement (typically 1)
    #[serde(default = "default_nav_ratio")]
    pub nav_ratio: u16,
}

fn default_measurement_ms() -> u16 {
    100
}
fn default_nav_ratio() -> u16 {
    1
}

impl Default for RateConfig {
    fn default() -> Self {
        Self {
            measurement_ms: 100,
            nav_ratio: 1,
        }
    }
}

/// Protocol configuration per port.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProtocolConfig {
    /// USB input protocols (ubx, nmea, rtcm3x)
    #[serde(default)]
    pub usb_in: Vec<String>,

    /// USB output protocols (ubx, nmea)
    #[serde(default)]
    pub usb_out: Vec<String>,

    /// UART1 input protocols
    #[serde(default)]
    pub uart1_in: Vec<String>,

    /// UART1 output protocols
    #[serde(default)]
    pub uart1_out: Vec<String>,
}

/// UBX message output configuration per port.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MessageConfig {
    /// USB message output rates
    #[serde(default)]
    pub usb: HashMap<String, u8>,

    /// UART1 message output rates
    #[serde(default)]
    pub uart1: HashMap<String, u8>,
}

/// Messages that are essential for core driver functionality.
pub const ESSENTIAL_MESSAGES: &[&str] = &["NAV_PVT"];

/// Messages that are recommended for integrity monitoring.
pub const RECOMMENDED_MESSAGES: &[&str] = &[
    "NAV_HPPOSLLH",
    "MON_RF",
    "MON_COMMS",
    "SEC_SIG",
];


impl UbloxConfig {
    /// Check if any configuration is set.
    pub fn has_config(&self) -> bool {
        !self.messages.usb.is_empty()
            || !self.messages.uart1.is_empty()
            || !self.protocols.usb_in.is_empty()
            || !self.protocols.usb_out.is_empty()
    }

    /// Validate configuration and emit warnings for issues.
    pub fn validate(&self) {
        // Check for essential messages
        let has_nav_pvt = self.messages.usb.contains_key("NAV_PVT")
            || self.messages.uart1.contains_key("NAV_PVT");
        if !has_nav_pvt {
            warn!(
                "NAV_PVT message not enabled - this is required for position/velocity output"
            );
        }
    }

    /// Get the total number of message configurations.
    pub fn message_count(&self) -> usize {
        self.messages.usb.len() + self.messages.uart1.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ublox_config() {
        let yaml = r#"
family: "F9P"
rate:
  measurement_ms: 100
  nav_ratio: 1
protocols:
  usb_in: [ubx, rtcm3x]
  usb_out: [ubx]
messages:
  usb:
    NAV_PVT: 1
    NAV_HPPOSLLH: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.family, Some("F9P".to_string()));
        assert_eq!(config.rate.measurement_ms, 100);
        assert_eq!(config.rate.nav_ratio, 1);
        assert_eq!(config.protocols.usb_in.len(), 2);
        assert_eq!(config.messages.usb.len(), 2);
        assert_eq!(config.messages.usb.get("NAV_PVT"), Some(&1));
    }

    #[test]
    fn test_empty_config() {
        let yaml = r#"{}"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.has_config());
    }

    #[test]
    fn test_default_rates() {
        let config = RateConfig::default();
        assert_eq!(config.measurement_ms, 100);
        assert_eq!(config.nav_ratio, 1);
    }

    #[test]
    fn test_message_count() {
        let yaml = r#"
messages:
  usb:
    NAV_PVT: 1
    NAV_SAT: 5
  uart1:
    NAV_PVT: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.message_count(), 3);
    }
}
