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
///     usb:
///       in_ubx: true
///       in_rtcm3x: true
///       out_ubx: true
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

    /// Navigation engine configuration (CFG-NAVSPG-*)
    #[serde(default)]
    pub nav_spg: NavSpgConfig,

    /// Timepulse (PPS) configuration (CFG-TP-*)
    #[serde(default)]
    pub timepulse: Option<TimepulseConfig>,

    /// Base station position configuration (CFG-TMODE-*)
    ///
    /// For static_base mode: configure survey-in or fixed position.
    #[serde(default)]
    pub base_position: Option<BasePositionConfig>,

    /// Time mark (EXTINT) configuration
    ///
    /// Enable TIM-TM2 message output for external event timestamping.
    #[serde(default)]
    pub time_mark: Option<TimeMarkConfig>,
}

/// Dynamic platform model for the navigation engine.
///
/// This affects the navigation filter's assumptions about motion dynamics.
/// Choosing the correct model improves position accuracy and filter stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicModel {
    /// Low dynamics, general purpose (default)
    #[default]
    Portable,
    /// No movement expected - for base stations
    Stationary,
    /// Walking speed dynamics (<30 km/h)
    Pedestrian,
    /// Car-like dynamics (<100 km/h)
    Automotive,
    /// Sea-level, low vertical dynamics
    Sea,
    /// Airborne with <1g acceleration
    #[serde(alias = "airborne_1g")]
    AirborneLight,
    /// Airborne with <2g acceleration
    #[serde(alias = "airborne_2g")]
    AirborneMedium,
    /// Airborne with <4g acceleration
    #[serde(alias = "airborne_4g")]
    AirborneHigh,
    /// Wrist-worn device (smartwatch)
    Wrist,
    /// Bicycle dynamics
    Bike,
    /// Robotic lawn mower (low speed, frequent turns)
    Mower,
    /// E-scooter dynamics
    Escooter,
    /// Generic robot platform
    Robot,
}

impl DynamicModel {
    /// Convert to the u-blox protocol value.
    pub fn to_ublox_value(self) -> u8 {
        match self {
            DynamicModel::Portable => 0,
            DynamicModel::Stationary => 2,
            DynamicModel::Pedestrian => 3,
            DynamicModel::Automotive => 4,
            DynamicModel::Sea => 5,
            DynamicModel::AirborneLight => 6,
            DynamicModel::AirborneMedium => 7,
            DynamicModel::AirborneHigh => 8,
            DynamicModel::Wrist => 9,
            DynamicModel::Bike => 10,
            DynamicModel::Mower => 11,
            DynamicModel::Escooter => 12,
            DynamicModel::Robot => 13,
        }
    }
}

/// Navigation engine configuration (CFG-NAVSPG-*).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NavSpgConfig {
    /// Enable protection level calculation and output (CFG-NAVSPG-PL_ENA)
    #[serde(default)]
    pub pl_ena: Option<bool>,

    /// Dynamic platform model (CFG-NAVSPG-DYNMODEL)
    ///
    /// Configures the navigation filter for specific motion dynamics.
    /// Choose the model that best matches your application.
    #[serde(default)]
    pub dynamic_model: Option<DynamicModel>,

    /// Minimum elevation angle for satellites (CFG-NAVSPG-INFIL_MINELEV)
    ///
    /// Satellites below this elevation (in degrees) are rejected.
    /// Useful for reducing multipath in urban or forested environments.
    /// Valid range: 0-90 degrees.
    #[serde(default)]
    pub elevation_mask: Option<i8>,

    /// PDOP mask / threshold (CFG-NAVSPG-OUTFIL_PDOP)
    ///
    /// Solutions with PDOP above this value are rejected.
    /// Value is in PDOP units (e.g., 6.0 means reject if PDOP > 6.0).
    /// Valid range: 0.0-25.5 (stored as u16 scaled by 10).
    #[serde(default)]
    pub pdop_mask: Option<f32>,
}

/// Time grid reference for timepulse alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeGrid {
    /// UTC time reference
    #[default]
    Utc,
    /// GPS time reference
    Gps,
    /// GLONASS time reference
    Glonass,
    /// BeiDou time reference
    Beidou,
    /// Galileo time reference
    Galileo,
}

/// Timepulse (PPS) output configuration (CFG-TP-*).
///
/// Configures the precise timing pulse output on the TIMEPULSE pin.
/// This is commonly used for:
/// - Camera trigger synchronization
/// - LiDAR time alignment
/// - IMU timestamp calibration
/// - Multi-receiver time synchronization
///
/// # Example
/// ```yaml
/// timepulse:
///   enabled: true
///   frequency_hz: 1           # 1 PPS (one pulse per second)
///   pulse_length_us: 100000   # 100ms pulse width
///   polarity: rising          # Rising edge aligned to time
///   time_grid: gps            # Align to GPS time
///   lock_required: true       # Only pulse when fix is valid
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TimepulseConfig {
    /// Enable the timepulse output (CFG-TP-TP1_ENA)
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Output frequency in Hz when GNSS time is locked (CFG-TP-FREQ_LOCK_TP1)
    ///
    /// Common values: 1 (1 PPS), 10, 100, 1000, etc.
    /// Set to 1 for standard PPS output.
    #[serde(default)]
    pub frequency_hz: Option<u32>,

    /// Output frequency in Hz before GNSS lock (CFG-TP-FREQ_TP1)
    ///
    /// If not specified, defaults to same as frequency_hz.
    #[serde(default)]
    pub frequency_unlocked_hz: Option<u32>,

    /// Pulse length in microseconds when locked (CFG-TP-LEN_LOCK_TP1)
    ///
    /// For 1 PPS at 50% duty cycle, use 500000 (500ms).
    /// For a short trigger pulse, use 1000-100000 (1-100ms).
    #[serde(default)]
    pub pulse_length_us: Option<u32>,

    /// Pulse length in microseconds before lock (CFG-TP-LEN_TP1)
    ///
    /// If not specified, defaults to same as pulse_length_us.
    #[serde(default)]
    pub pulse_length_unlocked_us: Option<u32>,

    /// Polarity of the pulse (CFG-TP-POL_TP1)
    ///
    /// - `rising`: Rising edge at the time mark (default)
    /// - `falling`: Falling edge at the time mark
    #[serde(default)]
    pub polarity: Option<TimepulsePolarity>,

    /// Time grid to align the pulse to (CFG-TP-TIMEGRID_TP1)
    ///
    /// Options: utc, gps, glonass, beidou, galileo
    #[serde(default)]
    pub time_grid: Option<TimeGrid>,

    /// Align pulse to top of second (CFG-TP-ALIGN_TO_TOW_TP1)
    ///
    /// When true, pulses are aligned to the top of each second.
    #[serde(default)]
    pub align_to_tow: Option<bool>,

    /// Use locked parameters when GNSS time is available (CFG-TP-USE_LOCKED_TP1)
    ///
    /// When true, switches to locked frequency/length when fix is valid.
    /// When false, always uses unlocked parameters.
    #[serde(default)]
    pub use_locked_params: Option<bool>,

    /// Synchronize pulse to GNSS time (CFG-TP-SYNC_GNSS_TP1)
    ///
    /// When true, pulse timing is synchronized to GNSS.
    #[serde(default)]
    pub sync_to_gnss: Option<bool>,

    /// Antenna cable delay compensation in nanoseconds (CFG-TP-ANT_CABLE_DELAY)
    ///
    /// Compensates for signal propagation delay in the antenna cable.
    /// Typical values: 0-100ns depending on cable length.
    #[serde(default)]
    pub cable_delay_ns: Option<i16>,
}

/// Polarity of timepulse output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimepulsePolarity {
    /// Rising edge aligned to the time mark
    #[default]
    Rising,
    /// Falling edge aligned to the time mark
    Falling,
}

/// Base station position configuration (CFG-TMODE-*).
///
/// Configures the receiver for base station operation with either
/// survey-in (auto-determine position) or fixed (known coordinates) mode.
///
/// # Example - Survey-In Mode
/// ```yaml
/// base_position:
///   mode: survey_in
///   survey_in:
///     min_duration_s: 60
///     accuracy_limit_m: 2.0
/// ```
///
/// # Example - Fixed Position Mode
/// ```yaml
/// base_position:
///   mode: fixed
///   fixed:
///     latitude: -35.12345678
///     longitude: 149.12345678
///     height_m: 600.123
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BasePositionConfig {
    /// Base station mode
    #[serde(default)]
    pub mode: BasePositionMode,

    /// Survey-in configuration (used when mode = survey_in)
    #[serde(default)]
    pub survey_in: Option<SurveyInConfig>,

    /// Fixed position configuration (used when mode = fixed)
    #[serde(default)]
    pub fixed: Option<FixedPositionConfig>,
}

/// Base station position mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasePositionMode {
    /// Disabled - not operating as base station
    #[default]
    Disabled,
    /// Survey-in - automatically determine position
    SurveyIn,
    /// Fixed - use known coordinates
    Fixed,
}

/// Survey-in configuration for base station position determination.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SurveyInConfig {
    /// Minimum survey-in duration in seconds (CFG-TMODE-SVIN_MIN_DUR)
    ///
    /// The receiver will continue surveying for at least this long,
    /// even if accuracy is achieved earlier.
    /// Typical values: 60-300 seconds.
    #[serde(default)]
    pub min_duration_s: Option<u32>,

    /// Required accuracy limit in meters (CFG-TMODE-SVIN_ACC_LIMIT)
    ///
    /// Survey-in completes when this 3D accuracy is achieved
    /// (and min_duration has elapsed).
    /// Typical values: 1.0-5.0 meters.
    #[serde(default)]
    pub accuracy_limit_m: Option<f32>,
}

/// Fixed position configuration for base station with known coordinates.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FixedPositionConfig {
    /// Latitude in degrees (WGS84)
    #[serde(default)]
    pub latitude: Option<f64>,

    /// Longitude in degrees (WGS84)
    #[serde(default)]
    pub longitude: Option<f64>,

    /// Height above ellipsoid in meters (WGS84)
    #[serde(default)]
    pub height_m: Option<f64>,

    /// Position accuracy estimate in meters (CFG-TMODE-FIXED_POS_ACC)
    ///
    /// Used by the receiver to weight the fixed position.
    /// Lower values indicate higher confidence in the coordinates.
    #[serde(default)]
    pub accuracy_m: Option<f32>,
}

/// Time mark (EXTINT) configuration for external event timestamping.
///
/// Enables TIM-TM2 message output which provides precise GNSS-synchronized
/// timestamps when an external signal is detected on the EXTINT pin.
///
/// # Example
/// ```yaml
/// time_mark:
///   enabled: true
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TimeMarkConfig {
    /// Enable TIM-TM2 message output
    ///
    /// When enabled, the receiver outputs TIM-TM2 messages when
    /// external events are detected on the EXTINT pin.
    #[serde(default)]
    pub enabled: Option<bool>,
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
///
/// Each port has input and output protocol settings. Each protocol can be
/// explicitly enabled (true), disabled (false), or left unchanged (not specified).
///
/// # Example
/// ```yaml
/// protocols:
///   usb:
///     in_ubx: true
///     in_nmea: false
///     in_rtcm3x: true
///     out_ubx: true
///     out_nmea: false
///   uart2:
///     in_rtcm3x: true
///     out_rtcm3x: true
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProtocolConfig {
    /// USB port protocols
    #[serde(default)]
    pub usb: PortProtocols,

    /// UART1 port protocols
    #[serde(default)]
    pub uart1: PortProtocols,

    /// UART2 port protocols
    #[serde(default)]
    pub uart2: PortProtocols,

    /// I2C (DDC) port protocols
    #[serde(default)]
    pub i2c: PortProtocols,

    /// SPI port protocols
    #[serde(default)]
    pub spi: PortProtocols,
}

/// Protocol enable/disable settings for a single port.
///
/// All fields are optional - if not specified, the device default is kept.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PortProtocols {
    /// Enable UBX protocol input
    #[serde(default)]
    pub in_ubx: Option<bool>,

    /// Enable NMEA protocol input
    #[serde(default)]
    pub in_nmea: Option<bool>,

    /// Enable RTCM3X protocol input
    #[serde(default)]
    pub in_rtcm3x: Option<bool>,

    /// Enable SPARTN protocol input (I2C/SPI only)
    #[serde(default)]
    pub in_spartn: Option<bool>,

    /// Enable UBX protocol output
    #[serde(default)]
    pub out_ubx: Option<bool>,

    /// Enable NMEA protocol output
    #[serde(default)]
    pub out_nmea: Option<bool>,

    /// Enable RTCM3X protocol output
    #[serde(default)]
    pub out_rtcm3x: Option<bool>,
}

impl PortProtocols {
    /// Check if any protocol setting is configured (not None).
    pub fn has_any_set(&self) -> bool {
        self.in_ubx.is_some()
            || self.in_nmea.is_some()
            || self.in_rtcm3x.is_some()
            || self.in_spartn.is_some()
            || self.out_ubx.is_some()
            || self.out_nmea.is_some()
            || self.out_rtcm3x.is_some()
    }
}

impl From<&super::modes::PortProtocols> for PortProtocols {
    fn from(p: &super::modes::PortProtocols) -> Self {
        Self {
            in_ubx: Some(p.ubx_in),
            in_nmea: Some(p.nmea_in),
            in_rtcm3x: Some(p.rtcm3x_in),
            out_ubx: Some(p.ubx_out),
            out_nmea: Some(p.nmea_out),
            out_rtcm3x: Some(p.rtcm3x_out),
            ..Default::default()
        }
    }
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

    /// I2C (DDC) port enabled
    #[serde(default)]
    pub i2c_enabled: Option<bool>,

    /// SPI port enabled
    #[serde(default)]
    pub spi_enabled: Option<bool>,
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
        reason: "High-precision position enhances ~/fix accuracy (mm-level when RTK fixed)",
        ros_topics: &["~/fix"],
    },
    MessageRequirement {
        message: "NAV_POSECEF",
        level: MessageLevel::Optional,
        reason: "ECEF coordinates for coordinate transforms (optional)",
        ros_topics: &[],
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
        ros_topics: &["~/integrity"],
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
            || self.protocols.usb.has_any_set()
            || self.protocols.uart1.has_any_set()
            || self.protocols.uart2.has_any_set()
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
    /// If `enabled_topics` is provided, only checks messages relevant to those topics.
    /// This makes validation mode-aware - it won't warn about messages for features
    /// that aren't enabled.
    ///
    /// Returns a ValidationResult with details about missing messages.
    pub fn validate(&self, enabled_topics: Option<&[&str]>) -> ValidationResult {
        use tracing::{error, info};

        let mut result = ValidationResult::default();

        for req in MESSAGE_REQUIREMENTS {
            let enabled = self.is_message_enabled(req.message);

            if !enabled {
                // For RequiredForFeature messages, only warn if one of its topics is enabled
                if req.level == MessageLevel::RequiredForFeature {
                    if let Some(topics) = enabled_topics {
                        let is_relevant = req.ros_topics.iter().any(|t| topics.contains(t));
                        if !is_relevant {
                            // Skip - this feature isn't enabled
                            continue;
                        }
                    }
                }

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
                        // Only warn about recommended messages if their topics are enabled
                        let is_relevant = if let Some(topics) = enabled_topics {
                            req.ros_topics.is_empty()
                                || req.ros_topics.iter().any(|t| topics.contains(t))
                        } else {
                            true
                        };
                        if is_relevant {
                            warn!(
                                message = req.message,
                                reason = req.reason,
                                topics = ?req.ros_topics,
                                "Recommended message not enabled - some features will be unavailable"
                            );
                            result.missing_recommended.push(req);
                        }
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
    ///
    /// If `enabled_topics` is provided, only checks those topics.
    /// Otherwise checks all possible topics (legacy behavior).
    pub fn check_topic_availability(
        &self,
        enabled_topics: Option<&[&str]>,
    ) -> Vec<(&'static str, Vec<&'static str>)> {
        let mut unavailable = Vec::new();

        // Collect all known topics from requirements (these are 'static)
        let all_topics: Vec<&'static str> = MESSAGE_REQUIREMENTS
            .iter()
            .flat_map(|m| m.ros_topics.iter().copied())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for topic in all_topics {
            // If enabled_topics is provided, skip topics not in the list
            if let Some(enabled) = enabled_topics {
                if !enabled.contains(&topic) {
                    continue;
                }
            }

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
  usb:
    in_ubx: true
    in_rtcm3x: true
    out_ubx: true
messages:
  usb:
    NAV_PVT: 1
    NAV_HPPOSLLH: 1
"#;
        let config: UbloxConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.family, Some("F9P".to_string()));
        assert_eq!(config.rate.measurement_ms, 100);
        assert_eq!(config.rate.nav_ratio, 1);
        assert_eq!(config.protocols.usb.in_ubx, Some(true));
        assert_eq!(config.protocols.usb.in_rtcm3x, Some(true));
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
        let result = config.validate(None);
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
        let result = config.validate(None);
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
        let result = config.validate(None);
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
        let unavailable = config.check_topic_availability(None);
        // ~/integrity should be unavailable (needs SEC_SIG, MON_RF, etc.)
        let integrity = unavailable.iter().find(|(t, _)| *t == "~/integrity");
        assert!(integrity.is_some());
        assert!(integrity.unwrap().1.contains(&"SEC_SIG"));
    }

    #[test]
    fn test_messages_for_topic() {
        let msgs = messages_for_topic("~/fix");
        assert!(msgs.iter().any(|m| m.message == "NAV_PVT"));
    }
}
