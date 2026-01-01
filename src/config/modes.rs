//! Operating modes and feature definitions for oxide_gnss.
//!
//! This module provides high-level configuration presets that abstract away
//! low-level u-blox UBX message configuration.

use serde::Deserialize;
use std::collections::HashMap;

/// Operating mode for the GNSS device.
///
/// Modes are mutually exclusive and define the primary role of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatingMode {
    /// Basic GPS, no RTK corrections
    #[default]
    Standalone,
    /// RTK rover with NTRIP corrections via network
    RoverNtrip,
    /// RTK rover with corrections via radio/serial on UART2
    RoverRadio,
    /// Moving base in MB+R pair (outputs RTCM on UART2)
    MovingBase,
    /// Rover in MB+R pair (receives RTCM from base on UART2)
    MovingBaseRover,
    /// Static base station (survey-in or fixed coords)
    StaticBase,
}

impl OperatingMode {
    /// Get the human-readable name of this mode.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Standalone => "standalone",
            Self::RoverNtrip => "rover_ntrip",
            Self::RoverRadio => "rover_radio",
            Self::MovingBase => "moving_base",
            Self::MovingBaseRover => "moving_base_rover",
            Self::StaticBase => "static_base",
        }
    }

    /// Get a description of this mode.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Standalone => "Basic GPS without RTK corrections",
            Self::RoverNtrip => "RTK rover with NTRIP corrections via network",
            Self::RoverRadio => "RTK rover with corrections via radio/serial",
            Self::MovingBase => "Moving base in MB+R pair",
            Self::MovingBaseRover => "Rover in MB+R pair",
            Self::StaticBase => "Static base station",
        }
    }

    /// Get the preset configuration for this mode.
    pub fn preset(&self) -> ModePreset {
        match self {
            Self::Standalone => presets::standalone(),
            Self::RoverNtrip => presets::rover_ntrip(),
            Self::RoverRadio => presets::rover_radio(),
            Self::MovingBase => presets::moving_base(),
            Self::MovingBaseRover => presets::moving_base_rover(),
            Self::StaticBase => presets::static_base(),
        }
    }

    /// Check if a feature is allowed for this mode.
    pub fn allows_feature(&self, feature: Feature) -> bool {
        self.preset().allowed_features.contains(&feature)
    }

    /// Check if this mode requires NTRIP configuration.
    pub fn requires_ntrip(&self) -> bool {
        matches!(self, Self::RoverNtrip)
    }

    /// Check if this mode optionally uses NTRIP.
    pub fn supports_ntrip(&self) -> bool {
        matches!(self, Self::RoverNtrip | Self::MovingBase)
    }
}

impl std::fmt::Display for OperatingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Optional features that can be enabled in addition to mode defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// Use high-precision position for ~/fix
    HighPrecision,
    /// Enable integrity monitoring (jamming/spoofing detection)
    Integrity,
    /// Enable per-satellite signal info
    Satellites,
    /// Enable heading from baseline (MB+R only)
    Heading,
    /// Enable dead reckoning (F9R only)
    DeadReckoning,
}

impl Feature {
    /// Get the human-readable name of this feature.
    pub fn name(&self) -> &'static str {
        match self {
            Self::HighPrecision => "high_precision",
            Self::Integrity => "integrity",
            Self::Satellites => "satellites",
            Self::Heading => "heading",
            Self::DeadReckoning => "dead_reckoning",
        }
    }

    /// Get a description of this feature.
    pub fn description(&self) -> &'static str {
        match self {
            Self::HighPrecision => "High-precision position for ~/fix topic",
            Self::Integrity => "Integrity monitoring (jamming/spoofing detection)",
            Self::Satellites => "Per-satellite signal information",
            Self::Heading => "Heading from baseline vector (moving base + rover)",
            Self::DeadReckoning => "Dead reckoning sensor fusion (F9R only)",
        }
    }

    /// Get the UBX messages required for this feature.
    pub fn required_messages(&self) -> &'static [&'static str] {
        match self {
            Self::HighPrecision => &["NAV_HPPOSLLH"],
            // NAV_SAT required for C/N0 signal quality checks
            // NAV_PL required for protection level reporting
            Self::Integrity => &["SEC_SIG", "MON_RF", "MON_COMMS", "NAV_SAT", "NAV_PL"],
            Self::Satellites => &["NAV_SAT"],
            Self::Heading => &["NAV_RELPOSNED"],
            Self::DeadReckoning => &[], // ESF messages - not yet implemented
        }
    }

    /// Get the ROS topics enabled by this feature.
    pub fn enabled_topics(&self) -> &'static [&'static str] {
        match self {
            Self::HighPrecision => &["~/fix (HP)"],
            Self::Integrity => &["~/integrity", "~/operational"],
            Self::Satellites => &["~/satellites"],
            Self::Heading => &["~/baseline_pose"],
            Self::DeadReckoning => &[],
        }
    }
}

impl std::fmt::Display for Feature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Feature configuration from YAML.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeaturesConfig {
    /// Enable high-precision position for ~/fix
    #[serde(default)]
    pub high_precision: bool,

    /// Enable integrity monitoring
    #[serde(default)]
    pub integrity: bool,

    /// Enable per-satellite info
    #[serde(default)]
    pub satellites: bool,

    /// Enable heading from baseline (MB+R only)
    #[serde(default)]
    pub heading: bool,

    /// Enable dead reckoning (F9R only)
    #[serde(default)]
    pub dead_reckoning: bool,
}

impl FeaturesConfig {
    /// Get the list of enabled features.
    pub fn enabled_features(&self) -> Vec<Feature> {
        let mut features = Vec::new();
        if self.high_precision {
            features.push(Feature::HighPrecision);
        }
        if self.integrity {
            features.push(Feature::Integrity);
        }
        if self.satellites {
            features.push(Feature::Satellites);
        }
        if self.heading {
            features.push(Feature::Heading);
        }
        if self.dead_reckoning {
            features.push(Feature::DeadReckoning);
        }
        features
    }

    /// Check if any features are enabled.
    pub fn has_features(&self) -> bool {
        self.high_precision
            || self.integrity
            || self.satellites
            || self.heading
            || self.dead_reckoning
    }
}

/// Protocol configuration for a port.
#[derive(Debug, Clone, Default)]
pub struct PortProtocols {
    pub ubx_in: bool,
    pub ubx_out: bool,
    pub nmea_in: bool,
    pub nmea_out: bool,
    pub rtcm3x_in: bool,
    pub rtcm3x_out: bool,
}

/// Port enable settings for a mode.
#[derive(Debug, Clone, Default)]
pub struct PortEnables {
    /// UART1 port enabled (None = don't configure, keep device default)
    pub uart1: Option<bool>,
    /// UART2 port enabled
    pub uart2: Option<bool>,
    /// I2C (DDC) port enabled - leave enabled but disable protocols to avoid firmware quirks
    pub i2c: Option<bool>,
    /// SPI port enabled
    pub spi: Option<bool>,
}

/// Mode preset containing all configuration for a mode.
#[derive(Debug, Clone)]
pub struct ModePreset {
    /// Port enable/disable settings
    pub port_enables: PortEnables,
    /// USB port protocol configuration
    pub usb: PortProtocols,
    /// UART1 port protocol configuration
    pub uart1: PortProtocols,
    /// UART2 port protocol configuration
    pub uart2: PortProtocols,
    /// I2C (DDC) port protocol configuration
    pub i2c: PortProtocols,
    /// SPI port protocol configuration
    pub spi: PortProtocols,
    /// Base UBX messages (always enabled for this mode)
    pub base_messages: Vec<(&'static str, u8)>,
    /// RTCM messages to output (for base modes)
    pub rtcm_output_uart2: Vec<&'static str>,
    /// Features allowed for this mode
    pub allowed_features: Vec<Feature>,
}

impl ModePreset {
    /// Get USB input protocols as a list of strings.
    pub fn usb_in_protocols(&self) -> Vec<String> {
        let mut protos = Vec::new();
        if self.usb.ubx_in {
            protos.push("ubx".to_string());
        }
        if self.usb.nmea_in {
            protos.push("nmea".to_string());
        }
        if self.usb.rtcm3x_in {
            protos.push("rtcm3x".to_string());
        }
        protos
    }

    /// Get USB output protocols as a list of strings.
    pub fn usb_out_protocols(&self) -> Vec<String> {
        let mut protos = Vec::new();
        if self.usb.ubx_out {
            protos.push("ubx".to_string());
        }
        if self.usb.nmea_out {
            protos.push("nmea".to_string());
        }
        if self.usb.rtcm3x_out {
            protos.push("rtcm3x".to_string());
        }
        protos
    }

    /// Get UART2 input protocols as a list of strings.
    pub fn uart2_in_protocols(&self) -> Vec<String> {
        let mut protos = Vec::new();
        if self.uart2.ubx_in {
            protos.push("ubx".to_string());
        }
        if self.uart2.nmea_in {
            protos.push("nmea".to_string());
        }
        if self.uart2.rtcm3x_in {
            protos.push("rtcm3x".to_string());
        }
        protos
    }

    /// Get UART2 output protocols as a list of strings.
    pub fn uart2_out_protocols(&self) -> Vec<String> {
        let mut protos = Vec::new();
        if self.uart2.ubx_out {
            protos.push("ubx".to_string());
        }
        if self.uart2.nmea_out {
            protos.push("nmea".to_string());
        }
        if self.uart2.rtcm3x_out {
            protos.push("rtcm3x".to_string());
        }
        protos
    }

    /// Convert base messages to a HashMap for UBX config.
    pub fn messages_as_map(&self) -> HashMap<String, u8> {
        self.base_messages
            .iter()
            .map(|(name, rate)| (name.to_string(), *rate))
            .collect()
    }
}

/// Preset definitions for each mode.
pub mod presets {
    use super::*;

    /// Standalone mode - basic GPS without RTK.
    /// Disables unused ports/protocols to reduce CPU load.
    pub fn standalone() -> ModePreset {
        ModePreset {
            port_enables: PortEnables {
                uart1: Some(false),
                uart2: Some(false),
                // Leave I2C enabled but disable protocols (firmware quirk)
                i2c: None,
                spi: Some(false),
            },
            usb: PortProtocols {
                ubx_in: true,
                ubx_out: true,
                ..Default::default()
            },
            uart1: PortProtocols::default(),
            uart2: PortProtocols::default(),
            i2c: PortProtocols::default(),
            spi: PortProtocols::default(),
            base_messages: vec![("NAV_PVT", 1)],
            rtcm_output_uart2: vec![],
            allowed_features: vec![Feature::Satellites],
        }
    }

    /// Rover with NTRIP corrections.
    /// Disables unused ports/protocols to reduce CPU load.
    pub fn rover_ntrip() -> ModePreset {
        ModePreset {
            port_enables: PortEnables {
                uart1: Some(false),
                uart2: Some(false),
                i2c: None,
                spi: Some(false),
            },
            usb: PortProtocols {
                ubx_in: true,
                ubx_out: true,
                rtcm3x_in: true, // NTRIP corrections come in via USB
                ..Default::default()
            },
            uart1: PortProtocols::default(),
            uart2: PortProtocols::default(),
            i2c: PortProtocols::default(),
            spi: PortProtocols::default(),
            base_messages: vec![("NAV_PVT", 1), ("NAV_HPPOSLLH", 1)],
            rtcm_output_uart2: vec![],
            allowed_features: vec![
                Feature::HighPrecision,
                Feature::Integrity,
                Feature::Satellites,
            ],
        }
    }

    /// Rover with radio/serial corrections on UART2.
    pub fn rover_radio() -> ModePreset {
        ModePreset {
            port_enables: PortEnables {
                uart1: Some(false),
                uart2: Some(true), // UART2 needed for radio corrections
                i2c: None,
                spi: Some(false),
            },
            usb: PortProtocols {
                ubx_in: true,
                ubx_out: true,
                ..Default::default()
            },
            uart1: PortProtocols::default(),
            uart2: PortProtocols {
                rtcm3x_in: true, // Corrections from radio
                ..Default::default()
            },
            i2c: PortProtocols::default(),
            spi: PortProtocols::default(),
            base_messages: vec![("NAV_PVT", 1), ("NAV_HPPOSLLH", 1)],
            rtcm_output_uart2: vec![],
            allowed_features: vec![
                Feature::HighPrecision,
                Feature::Integrity,
                Feature::Satellites,
            ],
        }
    }

    /// Moving base in MB+R pair.
    pub fn moving_base() -> ModePreset {
        ModePreset {
            port_enables: PortEnables {
                uart1: Some(false),
                uart2: Some(true), // UART2 outputs RTCM to rover
                i2c: None,
                spi: Some(false),
            },
            usb: PortProtocols {
                ubx_in: true,
                ubx_out: true,
                rtcm3x_in: true, // Optional NTRIP for absolute position
                ..Default::default()
            },
            uart1: PortProtocols::default(),
            uart2: PortProtocols {
                rtcm3x_out: true, // RTCM to rover
                ..Default::default()
            },
            i2c: PortProtocols::default(),
            spi: PortProtocols::default(),
            base_messages: vec![
                ("NAV_PVT", 1),
                ("NAV_HPPOSLLH", 1),
                ("NAV_COV", 1),
                ("NAV_STATUS", 1),
            ],
            rtcm_output_uart2: vec![
                "RTCM_3X_TYPE4072_0", // u-blox proprietary (moving base)
                "RTCM_3X_TYPE1074",   // GPS MSM4
                "RTCM_3X_TYPE1084",   // GLONASS MSM4
                "RTCM_3X_TYPE1094",   // Galileo MSM4
                "RTCM_3X_TYPE1124",   // BeiDou MSM4
                "RTCM_3X_TYPE1230",   // GLONASS code-phase biases
            ],
            allowed_features: vec![
                Feature::HighPrecision,
                Feature::Integrity,
                Feature::Satellites,
            ],
        }
    }

    /// Rover in MB+R pair.
    pub fn moving_base_rover() -> ModePreset {
        ModePreset {
            port_enables: PortEnables {
                uart1: Some(false),
                uart2: Some(true), // UART2 receives RTCM from base
                i2c: None,
                spi: Some(false),
            },
            usb: PortProtocols {
                ubx_in: true,
                ubx_out: true,
                ..Default::default()
            },
            uart1: PortProtocols::default(),
            uart2: PortProtocols {
                rtcm3x_in: true, // RTCM from moving base
                ..Default::default()
            },
            i2c: PortProtocols::default(),
            spi: PortProtocols::default(),
            base_messages: vec![
                ("NAV_PVT", 1),
                ("NAV_HPPOSLLH", 1),
                ("NAV_COV", 1),
                ("NAV_STATUS", 1),
                ("NAV_RELPOSNED", 1), // Relative position to base
            ],
            rtcm_output_uart2: vec![],
            allowed_features: vec![
                Feature::HighPrecision,
                Feature::Integrity,
                Feature::Satellites,
                Feature::Heading,
            ],
        }
    }

    /// Static base station.
    pub fn static_base() -> ModePreset {
        ModePreset {
            port_enables: PortEnables {
                uart1: Some(false),
                uart2: Some(true), // UART2 outputs RTCM
                i2c: None,
                spi: Some(false),
            },
            usb: PortProtocols {
                ubx_in: true,
                ubx_out: true,
                rtcm3x_out: true, // RTCM output
                ..Default::default()
            },
            uart1: PortProtocols::default(),
            uart2: PortProtocols {
                rtcm3x_out: true, // Alternative RTCM output
                ..Default::default()
            },
            i2c: PortProtocols::default(),
            spi: PortProtocols::default(),
            base_messages: vec![("NAV_PVT", 1), ("NAV_SVIN", 1)], // Survey-in status
            rtcm_output_uart2: vec![
                "RTCM_3X_TYPE1005", // Stationary RTK reference station ARP
                "RTCM_3X_TYPE1074", // GPS MSM4
                "RTCM_3X_TYPE1084", // GLONASS MSM4
                "RTCM_3X_TYPE1094", // Galileo MSM4
                "RTCM_3X_TYPE1124", // BeiDou MSM4
                "RTCM_3X_TYPE1230", // GLONASS code-phase biases
            ],
            allowed_features: vec![Feature::Satellites],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_preset_standalone() {
        let preset = OperatingMode::Standalone.preset();
        assert!(preset.usb.ubx_in);
        assert!(preset.usb.ubx_out);
        assert!(!preset.usb.rtcm3x_in);
        assert_eq!(preset.base_messages.len(), 1);
        assert_eq!(preset.base_messages[0].0, "NAV_PVT");
    }

    #[test]
    fn test_mode_preset_rover_ntrip() {
        let preset = OperatingMode::RoverNtrip.preset();
        assert!(preset.usb.rtcm3x_in);
        assert!(preset
            .base_messages
            .iter()
            .any(|(m, _)| *m == "NAV_HPPOSLLH"));
    }

    #[test]
    fn test_mode_preset_moving_base() {
        let preset = OperatingMode::MovingBase.preset();
        assert!(preset.uart2.rtcm3x_out);
        assert!(!preset.rtcm_output_uart2.is_empty());
    }

    #[test]
    fn test_mode_allows_feature() {
        assert!(OperatingMode::RoverNtrip.allows_feature(Feature::HighPrecision));
        assert!(OperatingMode::RoverNtrip.allows_feature(Feature::Integrity));
        assert!(!OperatingMode::Standalone.allows_feature(Feature::Integrity));
        assert!(OperatingMode::MovingBaseRover.allows_feature(Feature::Heading));
        assert!(!OperatingMode::RoverNtrip.allows_feature(Feature::Heading));
    }

    #[test]
    fn test_feature_messages() {
        assert!(Feature::HighPrecision
            .required_messages()
            .contains(&"NAV_HPPOSLLH"));
        assert!(Feature::Integrity.required_messages().contains(&"SEC_SIG"));
    }

    #[test]
    fn test_features_config_enabled() {
        let config = FeaturesConfig {
            high_precision: true,
            integrity: true,
            satellites: false,
            heading: false,
            dead_reckoning: false,
        };
        let enabled = config.enabled_features();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&Feature::HighPrecision));
        assert!(enabled.contains(&Feature::Integrity));
    }

    #[test]
    fn test_deserialize_mode() {
        let yaml = "rover_ntrip";
        let mode: OperatingMode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mode, OperatingMode::RoverNtrip);
    }

    #[test]
    fn test_deserialize_features() {
        let yaml = r#"
high_precision: true
integrity: true
satellites: false
"#;
        let features: FeaturesConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(features.high_precision);
        assert!(features.integrity);
        assert!(!features.satellites);
    }
}
