//! Integrity monitoring configuration.
//!
//! Configurable thresholds for GNSS integrity checks.

use serde::Deserialize;

use crate::state::IntegrityThresholds;

/// Integrity monitoring configuration.
///
/// All fields are optional - if not specified, defaults are used.
///
/// # Example
/// ```yaml
/// integrity:
///   thresholds:
///     min_satellites_critical: 4
///     min_satellites_high: 6
///     max_h_accuracy_m: 0.10
///     max_v_accuracy_m: 0.15
///     max_pdop: 3.0
///     max_correction_age_s: 10.0
///     min_cno_degraded: 25
///     min_mean_cno_degraded: 35.0
///     max_pvt_age_s: 2.0
///     operational_threshold: 1
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct IntegrityConfig {
    /// Threshold configuration for integrity checks
    #[serde(default)]
    pub thresholds: IntegrityThresholdsConfig,
}

/// Configurable thresholds for integrity checks.
///
/// All fields are optional with sensible defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct IntegrityThresholdsConfig {
    /// Minimum satellites for CRITICAL level (default: 4)
    #[serde(default = "default_min_satellites_critical")]
    pub min_satellites_critical: u8,

    /// Minimum satellites for high quality / DEGRADED if below (default: 6)
    #[serde(default = "default_min_satellites_high")]
    pub min_satellites_high: u8,

    /// Maximum horizontal accuracy (m) before DEGRADED (default: 0.10)
    #[serde(default = "default_max_h_accuracy_m")]
    pub max_h_accuracy_m: f32,

    /// Maximum vertical accuracy (m) before DEGRADED (default: 0.15)
    #[serde(default = "default_max_v_accuracy_m")]
    pub max_v_accuracy_m: f32,

    /// Maximum PDOP before DEGRADED (default: 3.0)
    #[serde(default = "default_max_pdop")]
    pub max_pdop: f32,

    /// Maximum correction age (s) before DEGRADED for RTK (default: 10.0)
    #[serde(default = "default_max_correction_age_s")]
    pub max_correction_age_s: f32,

    /// Minimum C/N0 (dB-Hz) for weakest satellite before DEGRADED (default: 25)
    #[serde(default = "default_min_cno_degraded")]
    pub min_cno_degraded: u8,

    /// Minimum mean C/N0 (dB-Hz) before DEGRADED (default: 35.0)
    #[serde(default = "default_min_mean_cno_degraded")]
    pub min_mean_cno_degraded: f32,

    /// Maximum time (s) without PVT before FAILED (staleness) (default: 2.0)
    #[serde(default = "default_max_pvt_age_s")]
    pub max_pvt_age_s: f32,

    /// Integrity level at which operational becomes false (default: 1)
    /// 0 = only OK is operational (strictest)
    /// 1 = OK or DEGRADED is operational (default)
    /// 2 = OK/DEGRADED/CRITICAL is operational (permissive)
    #[serde(default = "default_operational_threshold")]
    pub operational_threshold: u8,

    // Protection Level (NAV-PL) thresholds
    /// Maximum horizontal protection level (m) before DEGRADED (default: 0.50)
    #[serde(default = "default_max_horizontal_pl_m")]
    pub max_horizontal_pl_m: f32,

    /// Maximum vertical protection level (m) before DEGRADED (default: 1.00)
    #[serde(default = "default_max_vertical_pl_m")]
    pub max_vertical_pl_m: f32,

    /// Maximum velocity protection level (m/s) before DEGRADED (default: 0.10)
    #[serde(default = "default_max_velocity_pl_ms")]
    pub max_velocity_pl_ms: f32,

    /// Maximum TMIR (Target Misleading Information Risk) per epoch (default: 1e-5)
    #[serde(default = "default_max_tmir_per_epoch")]
    pub max_tmir_per_epoch: f64,

    /// Require valid protection level for operational status (default: false)
    /// If true, invalid PL will result in CRITICAL level
    #[serde(default = "default_require_valid_pl")]
    pub require_valid_pl: bool,
}

fn default_min_satellites_critical() -> u8 {
    4
}
fn default_min_satellites_high() -> u8 {
    6
}
fn default_max_h_accuracy_m() -> f32 {
    0.1
}
fn default_max_v_accuracy_m() -> f32 {
    0.15
}
fn default_max_pdop() -> f32 {
    3.0
}
fn default_max_correction_age_s() -> f32 {
    10.0
}
fn default_min_cno_degraded() -> u8 {
    25
}
fn default_min_mean_cno_degraded() -> f32 {
    35.0
}
fn default_max_pvt_age_s() -> f32 {
    2.0
}
fn default_operational_threshold() -> u8 {
    1
}
fn default_max_horizontal_pl_m() -> f32 {
    0.50
}
fn default_max_vertical_pl_m() -> f32 {
    1.00
}
fn default_max_velocity_pl_ms() -> f32 {
    0.10
}
fn default_max_tmir_per_epoch() -> f64 {
    1e-5
}
fn default_require_valid_pl() -> bool {
    false
}

impl Default for IntegrityThresholdsConfig {
    fn default() -> Self {
        Self {
            min_satellites_critical: default_min_satellites_critical(),
            min_satellites_high: default_min_satellites_high(),
            max_h_accuracy_m: default_max_h_accuracy_m(),
            max_v_accuracy_m: default_max_v_accuracy_m(),
            max_pdop: default_max_pdop(),
            max_correction_age_s: default_max_correction_age_s(),
            min_cno_degraded: default_min_cno_degraded(),
            min_mean_cno_degraded: default_min_mean_cno_degraded(),
            max_pvt_age_s: default_max_pvt_age_s(),
            operational_threshold: default_operational_threshold(),
            max_horizontal_pl_m: default_max_horizontal_pl_m(),
            max_vertical_pl_m: default_max_vertical_pl_m(),
            max_velocity_pl_ms: default_max_velocity_pl_ms(),
            max_tmir_per_epoch: default_max_tmir_per_epoch(),
            require_valid_pl: default_require_valid_pl(),
        }
    }
}

impl From<IntegrityThresholdsConfig> for IntegrityThresholds {
    fn from(config: IntegrityThresholdsConfig) -> Self {
        Self {
            min_satellites_critical: config.min_satellites_critical,
            min_satellites_high: config.min_satellites_high,
            max_h_accuracy_m: config.max_h_accuracy_m,
            max_v_accuracy_m: config.max_v_accuracy_m,
            max_pdop: config.max_pdop,
            max_correction_age_s: config.max_correction_age_s,
            min_cno_degraded: config.min_cno_degraded,
            min_mean_cno_degraded: config.min_mean_cno_degraded,
            max_pvt_age_s: config.max_pvt_age_s,
            operational_threshold: config.operational_threshold,
            max_horizontal_pl_m: config.max_horizontal_pl_m,
            max_vertical_pl_m: config.max_vertical_pl_m,
            max_velocity_pl_ms: config.max_velocity_pl_ms,
            max_tmir_per_epoch: config.max_tmir_per_epoch,
            require_valid_pl: config.require_valid_pl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_thresholds() {
        let config = IntegrityThresholdsConfig::default();
        assert_eq!(config.min_satellites_critical, 4);
        assert_eq!(config.min_satellites_high, 6);
        assert!((config.max_h_accuracy_m - 0.1).abs() < 0.001);
        assert_eq!(config.operational_threshold, 1);
    }

    #[test]
    fn test_deserialize_partial() {
        let yaml = r#"
min_satellites_critical: 5
max_pdop: 2.5
"#;
        let config: IntegrityThresholdsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.min_satellites_critical, 5);
        assert_eq!(config.min_satellites_high, 6); // default
        assert!((config.max_pdop - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_deserialize_full_config() {
        let yaml = r#"
thresholds:
  min_satellites_critical: 3
  operational_threshold: 0
"#;
        let config: IntegrityConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.thresholds.min_satellites_critical, 3);
        assert_eq!(config.thresholds.operational_threshold, 0);
    }

    #[test]
    fn test_convert_to_integrity_thresholds() {
        let config = IntegrityThresholdsConfig {
            min_satellites_critical: 5,
            operational_threshold: 0,
            ..Default::default()
        };
        let thresholds: IntegrityThresholds = config.into();
        assert_eq!(thresholds.min_satellites_critical, 5);
        assert_eq!(thresholds.operational_threshold, 0);
    }
}
