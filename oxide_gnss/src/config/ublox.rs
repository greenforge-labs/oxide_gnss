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

    /// Port-specific settings (baudrate, enabled)
    #[serde(default)]
    pub ports: PortSettings,

    /// GNSS signal/constellation configuration
    #[serde(default)]
    pub signals: SignalConfig,
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

    /// USB output protocols (ubx, nmea, rtcm3x)
    #[serde(default)]
    pub usb_out: Vec<String>,

    /// UART1 input protocols
    #[serde(default)]
    pub uart1_in: Vec<String>,

    /// UART1 output protocols
    #[serde(default)]
    pub uart1_out: Vec<String>,

    /// UART2 input protocols
    #[serde(default)]
    pub uart2_in: Vec<String>,

    /// UART2 output protocols
    #[serde(default)]
    pub uart2_out: Vec<String>,

    /// I2C input protocols
    #[serde(default)]
    pub i2c_in: Vec<String>,

    /// I2C output protocols
    #[serde(default)]
    pub i2c_out: Vec<String>,
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

    /// UART2 message output rates
    #[serde(default)]
    pub uart2: HashMap<String, u8>,
}

/// Port-specific settings (baudrate, enabled state).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PortSettings {
    /// UART1 settings
    #[serde(default)]
    pub uart1: UartPortConfig,

    /// UART2 settings
    #[serde(default)]
    pub uart2: UartPortConfig,
}

/// UART port configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UartPortConfig {
    /// Baudrate (e.g., 460800, 115200, 38400)
    #[serde(default)]
    pub baudrate: Option<u32>,

    /// Whether the port is enabled
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// GNSS signal/constellation configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SignalConfig {
    /// GPS configuration
    #[serde(default)]
    pub gps: GnssConstellationConfig,

    /// GLONASS configuration
    #[serde(default)]
    pub glonass: GnssConstellationConfig,

    /// Galileo configuration
    #[serde(default)]
    pub galileo: GnssConstellationConfig,

    /// BeiDou configuration
    #[serde(default)]
    pub beidou: BeidouConfig,

    /// SBAS configuration
    #[serde(default)]
    pub sbas: SbasConfig,

    /// QZSS configuration
    #[serde(default)]
    pub qzss: QzssConfig,
}

/// Generic GNSS constellation config (GPS, GLONASS, Galileo).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GnssConstellationConfig {
    /// Enable this constellation
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Enable L1 signal (L1C/A for GPS, L1OF for GLONASS, E1 for Galileo)
    #[serde(default)]
    pub l1: Option<bool>,

    /// Enable L2 signal (L2C for GPS, L2OF for GLONASS, E5b for Galileo)
    #[serde(default)]
    pub l2: Option<bool>,
}

/// BeiDou-specific configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BeidouConfig {
    /// Enable BeiDou constellation
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Enable B1 signal
    #[serde(default)]
    pub b1: Option<bool>,

    /// Enable B2 signal
    #[serde(default)]
    pub b2: Option<bool>,
}

/// SBAS configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SbasConfig {
    /// Enable SBAS
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Enable L1 signal
    #[serde(default)]
    pub l1: Option<bool>,
}

/// QZSS configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct QzssConfig {
    /// Enable QZSS
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Enable L1C/A signal
    #[serde(default)]
    pub l1ca: Option<bool>,

    /// Enable L1S signal
    #[serde(default)]
    pub l1s: Option<bool>,

    /// Enable L2C signal
    #[serde(default)]
    pub l2c: Option<bool>,
}

/// Message requirement level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    /// Required for core functionality - driver may not work correctly without this
    Essential,
    /// Recommended for full functionality - some features will be degraded without this
    Recommended,
    /// Required to enable a specific feature/output topic
    RequiredForFeature,
    /// Optional - provides additional data but not required
    Optional,
}

/// Metadata about a UBX message requirement.
#[derive(Debug, Clone)]
pub struct MessageRequirement {
    /// UBX message name (e.g., "NAV_PVT")
    pub message: &'static str,
    /// Requirement level
    pub level: MessageLevel,
    /// Human-readable reason why this message is needed
    pub reason: &'static str,
    /// ROS topics that depend on this message
    pub ros_topics: &'static [&'static str],
}

/// All message requirements with their dependencies.
pub const MESSAGE_REQUIREMENTS: &[MessageRequirement] = &[
    // Essential messages - driver won't work properly without these
    MessageRequirement {
        message: "NAV_PVT",
        level: MessageLevel::Essential,
        reason: "Primary position/velocity/time solution",
        ros_topics: &["~/fix", "~/velocity", "~/time_reference"],
    },
    // Recommended messages - important for full functionality
    MessageRequirement {
        message: "NAV_HPPOSLLH",
        level: MessageLevel::Recommended,
        reason: "High-precision position (mm-level when RTK fixed)",
        ros_topics: &["~/hp_pos"],
    },
    MessageRequirement {
        message: "NAV_POSECEF",
        level: MessageLevel::Recommended,
        reason: "ECEF coordinates for coordinate transforms",
        ros_topics: &["~/fix"],
    },
    MessageRequirement {
        message: "MON_RF",
        level: MessageLevel::RequiredForFeature,
        reason: "Required for ~/integrity (antenna status, jamming indicator)",
        ros_topics: &["~/integrity", "~/diagnostics"],
    },
    MessageRequirement {
        message: "MON_COMMS",
        level: MessageLevel::RequiredForFeature,
        reason: "Required for ~/integrity (communication port health)",
        ros_topics: &["~/integrity", "~/diagnostics"],
    },
    MessageRequirement {
        message: "SEC_SIG",
        level: MessageLevel::RequiredForFeature,
        reason: "Required for ~/integrity (jamming/spoofing detection)",
        ros_topics: &["~/integrity", "~/sec_sig_details"],
    },
    // Optional messages - nice to have
    MessageRequirement {
        message: "NAV_SAT",
        level: MessageLevel::Optional,
        reason: "Per-satellite signal info (can be high bandwidth)",
        ros_topics: &["~/satellites"],
    },
    MessageRequirement {
        message: "NAV_RELPOSNED",
        level: MessageLevel::RequiredForFeature,
        reason: "Required for ~/baseline_pose output (moving base/rover RTK)",
        ros_topics: &["~/baseline_pose"],
    },
    MessageRequirement {
        message: "NAV_COV",
        level: MessageLevel::Optional,
        reason: "Position covariance matrix (F9R/F9H only, not F9P)",
        ros_topics: &[],
    },
    MessageRequirement {
        message: "SEC_SIGLOG",
        level: MessageLevel::Optional,
        reason: "Security event log for forensic analysis",
        ros_topics: &[],
    },
];

/// Get all messages at a given requirement level.
pub fn messages_at_level(level: MessageLevel) -> impl Iterator<Item = &'static MessageRequirement> {
    MESSAGE_REQUIREMENTS
        .iter()
        .filter(move |m| m.level == level)
}

/// Get the requirement for a specific message, if known.
pub fn get_message_requirement(message: &str) -> Option<&'static MessageRequirement> {
    MESSAGE_REQUIREMENTS.iter().find(|m| m.message == message)
}

/// Get all messages required for a specific ROS topic.
pub fn messages_for_topic(topic: &str) -> Vec<&'static MessageRequirement> {
    MESSAGE_REQUIREMENTS
        .iter()
        .filter(|m| m.ros_topics.contains(&topic))
        .collect()
}

impl UbloxConfig {
    /// Check if any configuration is set.
    pub fn has_config(&self) -> bool {
        !self.messages.usb.is_empty()
            || !self.messages.uart1.is_empty()
            || !self.protocols.usb_in.is_empty()
            || !self.protocols.usb_out.is_empty()
    }

    /// Check if a message is enabled on any port with rate > 0.
    pub fn is_message_enabled(&self, message: &str) -> bool {
        let usb_rate = self.messages.usb.get(message).copied().unwrap_or(0);
        let uart1_rate = self.messages.uart1.get(message).copied().unwrap_or(0);
        let uart2_rate = self.messages.uart2.get(message).copied().unwrap_or(0);
        usb_rate > 0 || uart1_rate > 0 || uart2_rate > 0
    }

    /// Validate configuration and emit warnings for issues.
    ///
    /// Returns a ValidationResult with details about missing messages.
    pub fn validate(&self) -> ValidationResult {
        use tracing::{error, info};

        let mut result = ValidationResult::default();

        for req in MESSAGE_REQUIREMENTS {
            let enabled = self.is_message_enabled(req.message);

            if !enabled {
                match req.level {
                    MessageLevel::Essential => {
                        error!(
                            message = req.message,
                            reason = req.reason,
                            topics = ?req.ros_topics,
                            "ESSENTIAL message not enabled - driver may not function correctly"
                        );
                        result.missing_essential.push(req);
                    }
                    MessageLevel::Recommended => {
                        warn!(
                            message = req.message,
                            reason = req.reason,
                            topics = ?req.ros_topics,
                            "Recommended message not enabled - some features will be unavailable"
                        );
                        result.missing_recommended.push(req);
                    }
                    MessageLevel::RequiredForFeature => {
                        info!(
                            message = req.message,
                            reason = req.reason,
                            topics = ?req.ros_topics,
                            "Feature-required message not enabled - topic(s) will not publish"
                        );
                        result.missing_required_for_feature.push(req);
                    }
                    MessageLevel::Optional => {
                        // Don't warn for optional, just note in result
                        result.missing_optional.push(req);
                    }
                }
            }
        }

        // Summary log
        if !result.missing_essential.is_empty() {
            error!(
                "Configuration validation: {} essential message(s) missing!",
                result.missing_essential.len()
            );
        } else if !result.missing_recommended.is_empty() {
            warn!(
                "Configuration validation: {} recommended message(s) missing",
                result.missing_recommended.len()
            );
        } else {
            info!("Configuration validation: all essential and recommended messages enabled");
        }

        result
    }

    /// Check which ROS topics will be unavailable due to missing messages.
    pub fn check_topic_availability(&self) -> Vec<(&'static str, Vec<&'static str>)> {
        let mut unavailable = Vec::new();

        // Collect all topics and their missing dependencies
        let all_topics: std::collections::HashSet<&str> = MESSAGE_REQUIREMENTS
            .iter()
            .flat_map(|m| m.ros_topics.iter().copied())
            .collect();

        for topic in all_topics {
            let missing: Vec<&'static str> = messages_for_topic(topic)
                .into_iter()
                .filter(|req| !self.is_message_enabled(req.message))
                .map(|req| req.message)
                .collect();

            if !missing.is_empty() {
                unavailable.push((topic, missing));
            }
        }

        unavailable
    }

    /// Get the total number of message configurations.
    pub fn message_count(&self) -> usize {
        self.messages.usb.len() + self.messages.uart1.len()
    }
}

/// Result of configuration validation.
#[derive(Debug, Default)]
pub struct ValidationResult {
    /// Essential messages that are missing (driver may malfunction)
    pub missing_essential: Vec<&'static MessageRequirement>,
    /// Recommended messages that are missing (reduced functionality)
    pub missing_recommended: Vec<&'static MessageRequirement>,
    /// Feature-required messages that are missing (specific topics won't publish)
    pub missing_required_for_feature: Vec<&'static MessageRequirement>,
    /// Optional messages that are missing (informational only)
    pub missing_optional: Vec<&'static MessageRequirement>,
}

impl ValidationResult {
    /// Returns true if all essential messages are present.
    pub fn is_valid(&self) -> bool {
        self.missing_essential.is_empty()
    }

    /// Returns true if all essential and recommended messages are present.
    pub fn is_complete(&self) -> bool {
        self.missing_essential.is_empty() && self.missing_recommended.is_empty()
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

    #[test]
    fn test_validation_missing_essential() {
        let yaml = r#"
messages:
  usb:
    NAV_HPPOSLLH: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(!result.is_valid());
        assert_eq!(result.missing_essential.len(), 1);
        assert_eq!(result.missing_essential[0].message, "NAV_PVT");
    }

    #[test]
    fn test_validation_complete_config() {
        let yaml = r#"
messages:
  usb:
    NAV_PVT: 1
    NAV_HPPOSLLH: 1
    NAV_POSECEF: 1
    MON_RF: 1
    MON_COMMS: 1
    SEC_SIG: 1
    NAV_SAT: 5
    SEC_SIGLOG: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_valid());
        assert!(result.is_complete());
    }

    #[test]
    fn test_validation_missing_recommended() {
        let yaml = r#"
messages:
  usb:
    NAV_PVT: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_valid());
        assert!(!result.is_complete());
        assert!(!result.missing_recommended.is_empty());
    }

    #[test]
    fn test_is_message_enabled() {
        let yaml = r#"
messages:
  usb:
    NAV_PVT: 1
    NAV_SAT: 0
  uart1:
    MON_RF: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.is_message_enabled("NAV_PVT"));
        assert!(!config.is_message_enabled("NAV_SAT")); // rate 0 = disabled
        assert!(config.is_message_enabled("MON_RF"));
        assert!(!config.is_message_enabled("NAV_HPPOSLLH"));
    }

    #[test]
    fn test_topic_availability() {
        let yaml = r#"
messages:
  usb:
    NAV_PVT: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        let unavailable = config.check_topic_availability();
        // ~/hp_pos should be unavailable (needs NAV_HPPOSLLH)
        let hp_pos = unavailable.iter().find(|(t, _)| *t == "~/hp_pos");
        assert!(hp_pos.is_some());
        assert!(hp_pos.unwrap().1.contains(&"NAV_HPPOSLLH"));
    }

    #[test]
    fn test_messages_for_topic() {
        let msgs = messages_for_topic("~/fix");
        assert!(msgs.iter().any(|m| m.message == "NAV_PVT"));
    }

    #[test]
    fn test_messages_at_level() {
        let essential: Vec<_> = messages_at_level(MessageLevel::Essential).collect();
        assert_eq!(essential.len(), 1);
        assert_eq!(essential[0].message, "NAV_PVT");
    }
}
