//! Configuration module for oxide_gnss.
//!
//! Handles loading and validation of device and NTRIP configuration from YAML files.
//!
//! ## Configuration Modes
//!
//! The driver supports two configuration styles:
//!
//! 1. **Mode-based** (recommended): Specify an operating mode and optional features.
//!    The driver automatically configures the required UBX messages.
//!
//! 2. **Legacy**: Explicitly specify UBX messages in the `ublox.messages` section.
//!    This is still supported for advanced users.

mod device;
mod integrity;
mod modes;
mod ntrip;
mod ublox;

pub use device::{CoordinateFrame, DeviceConfig, NavigationConfig, ReconnectConfig};
pub use integrity::{IntegrityConfig, IntegrityThresholdsConfig};
pub use modes::{Feature, FeaturesConfig, ModePreset, OperatingMode};
pub use ntrip::{NtripConfig, NtripConnectionConfig, NtripVersion};
pub use ublox::{
    BasePositionConfig, BasePositionMode, BeidouConfig, DynamicModel, FixedPositionConfig,
    GnssConstellationConfig, MessageConfig, NavSpgConfig, PortProtocols, PortSettings,
    ProtocolConfig, QzssConfig, RateConfig, SbasConfig, SignalConfig, SurveyInConfig, TimeGrid,
    TimeMarkConfig, TimepulseConfig, TimepulsePolarity, UartPortConfig, UbloxConfig,
};

use regex::Regex;
use serde::Deserialize;
use std::path::Path;
use std::sync::LazyLock;

/// Regex for matching `${VAR}` environment variable placeholders.
static ENV_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}").expect("static regex"));

/// Root configuration structure containing all settings.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Operating mode (optional - if not set, uses legacy ublox config)
    #[serde(default)]
    pub mode: Option<OperatingMode>,

    /// Feature flags (only used with mode-based config)
    #[serde(default)]
    pub features: FeaturesConfig,

    /// Device configuration
    pub device: DeviceConfig,

    /// ROS publishing configuration (optional)
    #[serde(default)]
    pub ros: RosConfig,

    /// NTRIP client configuration (optional)
    #[serde(default)]
    pub ntrip: Option<NtripConfig>,

    /// Integrity monitoring configuration (optional)
    #[serde(default)]
    pub integrity: IntegrityConfig,
}

/// ROS-related configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RosConfig {
    /// Topic publish rates
    #[serde(default)]
    pub rates: RosRatesConfig,
}

/// ROS topic publish rate configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct RosRatesConfig {
    /// Diagnostics publish rate in Hz (default: 1.0)
    #[serde(default = "default_diagnostics_rate")]
    pub diagnostics_hz: f64,

    /// Integrity/operational publish rate in Hz (default: 1.0)
    #[serde(default = "default_integrity_rate")]
    pub integrity_hz: f64,
}

fn default_diagnostics_rate() -> f64 {
    1.0
}

fn default_integrity_rate() -> f64 {
    1.0
}

impl Default for RosRatesConfig {
    fn default() -> Self {
        Self {
            diagnostics_hz: 1.0,
            integrity_hz: 1.0,
        }
    }
}

impl Config {
    /// Load configuration from a YAML file.
    ///
    /// Supports environment variable substitution in the format `${VAR_NAME}`.
    ///
    /// # Arguments
    /// * `path` - Path to the YAML configuration file
    ///
    /// # Returns
    /// * `Ok(Config)` - Successfully loaded and validated configuration
    /// * `Err` - If file cannot be read or parsed
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::Io {
            path: path.as_ref().to_path_buf(),
            source: e,
        })?;

        let contents = Self::substitute_env_vars(&contents);

        let config: Config =
            serde_yaml::from_str(&contents).map_err(|e| ConfigError::Parse { source: e })?;

        config.validate()?;
        Ok(config)
    }

    /// Substitute environment variables in the format `${VAR}`.
    fn substitute_env_vars(input: &str) -> String {
        ENV_VAR_RE
            .replace_all(input, |caps: &regex::Captures| {
                let var_name = &caps[1];
                std::env::var(var_name).unwrap_or_else(|_| {
                    // Keep original if var not found
                    format!("${{{}}}", var_name)
                })
            })
            .to_string()
    }

    /// Check if this config uses mode-based configuration.
    pub fn is_mode_based(&self) -> bool {
        self.mode.is_some()
    }

    /// Check if this config uses legacy UBX message configuration.
    pub fn is_legacy(&self) -> bool {
        self.mode.is_none() && self.device.ublox.as_ref().is_some_and(|u| u.has_config())
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.device.validate()?;

        if let Some(ref ntrip) = self.ntrip {
            ntrip.validate()?;
        }

        // Validate mode-based configuration
        if let Some(mode) = self.mode {
            self.validate_mode_config(mode)?;
        }

        Ok(())
    }

    /// Validate mode-based configuration.
    fn validate_mode_config(&self, mode: OperatingMode) -> Result<(), ConfigError> {
        // Check NTRIP requirement
        if mode.requires_ntrip() && self.ntrip.is_none() {
            return Err(ConfigError::Validation {
                message: format!(
                    "Mode '{}' requires NTRIP configuration, but 'ntrip' section is missing",
                    mode
                ),
            });
        }

        // Validate feature compatibility
        for feature in self.features.enabled_features() {
            if !mode.allows_feature(feature) {
                return Err(ConfigError::Validation {
                    message: format!(
                        "Feature '{}' is not compatible with mode '{}'. Allowed features: {:?}",
                        feature,
                        mode,
                        mode.preset()
                            .allowed_features
                            .iter()
                            .map(|f| f.name())
                            .collect::<Vec<_>>()
                    ),
                });
            }
        }

        Ok(())
    }

    /// Resolve the effective UBX configuration.
    ///
    /// For mode-based config, this generates the UbloxConfig from mode + features.
    /// For legacy config, this returns the explicit ublox config.
    pub fn resolve_ublox_config(&self) -> UbloxConfig {
        if let Some(mode) = self.mode {
            self.generate_ublox_config_from_mode(mode)
        } else {
            // Legacy mode - use explicit config or defaults
            self.device.ublox.clone().unwrap_or_default()
        }
    }

    /// Generate UbloxConfig from mode and features.
    fn generate_ublox_config_from_mode(&self, mode: OperatingMode) -> UbloxConfig {
        use std::collections::HashMap;

        let preset = mode.preset();

        // Build protocol config from mode preset (explicit enable/disable)
        let protocols = ProtocolConfig {
            usb: (&preset.usb).into(),
            uart1: (&preset.uart1).into(),
            uart2: (&preset.uart2).into(),
            i2c: (&preset.i2c).into(),
            spi: (&preset.spi).into(),
        };

        // Build port settings from mode preset (enable/disable ports)
        let ports = PortSettings {
            uart1: UartPortConfig {
                enabled: preset.port_enables.uart1,
                baudrate: self
                    .device
                    .ublox
                    .as_ref()
                    .and_then(|u| u.ports.uart1.baudrate),
            },
            uart2: UartPortConfig {
                enabled: preset.port_enables.uart2,
                baudrate: self
                    .device
                    .ublox
                    .as_ref()
                    .and_then(|u| u.ports.uart2.baudrate),
            },
            i2c_enabled: preset.port_enables.i2c,
            spi_enabled: preset.port_enables.spi,
        };

        // Build message config: start with mode base messages
        let mut usb_messages: HashMap<String, u8> = preset.messages_as_map();

        // Add feature-required messages
        for feature in self.features.enabled_features() {
            for msg in feature.required_messages() {
                usb_messages.entry(msg.to_string()).or_insert(1);
            }
        }

        // Add any overrides from explicit ublox config
        if let Some(ref ublox_override) = self.device.ublox {
            for (msg, rate) in &ublox_override.messages.usb {
                usb_messages.insert(msg.clone(), *rate);
            }
        }

        // Build UART2 RTCM message config for base modes
        let mut uart2_messages: HashMap<String, u8> = HashMap::new();
        for rtcm_msg in &preset.rtcm_output_uart2 {
            uart2_messages.insert(rtcm_msg.to_string(), 1);
        }

        let messages = MessageConfig {
            usb: usb_messages,
            uart1: HashMap::new(),
            uart2: uart2_messages,
        };

        // Use rate config from device if specified, otherwise defaults
        let rate = self
            .device
            .ublox
            .as_ref()
            .map(|u| u.rate.clone())
            .unwrap_or_default();

        // Use signal config from device if specified
        let signals = self
            .device
            .ublox
            .as_ref()
            .map(|u| u.signals.clone())
            .unwrap_or_default();

        // Enable protection level calculation when integrity feature is active
        let nav_spg = if self.features.integrity {
            NavSpgConfig {
                pl_ena: Some(true),
                dynamic_model: None,
                elevation_mask: None,
                pdop_mask: None,
            }
        } else {
            self.device
                .ublox
                .as_ref()
                .map(|u| u.nav_spg.clone())
                .unwrap_or_default()
        };

        // Pass through optional configs from user
        let timepulse = self.device.ublox.as_ref().and_then(|u| u.timepulse.clone());
        let base_position = self
            .device
            .ublox
            .as_ref()
            .and_then(|u| u.base_position.clone());
        let time_mark = self.device.ublox.as_ref().and_then(|u| u.time_mark.clone());

        UbloxConfig {
            family: self.device.ublox.as_ref().and_then(|u| u.family.clone()),
            rate,
            protocols,
            messages,
            ports,
            signals,
            nav_spg,
            timepulse,
            base_position,
            time_mark,
        }
    }

    /// Get the list of ROS topics that will be published based on configuration.
    pub fn enabled_topics(&self) -> Vec<&'static str> {
        let mut topics = vec!["~/fix", "~/velocity", "~/time_reference"];

        if let Some(mode) = self.mode {
            // Add topics based on mode
            if mode == OperatingMode::MovingBaseRover {
                topics.push("~/baseline_pose");
            }

            // Add topics based on features
            for feature in self.features.enabled_features() {
                match feature {
                    Feature::HighPrecision => {
                        // HP data enhances ~/fix, no separate topic needed
                    }
                    Feature::Integrity => {
                        topics.push("~/integrity");
                        topics.push("~/operational");
                    }
                    Feature::Satellites => {
                        topics.push("~/satellites");
                    }
                    _ => {}
                }
            }
        } else {
            // Legacy mode - check explicit messages
            if let Some(ref ublox) = self.device.ublox {
                // NAV_HPPOSLLH just enhances ~/fix, no separate topic
                if ublox.is_message_enabled("SEC_SIG") {
                    topics.push("~/integrity");
                    topics.push("~/operational");
                }
                if ublox.is_message_enabled("NAV_SAT") {
                    topics.push("~/satellites");
                }
                if ublox.is_message_enabled("NAV_RELPOSNED") {
                    topics.push("~/baseline_pose");
                }
            }
        }

        topics
    }

    /// Log the effective configuration at startup.
    pub fn log_effective_config(&self) {
        use tracing::info;

        if let Some(mode) = self.mode {
            info!("Configuration mode: {}", mode);
            info!("Mode description: {}", mode.description());

            let features: Vec<_> = self
                .features
                .enabled_features()
                .iter()
                .map(|f| f.name())
                .collect();
            if features.is_empty() {
                info!("Features: (none)");
            } else {
                info!("Features: {:?}", features);
            }
        } else if self.is_legacy() {
            info!("Configuration mode: legacy (explicit UBX messages)");
        } else {
            info!("Configuration mode: minimal (defaults)");
        }

        let topics = self.enabled_topics();
        info!("Enabled topics: {:?}", topics);

        let resolved = self.resolve_ublox_config();
        let messages: Vec<_> = resolved.messages.usb.keys().collect();
        info!("UBX messages (USB): {:?}", messages);

        if self.ntrip.is_some() {
            info!("NTRIP: enabled");
        } else {
            info!("NTRIP: disabled");
        }
    }
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse config: {source}")]
    Parse {
        #[from]
        source: serde_yaml::Error,
    },

    #[error("Invalid configuration: {message}")]
    Validation { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
device:
  port: "/dev/ttyACM0"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.device.port, "/dev/ttyACM0");
        assert!(config.ntrip.is_none());
    }

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
device:
  port: "/dev/ttyACM0"
  baud_rate: 460800
  frame: ENU

ntrip:
  host: "auscors.ga.gov.au"
  port: 2101
  mountpoint: "ALIC00AUS0"
  username: "user"
  password: "pass"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.device.baud_rate, 460800);
        assert!(config.ntrip.is_some());
    }

    #[test]
    fn test_env_substitution() {
        std::env::set_var("TEST_PORT", "/dev/ttyUSB0");
        std::env::set_var("TEST_USER", "env_user");

        let yaml = r#"
device:
  port: "${TEST_PORT}"
ntrip:
  host: "test.com"
  port: 2101
  mountpoint: "MNT"
  username: "${TEST_USER}"
"#;

        let substituted = Config::substitute_env_vars(yaml);
        let config: Config = serde_yaml::from_str(&substituted).unwrap();

        assert_eq!(config.device.port, "/dev/ttyUSB0");
        assert_eq!(config.ntrip.unwrap().username.unwrap(), "env_user");
    }

    #[test]
    fn test_env_substitution_missing() {
        let yaml = "port: ${MISSING_VAR}";
        let substituted = Config::substitute_env_vars(yaml);
        assert_eq!(substituted, "port: ${MISSING_VAR}");
    }
}
