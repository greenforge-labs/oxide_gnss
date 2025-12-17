//! Configuration module for oxide_gnss.
//!
//! Handles loading and validation of device and NTRIP configuration from YAML files.

mod device;
mod ntrip;
mod ublox;

pub use device::{CoordinateFrame, DeviceConfig, NavigationConfig, ReconnectConfig};
pub use ntrip::{NtripConfig, NtripConnectionConfig};
pub use ublox::{MessageConfig, ProtocolConfig, RateConfig, UbloxConfig};

use serde::Deserialize;
use std::path::Path;

/// Root configuration structure containing all settings.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Device configuration
    pub device: DeviceConfig,

    /// ROS publishing configuration (optional)
    #[serde(default)]
    pub ros: RosConfig,

    /// NTRIP client configuration (optional)
    #[serde(default)]
    pub ntrip: Option<NtripConfig>,
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
        use regex::Regex;

        let re = Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();
        re.replace_all(input, |caps: &regex::Captures| {
            let var_name = &caps[1];
            std::env::var(var_name).unwrap_or_else(|_| {
                // Keep original if var not found
                format!("${{{}}}", var_name)
            })
        })
        .to_string()
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.device.validate()?;

        if let Some(ref ntrip) = self.ntrip {
            ntrip.validate()?;
        }

        Ok(())
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
