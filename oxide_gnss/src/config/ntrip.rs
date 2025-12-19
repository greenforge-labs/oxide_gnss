//! NTRIP client configuration.

use serde::Deserialize;

use super::ConfigError;

/// NTRIP client configuration for receiving RTK corrections.
#[derive(Debug, Clone, Deserialize)]
pub struct NtripConfig {
    /// NTRIP caster hostname or IP address
    pub host: String,

    /// NTRIP caster port (typically 2101)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Use HTTPS for connection
    #[serde(default = "default_false")]
    pub use_https: bool,

    /// Mountpoint name
    pub mountpoint: String,

    /// Username for authentication (optional for some casters)
    #[serde(default)]
    pub username: Option<String>,

    /// Password for authentication
    #[serde(default)]
    pub password: Option<String>,

    /// Send GGA position reports to caster
    #[serde(default = "default_true")]
    pub send_gga: bool,

    /// Interval for GGA position reports (seconds)
    #[serde(default = "default_gga_interval")]
    pub gga_interval_secs: u32,

    /// Connection settings
    #[serde(default)]
    pub connection: NtripConnectionConfig,
}

/// NTRIP connection behavior configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct NtripConnectionConfig {
    /// Connection timeout (seconds)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,

    /// Read timeout for data reception (seconds). If no data is received
    /// within this period, the connection is considered lost.
    #[serde(default = "default_read_timeout")]
    pub read_timeout_secs: u32,

    /// Enable automatic reconnection
    #[serde(default = "default_true")]
    pub reconnect: bool,

    /// Initial delay before first reconnection attempt (seconds)
    #[serde(default = "default_initial_delay")]
    pub initial_delay_secs: u32,

    /// Maximum delay between reconnection attempts (seconds)
    #[serde(default = "default_max_delay")]
    pub max_delay_secs: u32,

    /// Time in seconds of continuous successful streaming after which the
    /// reconnect backoff is reset (0 = never reset).
    #[serde(default = "default_backoff_reset_secs")]
    pub backoff_reset_secs: u32,
}

impl Default for NtripConnectionConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout(),
            read_timeout_secs: default_read_timeout(),
            reconnect: true,
            initial_delay_secs: default_initial_delay(),
            max_delay_secs: default_max_delay(),
            backoff_reset_secs: default_backoff_reset_secs(),
        }
    }
}

impl NtripConfig {
    /// Validate the NTRIP configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.host.is_empty() {
            return Err(ConfigError::Validation {
                message: "NTRIP host cannot be empty".to_string(),
            });
        }

        if self.port == 0 {
            return Err(ConfigError::Validation {
                message: "NTRIP port must be greater than 0".to_string(),
            });
        }

        if self.mountpoint.is_empty() {
            return Err(ConfigError::Validation {
                message: "NTRIP mountpoint cannot be empty".to_string(),
            });
        }

        if self.gga_interval_secs == 0 {
            return Err(ConfigError::Validation {
                message: "GGA interval must be greater than 0".to_string(),
            });
        }

        // TLS/HTTPS is not yet implemented - fail fast with clear message
        if self.use_https {
            return Err(ConfigError::Validation {
                message: "NTRIP HTTPS/TLS is not yet implemented. Set use_https: false or wait for TLS support.".to_string(),
            });
        }

        Ok(())
    }

    /// Build the NTRIP URL for connection.
    pub fn url(&self) -> String {
        let scheme = if self.use_https { "https" } else { "http" };
        format!(
            "{}://{}:{}/{}",
            scheme, self.host, self.port, self.mountpoint
        )
    }

    /// Check if authentication is configured.
    pub fn has_auth(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }
}

// Default value functions for serde
fn default_port() -> u16 {
    2101
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_gga_interval() -> u32 {
    10
}

fn default_timeout() -> u32 {
    10
}

fn default_initial_delay() -> u32 {
    1
}

fn default_max_delay() -> u32 {
    60
}

fn default_backoff_reset_secs() -> u32 {
    3600
}

fn default_read_timeout() -> u32 {
    30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let yaml = r#"
host: "example.com"
mountpoint: "MOUNT"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 2101);
        assert_eq!(config.mountpoint, "MOUNT");
        assert!(config.send_gga);
        assert_eq!(config.gga_interval_secs, 10);
    }

    #[test]
    fn test_url_generation() {
        let yaml = r#"
host: "auscors.ga.gov.au"
port: 2101
mountpoint: "ALIC00AUS0"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.url(), "http://auscors.ga.gov.au:2101/ALIC00AUS0");

        let yaml_https = r#"
host: "secure.ntrip.com"
port: 443
mountpoint: "MNT"
use_https: true
"#;
        let config_https: NtripConfig = serde_yaml::from_str(yaml_https).unwrap();
        assert_eq!(config_https.url(), "https://secure.ntrip.com:443/MNT");
    }

    #[test]
    fn test_validation_empty_host() {
        let yaml = r#"
host: ""
mountpoint: "MOUNT"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_has_auth() {
        let yaml = r#"
host: "example.com"
mountpoint: "MOUNT"
username: "user"
password: "pass"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.has_auth());
    }
}
