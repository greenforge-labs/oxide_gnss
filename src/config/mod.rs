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

        // Use rate config from device if specified, otherwise defaults.
        // If the user did not explicitly set `ublox.rate.measurement_ms`,
        // derive it from `device.navigation.rate_hz` so that the
        // user-facing Hz knob is actually wired through to CFG-RATE-MEAS.
        let mut rate = self
            .device
            .ublox
            .as_ref()
            .map(|u| u.rate.clone())
            .unwrap_or_default();
        if rate.measurement_ms.is_none() {
            let hz = self.device.navigation.rate_hz.max(1) as u16;
            rate.measurement_ms = Some(1000 / hz);
        }

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
        // For non-base modes, force TMODE3=Disabled unless the user has
        // explicitly set `device.ublox.base_position`. This prevents a
        // previous static_base session from leaving the receiver in
        // SurveyIn/Fixed mode when relaunched into a rover/moving-base mode
        // (observed during Test 5 on the bench rig — TMODE3 ghost caused
        // fixType=5 "Time Only" and no RTCM output on a moving_base F9P).
        let user_base_position = self
            .device
            .ublox
            .as_ref()
            .and_then(|u| u.base_position.clone());
        let base_position = if mode == OperatingMode::StaticBase {
            user_base_position
        } else {
            user_base_position.or_else(|| Some(BasePositionConfig::default()))
        };
        let time_mark = self.device.ublox.as_ref().and_then(|u| u.time_mark.clone());

        // Honor a user-supplied `ublox.clear_unmanaged` if present; otherwise
        // default to true (self-cleaning).
        let clear_unmanaged = self
            .device
            .ublox
            .as_ref()
            .map(|u| u.clear_unmanaged)
            .unwrap_or(true);

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
            clear_unmanaged,
        }
    }

    /// Get the list of ROS topics that will be published based on configuration.
    pub fn enabled_topics(&self) -> Vec<&'static str> {
        let mut topics = vec!["~/fix", "~/velocity", "~/time_reference"];

        if self.mode.is_some() {
            // Add topics based on features. `~/baseline_pose` is gated on
            // the `heading` feature (only allowed in MovingBaseRover mode),
            // so the flag is now authoritative: `heading: false` in that
            // mode disables both the ROS topic and the NAV_RELPOSNED UBX
            // message that feeds it.
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
                    Feature::Heading => {
                        topics.push("~/baseline_pose");
                    }
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
    /// Failed to read the configuration file from disk.
    #[error("Failed to read config file '{path}': {source}")]
    Io {
        /// Path to the config file.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Failed to parse YAML configuration.
    #[error("Failed to parse config: {source}")]
    Parse {
        /// Underlying YAML parse error.
        #[from]
        source: serde_yaml::Error,
    },

    /// Configuration validation failed.
    #[error("Invalid configuration: {message}")]
    Validation {
        /// Description of the validation failure.
        message: String,
    },
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

    // Issue #2: rate_hz must derive measurement_ms when the user didn't set it.
    #[test]
    fn test_rate_hz_derives_measurement_ms() {
        let yaml = r#"
mode: rover_ntrip
device:
  port: "/dev/ttyACM0"
  navigation:
    rate_hz: 5
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let ublox = config.resolve_ublox_config();
        assert_eq!(ublox.rate.measurement_ms, Some(200));
        assert_eq!(ublox.rate.effective_measurement_ms(), 200);
    }

    #[test]
    fn test_explicit_measurement_ms_overrides_rate_hz() {
        let yaml = r#"
mode: rover_ntrip
device:
  port: "/dev/ttyACM0"
  navigation:
    rate_hz: 5
  ublox:
    rate:
      measurement_ms: 50
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let ublox = config.resolve_ublox_config();
        assert_eq!(ublox.rate.measurement_ms, Some(50));
    }

    // Issue #5: non-base modes must force TMODE3=Disabled.
    #[test]
    fn test_non_base_mode_forces_tmode3_disabled() {
        for mode in [
            "standalone",
            "rover_ntrip",
            "moving_base",
            "moving_base_rover",
        ] {
            let yaml = format!(
                r#"
mode: {}
device:
  port: "/dev/ttyACM0"
"#,
                mode
            );
            let config: Config = serde_yaml::from_str(&yaml).unwrap();
            let ublox = config.resolve_ublox_config();
            let bp = ublox
                .base_position
                .expect("non-base modes must inject a BasePositionConfig");
            assert_eq!(
                bp.mode,
                BasePositionMode::Disabled,
                "mode {} should force TMODE3=Disabled",
                mode
            );
        }
    }

    #[test]
    fn test_static_base_passes_through_user_base_position() {
        let yaml = r#"
mode: static_base
device:
  port: "/dev/ttyACM0"
  ublox:
    base_position:
      mode: survey_in
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let ublox = config.resolve_ublox_config();
        let bp = ublox.base_position.expect("user base_position");
        assert_eq!(bp.mode, BasePositionMode::SurveyIn);
    }

    // Issue #6: `~/baseline_pose` is gated on Feature::Heading.
    #[test]
    fn test_baseline_pose_requires_heading_feature() {
        let yaml_without = r#"
mode: moving_base_rover
device:
  port: "/dev/ttyACM0"
"#;
        let config: Config = serde_yaml::from_str(yaml_without).unwrap();
        let topics = config.enabled_topics();
        assert!(
            !topics.contains(&"~/baseline_pose"),
            "MovingBaseRover without heading:true must not publish ~/baseline_pose; got {:?}",
            topics
        );

        let yaml_with = r#"
mode: moving_base_rover
features:
  heading: true
device:
  port: "/dev/ttyACM0"
"#;
        let config: Config = serde_yaml::from_str(yaml_with).unwrap();
        let topics = config.enabled_topics();
        assert!(
            topics.contains(&"~/baseline_pose"),
            "MovingBaseRover with heading:true must publish ~/baseline_pose; got {:?}",
            topics
        );
    }

    #[test]
    fn test_nav_relposned_requires_heading_feature() {
        let yaml_without = r#"
mode: moving_base_rover
device:
  port: "/dev/ttyACM0"
"#;
        let config: Config = serde_yaml::from_str(yaml_without).unwrap();
        let ublox = config.resolve_ublox_config();
        assert!(
            !ublox.messages.usb.contains_key("NAV_RELPOSNED"),
            "NAV_RELPOSNED must not be enabled without heading feature"
        );

        let yaml_with = r#"
mode: moving_base_rover
features:
  heading: true
device:
  port: "/dev/ttyACM0"
"#;
        let config: Config = serde_yaml::from_str(yaml_with).unwrap();
        let ublox = config.resolve_ublox_config();
        assert!(
            ublox.messages.usb.contains_key("NAV_RELPOSNED"),
            "NAV_RELPOSNED must be enabled when heading feature is on"
        );
    }
}
