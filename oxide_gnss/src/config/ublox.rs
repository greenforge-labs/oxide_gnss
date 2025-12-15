//! u-blox device-specific configuration.
//!
//! Configuration key names use the u-blox `CFG_*` naming convention
//! (e.g., `CFG_MSGOUT_UBX_NAV_PVT_USB`) for familiarity with u-blox documentation.

use std::collections::HashMap;

use serde::Deserialize;

/// u-blox device-specific configuration.
///
/// Uses the u-blox `CFG_*` naming convention for keys.
/// Values can be integers (for message rates) or booleans (for protocol enables).
///
/// # Example
/// ```yaml
/// ublox:
///   family: "F9P"
///   CFG_RATE_MEAS: 100
///   CFG_MSGOUT_UBX_NAV_PVT_USB: 1
///   CFG_USBINPROT_UBX: true
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UbloxConfig {
    /// Device family (F9P, F9R, X20P) - informational only
    #[serde(default)]
    pub family: Option<String>,

    /// CFG_* keys with their values.
    /// Keys are strings matching u-blox documentation names.
    /// Values can be integers or booleans.
    #[serde(flatten)]
    pub cfg_keys: HashMap<String, serde_yaml::Value>,
}

impl UbloxConfig {
    /// Check if any configuration keys are set.
    pub fn has_config(&self) -> bool {
        !self.cfg_keys.is_empty()
    }

    /// Get the number of configuration keys.
    pub fn len(&self) -> usize {
        self.cfg_keys.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.cfg_keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ublox_config() {
        let yaml = r#"
family: "F9P"
CFG_RATE_MEAS: 100
CFG_MSGOUT_UBX_NAV_PVT_USB: 1
CFG_USBINPROT_UBX: true
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.family, Some("F9P".to_string()));
        assert_eq!(config.cfg_keys.len(), 3);
        assert_eq!(
            config.cfg_keys.get("CFG_RATE_MEAS").unwrap().as_u64(),
            Some(100)
        );
        assert_eq!(
            config.cfg_keys.get("CFG_USBINPROT_UBX").unwrap().as_bool(),
            Some(true)
        );
    }

    #[test]
    fn test_empty_config() {
        let yaml = r#"{}"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.is_empty());
        assert!(!config.has_config());
    }
}
