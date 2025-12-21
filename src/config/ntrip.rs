//! NTRIP client configuration.

use std::fmt;

use serde::Deserialize;

use super::ConfigError;

/// NTRIP protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NtripVersion {
    /// NTRIP v1 - Legacy ICY protocol (HTTP/1.0, ICY 200 OK response)
    V1,
    /// NTRIP v2 - Standard HTTP/1.1 with chunked transfer encoding
    V2,
    /// Auto-detect version from server response (default)
    #[default]
    Auto,
}

impl fmt::Display for NtripVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V1 => write!(f, "1"),
            Self::V2 => write!(f, "2"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

impl<'de> Deserialize<'de> for NtripVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "1" | "v1" | "ntrip1" => Ok(Self::V1),
            "2" | "v2" | "ntrip2" => Ok(Self::V2),
            "auto" | "" => Ok(Self::Auto),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid NTRIP version '{}'. Use '1', '2', or 'auto'",
                s
            ))),
        }
    }
}

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

    /// Skip TLS certificate verification (for self-signed certs, testing only)
    #[serde(default = "default_false")]
    pub tls_skip_verify: bool,

    /// NTRIP protocol version: "1", "2", or "auto" (default: auto)
    #[serde(default)]
    pub ntrip_version: NtripVersion,

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
    /// Convert to ntrip_core::NtripConfig for use with the NTRIP client.
    ///
    /// Note: Reconnection is disabled in the returned config - oxide_gnss
    /// manages reconnection externally via NtripTask with exponential backoff.
    pub fn to_ntrip_core_config(&self) -> ntrip_core::NtripConfig {
        let mut config = ntrip_core::NtripConfig::new(&self.host, self.port, &self.mountpoint)
            .with_timeout(self.connection.timeout_secs)
            .with_read_timeout(self.connection.read_timeout_secs)
            .without_reconnect(); // Oxide manages reconnection externally

        // Set credentials if provided
        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            config = config.with_credentials(user, pass);
        }

        // Set TLS options
        if self.use_https {
            config = config.with_tls();
        }
        if self.tls_skip_verify {
            config = config.with_tls_skip_verify();
        }

        // Set protocol version
        config = config.with_version(match self.ntrip_version {
            NtripVersion::V1 => ntrip_core::NtripVersion::V1,
            NtripVersion::V2 => ntrip_core::NtripVersion::V2,
            NtripVersion::Auto => ntrip_core::NtripVersion::Auto,
        });

        config
    }

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

    #[test]
    fn test_to_ntrip_core_config_basic() {
        let yaml = r#"
host: "example.com"
port: 2101
mountpoint: "TEST"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        let core_config = config.to_ntrip_core_config();

        assert_eq!(core_config.host, "example.com");
        assert_eq!(core_config.port, 2101);
        assert_eq!(core_config.mountpoint, "TEST");
        assert!(!core_config.use_tls);
        assert_eq!(core_config.connection.max_reconnect_attempts, 0); // Reconnect disabled
    }

    #[test]
    fn test_to_ntrip_core_config_with_credentials() {
        let yaml = r#"
host: "example.com"
mountpoint: "TEST"
username: "myuser"
password: "mypass"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        let core_config = config.to_ntrip_core_config();

        assert_eq!(core_config.username, Some("myuser".to_string()));
        assert_eq!(core_config.password, Some("mypass".to_string()));
    }

    #[test]
    fn test_to_ntrip_core_config_with_tls() {
        let yaml = r#"
host: "secure.example.com"
port: 443
mountpoint: "TEST"
use_https: true
tls_skip_verify: true
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        let core_config = config.to_ntrip_core_config();

        assert!(core_config.use_tls);
        assert!(core_config.tls_skip_verify);
    }

    #[test]
    fn test_to_ntrip_core_config_version_mapping() {
        // Test V1
        let yaml = r#"
host: "example.com"
mountpoint: "TEST"
ntrip_version: "v1"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        let core_config = config.to_ntrip_core_config();
        assert_eq!(core_config.ntrip_version, ntrip_core::NtripVersion::V1);

        // Test V2
        let yaml = r#"
host: "example.com"
mountpoint: "TEST"
ntrip_version: "v2"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        let core_config = config.to_ntrip_core_config();
        assert_eq!(core_config.ntrip_version, ntrip_core::NtripVersion::V2);

        // Test Auto (default)
        let yaml = r#"
host: "example.com"
mountpoint: "TEST"
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        let core_config = config.to_ntrip_core_config();
        assert_eq!(core_config.ntrip_version, ntrip_core::NtripVersion::Auto);
    }

    #[test]
    fn test_to_ntrip_core_config_timeouts() {
        let yaml = r#"
host: "example.com"
mountpoint: "TEST"
connection:
  timeout_secs: 20
  read_timeout_secs: 45
"#;
        let config: NtripConfig = serde_yaml::from_str(yaml).unwrap();
        let core_config = config.to_ntrip_core_config();

        assert_eq!(core_config.connection.timeout_secs, 20);
        assert_eq!(core_config.connection.read_timeout_secs, 45);
    }
}
