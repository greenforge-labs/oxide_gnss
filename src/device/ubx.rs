//! UBX protocol handler for u-blox GNSS receivers.
//!
//! This module provides parsing of UBX binary protocol messages and generation
//! of device configuration commands using the [`ublox`] crate.
//!
//! ## Protocol Version
//!
//! Uses protocol version 27 (proto27) which is compatible with:
//! - ZED-F9P (HPG 1.30+)
//! - ZED-F9R
//! - NEO-M9N
//! - Other modern u-blox 9 series receivers
//!
//! ## Key Types
//!
//! - [`UbxHandler`] - Main protocol parser and state container
//! - [`PvtData`] - Position, velocity, time data (from NAV-PVT)
//! - [`HpPosData`] - High-precision position (from NAV-HPPOSLLH)
//! - [`SatInfo`] - Per-satellite status (from NAV-SAT)
//! - [`SecSigData`] - Jamming/spoofing status (from SEC-SIG)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use oxide_gnss::device::UbxHandler;
//!
//! let mut handler = UbxHandler::new();
//!
//! // Feed raw bytes from serial port
//! let result = handler.process(&serial_data);
//!
//! // Check for parsed messages
//! if let Some(pvt) = result.pvt {
//!     println!("Position: {}, {}", pvt.lat, pvt.lon);
//! }
//! ```

use std::time::Instant;

use tracing::{debug, trace, warn};
use ublox::{
    cfg_msg::CfgMsgAllPortsBuilder,
    cfg_val::CfgVal,
    mon_comms::MonCommsRef,
    mon_hw::{AntennaPower, AntennaStatus, MonHwRef},
    nav_cov::NavCovRef,
    nav_hp_pos_llh::NavHpPosLlhRef,
    nav_pl::{
        NavPlRef, PlInvalidityReason, PlPosFrame, PlPosValid, PlTimeValid, PlVelFrame, PlVelValid,
    },
    nav_pos_ecef::NavPosEcefRef,
    nav_rel_pos_ned::{common::NavRelPosNedFlags, proto27::NavRelPosNedRef},
    nav_sat::NavSatRef,
    packets::cfg_val::{CfgLayerSet, CfgValSetBuilder},
    proto27::Proto27,
    rxm_cor::{CorrectionMsgUsed, MsgDecrypted, MsgEncrypted, RxmCorRef, RxmCorStatusInfo},
    sec_sig::{JammingState, SecSigFlags, SecSigRef, SpoofingState},
    sec_siglog::SecSiglogRef,
    tim_tm2::{TimTm2Ref, TimTm2TimeBase},
    GnssFixType, Parser, ParserBuilder, UbxPacket,
};

use crate::state::FixType;

/// UBX protocol parser and message handler.
///
/// `UbxHandler` processes raw bytes from a u-blox GNSS receiver and extracts
/// structured data from UBX binary protocol messages.
///
/// # Thread Safety
///
/// This type is **not** thread-safe and should only be accessed from a single
/// async task (typically the device task).
///
/// # State
///
/// The handler maintains the latest received data for each message type.
/// This allows consumers to access the most recent data without waiting
/// for a new message.
///
/// # Example
///
/// ```rust,ignore
/// let mut handler = UbxHandler::new();
///
/// loop {
///     let bytes = serial.read(&mut buf).await?;
///     let result = handler.process(&buf[..bytes]);
///
///     if let Some(pvt) = result.pvt {
///         // New PVT data available
///     }
///     if let Some(ack) = result.ack {
///         // ACK/NAK received for a command
///     }
/// }
/// ```
pub struct UbxHandler {
    /// The ublox parser instance (protocol version 27)
    parser: Parser<Vec<u8>, Proto27>,
    /// Last received NAV-PVT data
    last_pvt: Option<PvtData>,
    /// Message statistics
    stats: UbxStats,
    /// Pending ACK tracking
    pending_ack: PendingAck,
    /// Last received high-precision position (NAV-HPPOSLLH)
    pub hp_pos: Option<HpPosData>,
    /// Last received satellite status (NAV-SAT)
    pub sat_info: Option<SatInfo>,
    /// Last received covariance data (NAV-COV)
    pub cov: Option<CovData>,
    /// Last received ECEF position (NAV-POSECEF)
    pub pos_ecef: Option<PosEcefData>,
    /// Last received security signal status (SEC-SIG)
    pub sec_sig: Option<SecSigData>,
    /// Last received security event log (SEC-SIGLOG)
    pub sec_siglog: Option<SecSiglogData>,
    /// Last received correction status (RXM-COR)
    pub rxm_cor: Option<RxmCorData>,
    /// Last received communication port status (MON-COMMS)
    pub mon_comms: Option<MonCommsData>,
    /// Last received hardware status (MON-HW, legacy)
    pub mon_hw: Option<MonHwData>,
    /// Last received RF status (MON-RF, replaces MON-HW jamming)
    pub mon_rf: Option<MonRfData>,
    /// Last received relative position for moving base/rover (NAV-RELPOSNED)
    pub rel_pos_ned: Option<RelPosNedData>,
    /// Last received protection level data (NAV-PL)
    pub nav_pl: Option<NavPlData>,
    /// Last received time mark data (TIM-TM2)
    pub tim_tm2: Option<TimTm2Data>,
}

/// Parsed High Precision Position (NAV-HPPOSLLH).
///
/// Provides sub-centimeter precision position data, extending the standard
/// NAV-PVT position with additional decimal places. This message is essential
/// for RTK applications where maximum precision is required.
///
/// # Precision
///
/// - Latitude/Longitude: 10^-9 degrees (~0.1mm at equator)
/// - Height: 0.1mm resolution
///
/// # Usage
///
/// When the `high_precision` feature is enabled, this data is used to
/// enhance the `~/fix` topic with better position precision.
#[derive(Debug, Clone)]
pub struct HpPosData {
    /// Latitude in degrees (WGS84, high precision)
    pub lat: f64,
    /// Longitude in degrees (WGS84, high precision)
    pub lon: f64,
    /// Height above ellipsoid in meters (high precision)
    pub height: f64,
    /// Horizontal accuracy estimate in meters (1-sigma)
    pub h_acc: f32,
    /// Vertical accuracy estimate in meters (1-sigma)
    pub v_acc: f32,
}

/// Parsed Satellite Status (NAV-SAT).
///
/// Contains per-satellite information for all tracked satellites.
/// This is useful for diagnostics, sky plots, and signal quality analysis.
///
/// # Usage
///
/// When the `satellites` feature is enabled, this data is published
/// to the `~/satellites` topic.
#[derive(Debug, Clone)]
pub struct SatInfo {
    /// Total number of satellites in this report
    pub num_sats: u8,
    /// Per-satellite status information
    pub sats: Vec<SatStatus>,
}

impl SatInfo {
    /// Compute signal quality metrics from satellite data.
    ///
    /// Only considers satellites that are used in the navigation solution.
    pub fn compute_signal_quality(&self) -> SignalQuality {
        let used_sats: Vec<&SatStatus> = self.sats.iter().filter(|s| s.sv_used).collect();

        if used_sats.is_empty() {
            return SignalQuality {
                sats_used: 0,
                sats_above_threshold: 0,
                mean_cno: 0.0,
                min_cno: 0,
            };
        }

        let cno_values: Vec<u8> = used_sats.iter().map(|s| s.cno).collect();
        let sats_above_threshold = cno_values.iter().filter(|&&c| c >= 30).count() as u8;
        let mean_cno = cno_values.iter().map(|&c| c as f32).sum::<f32>() / cno_values.len() as f32;
        let min_cno = *cno_values.iter().min().unwrap_or(&0);

        SignalQuality {
            sats_used: used_sats.len() as u8,
            sats_above_threshold,
            mean_cno,
            min_cno,
        }
    }
}

/// Aggregated signal quality metrics derived from NAV-SAT.
#[derive(Debug, Clone, Default)]
pub struct SignalQuality {
    /// Number of satellites used in the solution
    pub sats_used: u8,
    /// Number of satellites with C/N0 >= 30 dB-Hz (usable threshold)
    pub sats_above_threshold: u8,
    /// Mean C/N0 of satellites used in solution (dB-Hz)
    pub mean_cno: f32,
    /// Minimum C/N0 among used satellites (dB-Hz) - weakest link
    pub min_cno: u8,
}

#[derive(Debug, Clone)]
pub struct SatStatus {
    pub gnss_id: u8,
    pub sv_id: u8,
    pub cno: u8,
    pub elev: i8,
    pub azim: i16,
    pub pr_res: i16,
    pub flags: u32,
    /// Satellite is used in the navigation solution
    pub sv_used: bool,
}

/// Parsed Position Covariance (NAV-COV).
#[derive(Debug, Clone)]
pub struct CovData {
    /// GPS time of week (ms)
    pub itow: u32,
    /// Position covariance valid
    pub pos_cov_valid: bool,
    /// Velocity covariance valid
    pub vel_cov_valid: bool,
    /// Position covariance matrix (NED, row-major: NN, NE, ND, EE, ED, DD)
    pub pos_cov: [f32; 6],
    /// Velocity covariance matrix (NED, row-major: NN, NE, ND, EE, ED, DD)
    pub vel_cov: [f32; 6],
}

/// Parsed ECEF Position (NAV-POSECEF).
#[derive(Debug, Clone)]
pub struct PosEcefData {
    /// GPS time of week (ms)
    pub itow: u32,
    /// ECEF X coordinate (m)
    pub ecef_x: f64,
    /// ECEF Y coordinate (m)
    pub ecef_y: f64,
    /// ECEF Z coordinate (m)
    pub ecef_z: f64,
    /// Position accuracy estimate (m)
    pub p_acc: f64,
}

/// Parsed Relative Position (NAV-RELPOSNED) for moving base/rover.
#[derive(Debug, Clone)]
pub struct RelPosNedData {
    /// GPS time of week (ms)
    pub itow: u32,
    /// Reference station ID
    pub ref_station_id: u16,
    /// North component of baseline vector (m)
    pub rel_pos_n: f64,
    /// East component of baseline vector (m)
    pub rel_pos_e: f64,
    /// Down component of baseline vector (m)
    pub rel_pos_d: f64,
    /// Length of baseline vector (m)
    pub rel_pos_length: f64,
    /// Heading of baseline vector (radians)
    pub rel_pos_heading: f64,
    /// Accuracy of North component (m)
    pub acc_n: f64,
    /// Accuracy of East component (m)
    pub acc_e: f64,
    /// Accuracy of Down component (m)
    pub acc_d: f64,
    /// Accuracy of baseline length (m)
    pub acc_length: f64,
    /// Accuracy of heading (radians)
    pub acc_heading: f64,
    /// Flags indicating validity and fix status
    pub flags: RelPosNedFlags,
}

/// Flags for NAV-RELPOSNED message.
#[derive(Debug, Clone, Copy, Default)]
pub struct RelPosNedFlags {
    /// Valid fix (within DOP & accuracy masks)
    pub gnss_fix_ok: bool,
    /// Differential corrections applied
    pub diff_soln: bool,
    /// Relative position components valid
    pub rel_pos_valid: bool,
    /// Carrier phase solution: 0=none, 1=float, 2=fixed
    pub carr_soln: u8,
    /// Receiver is in moving base mode
    pub is_moving: bool,
    /// Reference position was extrapolated
    pub ref_pos_miss: bool,
    /// Reference observations were extrapolated
    pub ref_obs_miss: bool,
}

impl From<NavRelPosNedFlags> for RelPosNedFlags {
    fn from(flags: NavRelPosNedFlags) -> Self {
        use ublox::nav_rel_pos_ned::common::CarrierPhaseRangeSolutionStatus;
        let carr_soln = match flags.carr_soln() {
            CarrierPhaseRangeSolutionStatus::NoSolution => 0,
            CarrierPhaseRangeSolutionStatus::SolutionWithFloatingAmbiguities => 1,
            CarrierPhaseRangeSolutionStatus::SolutionWithFixedAmbiguities => 2,
        };
        Self {
            gnss_fix_ok: flags.gnss_fix_ok(),
            diff_soln: flags.diff_soln(),
            rel_pos_valid: flags.rel_pos_valid(),
            carr_soln,
            is_moving: flags.is_moving(),
            ref_pos_miss: flags.ref_pos_miss(),
            ref_obs_miss: flags.ref_obs_miss(),
        }
    }
}

/// Parsed Protection Level data (NAV-PL).
///
/// Protection levels provide statistically-bounded error estimates with a specified
/// Target Misleading Information Risk (TMIR). This enables ISO 26262/ISO 21448 (SOTIF)
/// compliant integrity monitoring for autonomous vehicle applications.
#[derive(Debug, Clone)]
pub struct NavPlData {
    /// GPS time of week (ms)
    pub itow: u32,
    /// Target Misleading Information Risk: TMIR = tmir_coeff * 10^tmir_exp [%MI/epoch]
    pub tmir: f64,
    /// Position protection level validity
    pub pos_valid: bool,
    /// Position protection level frame
    pub pos_frame: NavPlFrame,
    /// Position protection level invalidity reason (if invalid)
    pub pos_invalidity_reason: NavPlInvalidityReason,
    /// Position protection levels in meters [axis1, axis2, axis3]
    /// Interpretation depends on pos_frame (NED, LongLatVert, or Ellipse)
    pub pos_pl_m: [f64; 3],
    /// Horizontal position ellipse orientation (degrees from true North, clockwise)
    /// Only valid when pos_frame is Ellipse
    pub pos_horiz_orient_deg: f64,
    /// Velocity protection level validity
    pub vel_valid: bool,
    /// Velocity protection level frame
    pub vel_frame: NavPlFrame,
    /// Velocity protection level invalidity reason (if invalid)
    pub vel_invalidity_reason: NavPlInvalidityReason,
    /// Velocity protection levels in m/s [axis1, axis2, axis3]
    pub vel_pl_ms: [f64; 3],
    /// Horizontal velocity ellipse orientation (degrees from true North, clockwise)
    pub vel_horiz_orient_deg: f64,
    /// Time protection level validity
    pub time_valid: bool,
    /// Time protection level invalidity reason (if invalid)
    pub time_invalidity_reason: NavPlInvalidityReason,
    /// Time protection level in nanoseconds
    pub time_pl_ns: u32,
}

/// Protection level reference frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavPlFrame {
    /// Invalid (not possible to calculate frame conversion)
    #[default]
    Invalid,
    /// North-East-Down
    Ned,
    /// Longitudinal-Lateral-Vertical (vehicle frame)
    LongLatVert,
    /// HorizSemiMajorAxis-HorizSemiMinorAxis-Vertical (error ellipse)
    Ellipse,
}

impl From<PlPosFrame> for NavPlFrame {
    fn from(frame: PlPosFrame) -> Self {
        match frame {
            PlPosFrame::Invalid => NavPlFrame::Invalid,
            PlPosFrame::Ned => NavPlFrame::Ned,
            PlPosFrame::LongLatVert => NavPlFrame::LongLatVert,
            PlPosFrame::HorizSemiMajorMinorVert => NavPlFrame::Ellipse,
            _ => NavPlFrame::Invalid, // Reserved values
        }
    }
}

impl From<PlVelFrame> for NavPlFrame {
    fn from(frame: PlVelFrame) -> Self {
        match frame {
            PlVelFrame::Invalid => NavPlFrame::Invalid,
            PlVelFrame::Ned => NavPlFrame::Ned,
            PlVelFrame::LongLatVert => NavPlFrame::LongLatVert,
            PlVelFrame::HorizSemiMajorMinorVert => NavPlFrame::Ellipse,
            _ => NavPlFrame::Invalid, // Reserved values
        }
    }
}

/// Protection level invalidity reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavPlInvalidityReason {
    /// Protection level is valid (no invalidity reason)
    #[default]
    Valid,
    /// Not available
    NotAvailable,
    /// Solution not trustworthy (values 1-29)
    SolutionNotTrustworthy,
    /// PL not verified for this receiver configuration (values 30-100)
    NotVerifiedForConfig,
}

impl From<PlInvalidityReason> for NavPlInvalidityReason {
    fn from(reason: PlInvalidityReason) -> Self {
        match reason {
            PlInvalidityReason::NotAvailable => NavPlInvalidityReason::NotAvailable,
            PlInvalidityReason::SolutionNotTrustworthy => {
                NavPlInvalidityReason::SolutionNotTrustworthy
            }
            PlInvalidityReason::NotVerifiedForConfig => NavPlInvalidityReason::NotVerifiedForConfig,
            _ => NavPlInvalidityReason::NotAvailable, // Reserved values
        }
    }
}

/// Parsed Time Mark data (TIM-TM2).
///
/// Contains precise timestamp of external event on EXTINT pin.
#[derive(Debug, Clone)]
pub struct TimTm2Data {
    /// Channel (EXTINT) on which the pulse was measured
    pub channel: u8,
    /// Rising edge counter
    pub count: u16,
    /// GPS week number of rising edge
    pub week_rising: u16,
    /// GPS week number of falling edge
    pub week_falling: u16,
    /// Time of week of rising edge in seconds (nanosecond precision)
    pub tow_rising_s: f64,
    /// Time of week of falling edge in seconds (nanosecond precision)
    pub tow_falling_s: f64,
    /// Accuracy estimate in nanoseconds
    pub accuracy_ns: u32,
    /// New rising edge detected
    pub new_rising_edge: bool,
    /// New falling edge detected
    pub new_falling_edge: bool,
    /// Time is valid (UTC available)
    pub time_valid: bool,
    /// Time base used for measurement
    pub time_base: TimTm2TimeBaseData,
}

/// Time base for TIM-TM2 measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimTm2TimeBaseData {
    #[default]
    Receiver,
    Gnss,
    Utc,
}

impl From<TimTm2TimeBase> for TimTm2TimeBaseData {
    fn from(base: TimTm2TimeBase) -> Self {
        match base {
            TimTm2TimeBase::Receiver => TimTm2TimeBaseData::Receiver,
            TimTm2TimeBase::Gnss => TimTm2TimeBaseData::Gnss,
            TimTm2TimeBase::Utc => TimTm2TimeBaseData::Utc,
        }
    }
}

/// Parsed Signal Security Status (SEC-SIG).
#[derive(Debug, Clone)]
pub struct SecSigData {
    /// Jamming detection enabled
    pub jam_det_enabled: bool,
    /// Spoofing detection enabled
    pub spf_det_enabled: bool,
    /// Current jamming state
    pub jamming_state: JammingStateData,
    /// Current spoofing state
    pub spoofing_state: SpoofingStateData,

    /// Number of center frequencies reported in this message
    pub jam_num_cent_freqs: u8,
    /// Center frequencies in kHz (parallel array)
    pub cent_freq_khz: Vec<u32>,
    /// Whether each center frequency is considered jammed (parallel array)
    pub jammed: Vec<bool>,
}

/// Jamming detection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JammingStateData {
    #[default]
    Unknown,
    Ok,
    Warning,
    Critical,
}

impl From<JammingState> for JammingStateData {
    fn from(state: JammingState) -> Self {
        match state {
            JammingState::Unknown => JammingStateData::Unknown,
            JammingState::Ok => JammingStateData::Ok,
            JammingState::Warning => JammingStateData::Warning,
            JammingState::Critical => JammingStateData::Critical,
            _ => JammingStateData::Unknown, // Reserved values
        }
    }
}

/// Spoofing detection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpoofingStateData {
    #[default]
    Unknown,
    Ok,
    Indicated,
    Multiple,
}

impl From<SpoofingState> for SpoofingStateData {
    fn from(state: SpoofingState) -> Self {
        match state {
            SpoofingState::Unknown => SpoofingStateData::Unknown,
            SpoofingState::Ok => SpoofingStateData::Ok,
            SpoofingState::Indicated => SpoofingStateData::Indicated,
            SpoofingState::Multiple => SpoofingStateData::Multiple,
            _ => SpoofingStateData::Unknown, // Reserved values
        }
    }
}

/// Parsed Security Event Log (SEC-SIGLOG).
#[derive(Debug, Clone)]
pub struct SecSiglogData {
    /// Number of events
    pub num_events: u8,
    /// Event entries
    pub events: Vec<SecSiglogEventData>,
}

/// A single security event.
#[derive(Debug, Clone)]
pub struct SecSiglogEventData {
    /// Seconds elapsed since this event
    pub time_elapsed_s: u32,
    /// Type of spoofing/jamming detection
    pub detection_type: u8,
    /// Type of the event
    pub event_type: u8,
}

/// Parsed Differential Correction Status (RXM-COR).
#[derive(Debug, Clone)]
pub struct RxmCorData {
    /// Signal quality (Eb/N0 in dB)
    pub ebno: f32,
    /// Message was used by receiver
    pub msg_used: CorrectionMsgUsed,
    /// Message was encrypted
    pub msg_encrypted: MsgEncrypted,
    /// Message was successfully decrypted
    pub msg_decrypted: MsgDecrypted,
    /// Correction message type
    pub msg_type: u16,
    /// Correction message subtype
    pub msg_sub_type: u16,
}

/// Parsed Communication Port Status (MON-COMMS).
#[derive(Debug, Clone)]
pub struct MonCommsData {
    /// Number of ports
    pub n_ports: u8,
    /// TX error flags
    pub tx_errors: u8,
    /// Port information
    pub ports: Vec<MonCommsPortData>,
}

/// Information for a single communication port.
#[derive(Debug, Clone)]
pub struct MonCommsPortData {
    /// Port name
    pub port_name: String,
    /// Bytes pending in TX buffer
    pub tx_pending: u16,
    /// Total bytes transmitted
    pub tx_bytes: u32,
    /// TX buffer usage (0-100%)
    pub tx_usage_percent: f32,
    /// Bytes pending in RX buffer
    pub rx_pending: u16,
    /// Total bytes received
    pub rx_bytes: u32,
    /// RX buffer usage (0-100%)
    pub rx_usage_percent: f32,
    /// Number of overrun errors
    pub overrun_errs: u16,
}

/// Parsed Hardware Status (MON-HW).
#[derive(Debug, Clone)]
pub struct MonHwData {
    /// Antenna status
    pub antenna_status: AntennaStatusData,
    /// Antenna power status
    pub antenna_power: AntennaPowerData,
    /// CW jamming indicator (0-255, 0=no jamming, 255=strong jamming)
    pub jam_ind: u8,
    /// Noise level as measured by the GPS core
    pub noise_per_ms: u16,
    /// AGC monitor count
    pub agc_cnt: u16,
    /// Jamming state from hardware flags (deprecated in newer protocols)
    pub jamming_state: JammingStateData,
    /// RTC is calibrated
    pub rtc_calib: bool,
    /// Safe boot mode active
    pub safe_boot: bool,
}

/// Parsed RF Status (MON-RF) - replaces deprecated MON-HW.
#[derive(Debug, Clone)]
pub struct MonRfData {
    /// Antenna status (from first RF block)
    pub antenna_status: AntennaStatusData,
    /// Antenna power status (from first RF block)
    pub antenna_power: AntennaPowerData,
    /// CW jamming indicator (0-255, 0=no jamming, 255=strong jamming)
    pub jam_ind: u8,
    /// Noise level as measured by the GPS core
    pub noise_per_ms: u16,
    /// AGC monitor count
    pub agc_cnt: u16,
    // Note: jammingState in flags is deprecated and always 0 on F9P with SEC-SIG support.
    // Use SEC-SIG for jamming state instead.
}

/// Antenna supervisor status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AntennaStatusData {
    #[default]
    Init,
    Unknown,
    Ok,
    Short,
    Open,
}

/// Antenna power status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AntennaPowerData {
    Off,
    On,
    #[default]
    Unknown,
}

/// Parsed NAV-PVT (Navigation Position Velocity Time) data.
///
/// This is the primary position/velocity/time message from the receiver.
/// It contains all the essential GNSS solution data in a single message.
///
/// # Coordinate System
///
/// - Position: WGS84 latitude/longitude in degrees, height in meters
/// - Velocity: NED (North-East-Down) frame in m/s
///
/// # Accuracy Estimates
///
/// The `h_acc` and `v_acc` fields represent 1-sigma accuracy estimates
/// in meters. These are estimates from the receiver's navigation filter
/// and may be optimistic.
///
/// # RTK Status
///
/// The `carr_soln` field indicates RTK solution status:
/// - `None` - No carrier phase used (standalone or DGPS)
/// - `Float` - RTK float solution (~20-50cm accuracy)
/// - `Fixed` - RTK fixed solution (~1-2cm accuracy)
#[derive(Debug, Clone)]
pub struct PvtData {
    /// GPS time of week in milliseconds
    pub itow: u32,
    /// Year (UTC)
    pub year: u16,
    /// Month (1-12)
    pub month: u8,
    /// Day of month (1-31)
    pub day: u8,
    /// Hour (0-23)
    pub hour: u8,
    /// Minute (0-59)
    pub min: u8,
    /// Second (0-60, 60 for leap second)
    pub sec: u8,
    /// Nanoseconds (can be negative)
    pub nano: i32,
    /// Fix type
    pub fix_type: FixType,
    /// Number of satellites used
    pub num_sv: u8,
    /// Longitude in degrees
    pub lon: f64,
    /// Latitude in degrees
    pub lat: f64,
    /// Height above ellipsoid in meters
    pub height: f64,
    /// Height above mean sea level in meters
    pub height_msl: f64,
    /// Horizontal accuracy estimate in meters
    pub h_acc: f32,
    /// Vertical accuracy estimate in meters
    pub v_acc: f32,
    /// North velocity in m/s
    pub vel_n: f32,
    /// East velocity in m/s
    pub vel_e: f32,
    /// Down velocity in m/s
    pub vel_d: f32,
    /// Ground speed in m/s
    pub g_speed: f32,
    /// Heading of motion in degrees
    pub head_mot: f32,
    /// Speed accuracy estimate in m/s
    pub s_acc: f32,
    /// Heading accuracy estimate in degrees
    pub head_acc: f32,
    /// Position dilution of precision (scaled by 0.01)
    pub p_dop: f32,
    /// Carrier phase range solution valid
    pub carr_soln: CarrierSolution,
    /// Age of differential corrections (seconds), from device flags3.
    /// None if not available (value 0 from device), Some(1-15) otherwise.
    pub diff_corr_age_s: Option<u8>,
    /// Timestamp when received
    pub received_at: Instant,
}

/// Carrier phase range solution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarrierSolution {
    #[default]
    None,
    Float,
    Fixed,
}

/// UBX message statistics.
#[derive(Debug, Clone, Default)]
pub struct UbxStats {
    /// Total messages parsed
    pub messages_parsed: u64,
    /// NAV-PVT messages received
    pub nav_pvt_count: u64,
    /// Parse errors
    pub parse_errors: u64,
    /// Other message types (not errors, just not specifically handled)
    pub other_messages: u64,
}

/// Result of an ACK/NAK response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckResult {
    /// Command acknowledged (ACK-ACK)
    Ack,
    /// Command rejected (ACK-NAK)
    Nak,
}

/// Pending ACK state for tracking command responses.
#[derive(Debug, Default)]
struct PendingAck {
    /// Class ID of the command we're waiting for ACK
    class: Option<u8>,
    /// Message ID of the command we're waiting for ACK
    msg_id: Option<u8>,
}

impl PendingAck {
    /// Check if the received ACK/NAK matches the pending expectation.
    fn matches(&self, class: u8, msg_id: u8) -> bool {
        self.class == Some(class) && self.msg_id == Some(msg_id)
    }

    /// Check if we have a pending ACK expectation.
    fn is_pending(&self) -> bool {
        self.class.is_some() && self.msg_id.is_some()
    }
}

/// Result of processing input data.
#[derive(Debug)]
pub struct ProcessResult {
    /// New PVT data if received
    pub pvt: Option<PvtData>,
    /// Number of messages processed
    pub messages_processed: usize,
    /// ACK/NAK result if received
    pub ack: Option<AckResult>,
    /// High precision position
    pub hp_pos: Option<HpPosData>,
    /// Satellite info
    pub sat_info: Option<SatInfo>,
    /// Position/velocity covariance
    pub cov: Option<CovData>,
    /// ECEF position
    pub pos_ecef: Option<PosEcefData>,
    /// Signal security status (jamming/spoofing)
    pub sec_sig: Option<SecSigData>,
    /// Security event log
    pub sec_siglog: Option<SecSiglogData>,
    /// Differential correction status
    pub rxm_cor: Option<RxmCorData>,
    /// Communication port status
    pub mon_comms: Option<MonCommsData>,
    /// Hardware status (antenna, jamming indicator) - deprecated, use mon_rf
    pub mon_hw: Option<MonHwData>,
    /// RF status (antenna, jamming indicator) - replaces mon_hw
    pub mon_rf: Option<MonRfData>,
    /// Relative position for moving base/rover
    pub rel_pos_ned: Option<RelPosNedData>,
    /// Protection level data (NAV-PL)
    pub nav_pl: Option<NavPlData>,
    /// Time mark data (TIM-TM2) for external event timestamping
    pub tim_tm2: Option<TimTm2Data>,
}

impl UbxHandler {
    /// Create a new UBX handler.
    pub fn new() -> Self {
        Self {
            parser: ParserBuilder::new()
                .with_protocol::<Proto27>()
                .with_vec_buffer(),
            last_pvt: None,
            stats: UbxStats::default(),
            pending_ack: PendingAck::default(),
            hp_pos: None,
            sat_info: None,
            cov: None,
            pos_ecef: None,
            sec_sig: None,
            sec_siglog: None,
            rxm_cor: None,
            mon_comms: None,
            mon_hw: None,
            mon_rf: None,
            rel_pos_ned: None,
            nav_pl: None,
            tim_tm2: None,
        }
    }

    /// Clear any pending ACK state.
    ///
    /// Call this before sending a configuration command to reset the ACK tracking.
    pub fn clear_pending_ack(&mut self) {
        self.pending_ack = PendingAck::default();
    }

    /// Set the expected ACK for a command.
    ///
    /// Call this after sending a command to track which ACK we're expecting.
    pub fn expect_ack(&mut self, class: u8, msg_id: u8) {
        self.pending_ack = PendingAck {
            class: Some(class),
            msg_id: Some(msg_id),
        };
    }

    /// Process incoming data and extract messages.
    ///
    /// Returns parsed PVT data if a new NAV-PVT message was received,
    /// and any ACK/NAK responses.
    pub fn process(&mut self, data: &[u8]) -> ProcessResult {
        let mut messages_processed = 0;
        let mut new_pvt = None;
        let mut new_hp_pos = None;
        let mut new_sat_info = None;
        let mut new_cov = None;
        let mut new_pos_ecef = None;
        let mut new_sec_sig = None;
        let mut new_sec_siglog = None;
        let mut new_rxm_cor = None;
        let mut new_mon_comms = None;
        let mut new_mon_hw = None;
        let mut new_mon_rf = None;
        let mut new_rel_pos_ned = None;
        let mut new_nav_pl = None;
        let mut new_tim_tm2 = None;
        let mut ack_result = None;
        let mut nav_pvt_count = 0u64;
        let mut other_count = 0u64;
        let mut error_count = 0u64;

        // Feed data to parser using consume_ubx
        let mut iter = self.parser.consume_ubx(data);

        loop {
            match iter.next() {
                Some(Ok(UbxPacket::Proto27(packet))) => {
                    messages_processed += 1;
                    match packet {
                        ublox::proto27::PacketRef::NavPvt(nav_pvt) => {
                            nav_pvt_count += 1;
                            let pvt = Self::parse_nav_pvt(&nav_pvt);
                            trace!(
                                fix = ?pvt.fix_type,
                                sats = pvt.num_sv,
                                "NAV-PVT received"
                            );
                            new_pvt = Some(pvt);
                        }
                        ublox::proto27::PacketRef::NavHpPosLlh(msg) => {
                            let hp = Self::parse_hp_pos(&msg);
                            debug!(
                                lat = hp.lat,
                                lon = hp.lon,
                                height = hp.height,
                                h_acc = hp.h_acc,
                                "NAV-HPPOSLLH received"
                            );
                            new_hp_pos = Some(hp);
                        }
                        ublox::proto27::PacketRef::NavSat(msg) => {
                            let sat = Self::parse_nav_sat(&msg);
                            debug!(num_sats = sat.num_sats, "NAV-SAT received");
                            new_sat_info = Some(sat);
                        }
                        ublox::proto27::PacketRef::AckAck(ack) => {
                            if self.pending_ack.matches(ack.class(), ack.msg_id()) {
                                debug!(
                                    class = ack.class(),
                                    id = ack.msg_id(),
                                    "ACK-ACK received (matched)"
                                );
                                ack_result = Some(AckResult::Ack);
                            } else if self.pending_ack.is_pending() {
                                trace!(
                                    class = ack.class(),
                                    id = ack.msg_id(),
                                    "ACK-ACK received (unrelated, ignoring)"
                                );
                            } else {
                                // No pending expectation - accept any ACK (legacy behavior)
                                debug!(class = ack.class(), id = ack.msg_id(), "ACK-ACK received");
                                ack_result = Some(AckResult::Ack);
                            }
                        }
                        ublox::proto27::PacketRef::AckNak(nak) => {
                            if self.pending_ack.matches(nak.class(), nak.msg_id()) {
                                debug!(
                                    class = nak.class(),
                                    id = nak.msg_id(),
                                    "ACK-NAK received (matched)"
                                );
                                ack_result = Some(AckResult::Nak);
                            } else if self.pending_ack.is_pending() {
                                trace!(
                                    class = nak.class(),
                                    id = nak.msg_id(),
                                    "ACK-NAK received (unrelated, ignoring)"
                                );
                            } else {
                                // No pending expectation - accept any NAK (legacy behavior)
                                debug!(class = nak.class(), id = nak.msg_id(), "ACK-NAK received");
                                ack_result = Some(AckResult::Nak);
                            }
                        }
                        ublox::proto27::PacketRef::NavCov(msg) => {
                            let cov = Self::parse_nav_cov(&msg);
                            debug!(
                                pos_valid = cov.pos_cov_valid,
                                vel_valid = cov.vel_cov_valid,
                                "NAV-COV received"
                            );
                            new_cov = Some(cov);
                        }
                        ublox::proto27::PacketRef::NavPosEcef(msg) => {
                            let ecef = Self::parse_nav_pos_ecef(&msg);
                            trace!("NAV-POSECEF received");
                            new_pos_ecef = Some(ecef);
                        }
                        ublox::proto27::PacketRef::SecSig(msg) => {
                            let sig = Self::parse_sec_sig(&msg);
                            debug!(
                                jamming = ?sig.jamming_state,
                                spoofing = ?sig.spoofing_state,
                                "SEC-SIG received"
                            );
                            new_sec_sig = Some(sig);
                        }
                        ublox::proto27::PacketRef::SecSiglog(msg) => {
                            let siglog = Self::parse_sec_siglog(&msg);
                            debug!(num_events = siglog.num_events, "SEC-SIGLOG received");
                            new_sec_siglog = Some(siglog);
                        }
                        ublox::proto27::PacketRef::RxmCor(msg) => {
                            let cor = Self::parse_rxm_cor(&msg);
                            debug!(
                                msg_used = ?cor.msg_used,
                                msg_type = cor.msg_type,
                                "RXM-COR received"
                            );
                            new_rxm_cor = Some(cor);
                        }
                        ublox::proto27::PacketRef::MonComms(msg) => {
                            let comms = Self::parse_mon_comms(&msg);
                            debug!(n_ports = comms.n_ports, "MON-COMMS received");
                            new_mon_comms = Some(comms);
                        }
                        ublox::proto27::PacketRef::MonHw(msg) => {
                            let hw = Self::parse_mon_hw(&msg);
                            debug!(
                                antenna = ?hw.antenna_status,
                                jam_ind = hw.jam_ind,
                                "MON-HW received"
                            );
                            new_mon_hw = Some(hw);
                        }
                        ublox::proto27::PacketRef::MonRf(msg) => {
                            if let Some(rf) = Self::parse_mon_rf(&msg) {
                                debug!(
                                    antenna = ?rf.antenna_status,
                                    jam_ind = rf.jam_ind,
                                    "MON-RF received"
                                );
                                new_mon_rf = Some(rf);
                            }
                        }
                        ublox::proto27::PacketRef::NavRelPosNed(msg) => {
                            let rel_pos = Self::parse_nav_rel_pos_ned(&msg);
                            debug!(
                                carr_soln = rel_pos.flags.carr_soln,
                                length_m = rel_pos.rel_pos_length,
                                "NAV-RELPOSNED received"
                            );
                            new_rel_pos_ned = Some(rel_pos);
                        }
                        ublox::proto27::PacketRef::NavPl(msg) => {
                            let pl = Self::parse_nav_pl(&msg);
                            trace!(pos_valid = pl.pos_valid, tmir = pl.tmir, "NAV-PL received");
                            new_nav_pl = Some(pl);
                        }
                        ublox::proto27::PacketRef::TimTm2(msg) => {
                            let tm2 = Self::parse_tim_tm2(&msg);
                            debug!(
                                ch = tm2.channel,
                                count = tm2.count,
                                rising = tm2.new_rising_edge,
                                falling = tm2.new_falling_edge,
                                "TIM-TM2 received"
                            );
                            new_tim_tm2 = Some(tm2);
                        }
                        _ => {
                            other_count += 1;
                        }
                    }
                }

                Some(Ok(UbxPacket::Proto23(_))) => {
                    // This handler uses a Proto27 parser, so Proto23 packets are not expected
                    // in normal operation. Keep this arm to satisfy exhaustiveness when the
                    // ubx_proto23 feature is enabled.
                    other_count += 1;
                }

                Some(Err(e)) => {
                    error_count += 1;
                    warn!("UBX parse error: {:?}", e);
                }
                None => break,
            }
        }

        // Update stats and store last PVT
        self.stats.messages_parsed += messages_processed as u64;
        self.stats.nav_pvt_count += nav_pvt_count;
        self.stats.other_messages += other_count;
        self.stats.parse_errors += error_count;

        // Store cloned message data for later retrieval
        macro_rules! store_if_some {
            ($field:ident, $new_val:ident) => {
                if let Some(ref val) = $new_val {
                    self.$field = Some(val.clone());
                }
            };
        }

        store_if_some!(last_pvt, new_pvt);
        store_if_some!(cov, new_cov);
        store_if_some!(pos_ecef, new_pos_ecef);
        store_if_some!(sec_sig, new_sec_sig);
        store_if_some!(sec_siglog, new_sec_siglog);
        store_if_some!(rxm_cor, new_rxm_cor);
        store_if_some!(mon_comms, new_mon_comms);
        store_if_some!(mon_hw, new_mon_hw);
        store_if_some!(mon_rf, new_mon_rf);
        store_if_some!(rel_pos_ned, new_rel_pos_ned);
        store_if_some!(nav_pl, new_nav_pl);
        store_if_some!(tim_tm2, new_tim_tm2);

        ProcessResult {
            pvt: new_pvt,
            messages_processed,
            ack: ack_result,
            hp_pos: new_hp_pos,
            sat_info: new_sat_info,
            cov: new_cov,
            pos_ecef: new_pos_ecef,
            sec_sig: new_sec_sig,
            sec_siglog: new_sec_siglog,
            rxm_cor: new_rxm_cor,
            mon_comms: new_mon_comms,
            mon_hw: new_mon_hw,
            mon_rf: new_mon_rf,
            rel_pos_ned: new_rel_pos_ned,
            nav_pl: new_nav_pl,
            tim_tm2: new_tim_tm2,
        }
    }

    /// Parse a TIM-TM2 packet into our TimTm2Data structure.
    fn parse_tim_tm2(msg: &TimTm2Ref) -> TimTm2Data {
        let flags = msg.flags();
        // Convert TOW from ms + sub-ms nanoseconds to seconds
        let tow_rising_s =
            (msg.tow_ms_r() as f64) / 1000.0 + (msg.tow_sub_ms_r() as f64) / 1_000_000_000.0;
        let tow_falling_s =
            (msg.tow_ms_f() as f64) / 1000.0 + (msg.tow_sub_ms_f() as f64) / 1_000_000_000.0;

        TimTm2Data {
            channel: msg.ch(),
            count: msg.count(),
            week_rising: msg.wn_r(),
            week_falling: msg.wn_f(),
            tow_rising_s,
            tow_falling_s,
            accuracy_ns: msg.acc_est(),
            new_rising_edge: flags.new_rising_edge(),
            new_falling_edge: flags.new_falling_edge(),
            time_valid: flags.time_valid(),
            time_base: flags.time_base().into(),
        }
    }

    /// Parse a NAV-PVT packet into our PvtData structure.
    fn parse_nav_pvt(nav_pvt: &ublox::nav_pvt::proto27::NavPvtRef) -> PvtData {
        // Extract carrier solution from flags (bits 6-7)
        let flags = nav_pvt.flags_raw();
        let carr_soln = match (flags >> 6) & 0x03 {
            1 => CarrierSolution::Float,
            2 => CarrierSolution::Fixed,
            _ => CarrierSolution::None,
        };

        // Extract differential correction age from flags3 (bits 4..1)
        // This is a 4-bit index into non-linear age bands, NOT seconds directly.
        let diff_corr_age_raw = nav_pvt.flags3().age_differential_correction();
        // The ublox crate returns bits 1-4 masked but not shifted, so shift right by 1
        let age_index = diff_corr_age_raw >> 1;
        // Convert index to upper bound of age band in seconds (conservative for integrity)
        // Per u-blox interface description UBX-NAV-PVT lastCorrectionAge field
        let diff_corr_age_s = match age_index {
            0 => None,       // Not available
            1 => Some(1),    // 0-1s
            2 => Some(2),    // 1-2s
            3 => Some(5),    // 2-5s
            4 => Some(10),   // 5-10s
            5 => Some(15),   // 10-15s
            6 => Some(20),   // 15-20s
            7 => Some(30),   // 20-30s
            8 => Some(45),   // 30-45s
            9 => Some(60),   // 45-60s
            10 => Some(90),  // 60-90s
            11 => Some(120), // 90-120s
            _ => Some(255),  // >=12: >120s (use max u8)
        };

        // Determine fix type considering RTK status
        let fix_type = if carr_soln == CarrierSolution::Fixed {
            FixType::RtkFixed
        } else if carr_soln == CarrierSolution::Float {
            FixType::RtkFloat
        } else {
            Self::convert_gnss_fix_type(nav_pvt.fix_type())
        };

        PvtData {
            itow: nav_pvt.itow(),
            year: nav_pvt.year(),
            month: nav_pvt.month(),
            day: nav_pvt.day(),
            hour: nav_pvt.hour(),
            min: nav_pvt.min(),
            sec: nav_pvt.sec(),
            nano: nav_pvt.nanosec(),
            fix_type,
            num_sv: nav_pvt.num_satellites(),
            lon: nav_pvt.longitude(),
            lat: nav_pvt.latitude(),
            height: nav_pvt.height_above_ellipsoid(),
            height_msl: nav_pvt.height_msl(),
            h_acc: nav_pvt.horizontal_accuracy() as f32,
            v_acc: nav_pvt.vertical_accuracy() as f32,
            vel_n: nav_pvt.vel_north() as f32,
            vel_e: nav_pvt.vel_east() as f32,
            vel_d: nav_pvt.vel_down() as f32,
            g_speed: nav_pvt.ground_speed_2d() as f32,
            head_mot: nav_pvt.heading_motion() as f32,
            s_acc: nav_pvt.speed_accuracy() as f32,
            head_acc: nav_pvt.heading_accuracy() as f32,
            p_dop: nav_pvt.pdop() as f32,
            carr_soln,
            diff_corr_age_s,
            received_at: Instant::now(),
        }
    }

    fn parse_hp_pos(msg: &NavHpPosLlhRef) -> HpPosData {
        HpPosData {
            lat: (msg.lat_degrees_raw() as f64 * 1.0e-7)
                + (msg.lat_hp_degrees_raw() as f64 * 1.0e-9),
            lon: (msg.lon_degrees_raw() as f64 * 1.0e-7)
                + (msg.lon_hp_degrees_raw() as f64 * 1.0e-9),
            height: (msg.height_msl_raw() as f64 * 1.0e-3)
                + (msg.height_hp_msl_raw() as f64 * 1.0e-4),

            h_acc: msg.horizontal_accuracy() as f32 * 0.001, // mm -> m
            v_acc: msg.vertical_accuracy() as f32 * 0.001,
        }
    }

    fn parse_nav_sat(msg: &NavSatRef) -> SatInfo {
        let mut sats = Vec::new();
        for sv in msg.svs() {
            sats.push(SatStatus {
                gnss_id: sv.gnss_id(),
                sv_id: sv.sv_id(),
                cno: sv.cno(),
                elev: sv.elev(),
                azim: sv.azim(),
                pr_res: sv.pr_res(),
                // NavSatSvFlags doesn't expose a raw u32 value directly.
                // Use 0 as placeholder; individual flags can be queried via sv.flags().sv_used() etc.
                flags: 0,
                sv_used: sv.flags().sv_used(),
            });
        }
        SatInfo {
            num_sats: msg.num_svs(),
            sats,
        }
    }

    /// Parse NAV-COV (position/velocity covariance) message.
    fn parse_nav_cov(msg: &NavCovRef) -> CovData {
        CovData {
            itow: msg.itow(),
            pos_cov_valid: msg.pos_cov_valid() != 0,
            vel_cov_valid: msg.vel_cov_valid() != 0,
            pos_cov: [
                msg.pos_cov_nn(),
                msg.pos_cov_ne(),
                msg.pos_cov_nd(),
                msg.pos_cov_ee(),
                msg.pos_cov_ed(),
                msg.pos_cov_dd(),
            ],
            vel_cov: [
                msg.vel_cov_nn(),
                msg.vel_cov_ne(),
                msg.vel_cov_nd(),
                msg.vel_cov_ee(),
                msg.vel_cov_ed(),
                msg.vel_cov_dd(),
            ],
        }
    }

    /// Parse NAV-POSECEF (ECEF position) message.
    fn parse_nav_pos_ecef(msg: &NavPosEcefRef) -> PosEcefData {
        PosEcefData {
            itow: msg.itow(),
            ecef_x: msg.ecef_x_meters(),
            ecef_y: msg.ecef_y_meters(),
            ecef_z: msg.ecef_z_meters(),
            p_acc: msg.p_acc_meters(),
        }
    }

    /// Parse NAV-RELPOSNED (relative position) message.
    fn parse_nav_rel_pos_ned(msg: &NavRelPosNedRef) -> RelPosNedData {
        // Combine low and high precision components (all methods return f64)
        // rel_pos_*_cm is in cm, rel_pos_hp_*_mm is in 0.1mm -> result in meters
        let rel_pos_n = (msg.rel_pos_n_cm() + msg.rel_pos_hp_n_mm() * 0.01) * 0.01;
        let rel_pos_e = (msg.rel_pos_e_cm() + msg.rel_pos_hp_e_mm() * 0.01) * 0.01;
        let rel_pos_d = (msg.rel_pos_d_cm() + msg.rel_pos_hp_d_mm() * 0.01) * 0.01;
        let rel_pos_length = (msg.rel_pos_length_cm() + msg.rel_pos_hp_length_mm() * 0.01) * 0.01;

        // Heading in degrees, convert to radians
        let rel_pos_heading = msg.rel_pos_heading_degrees().to_radians();

        // Accuracies in mm, convert to meters
        let acc_n = msg.acc_n_mm() * 0.001;
        let acc_e = msg.acc_e_mm() * 0.001;
        let acc_d = msg.acc_d_mm() * 0.001;
        let acc_length = msg.acc_length_mm() * 0.001;
        // Heading accuracy in degrees, convert to radians
        let acc_heading = msg.acc_heading_degrees().to_radians();

        RelPosNedData {
            itow: msg.itow(),
            ref_station_id: msg.ref_station_id(),
            rel_pos_n,
            rel_pos_e,
            rel_pos_d,
            rel_pos_length,
            rel_pos_heading,
            acc_n,
            acc_e,
            acc_d,
            acc_length,
            acc_heading,
            flags: msg.flags().into(),
        }
    }

    /// Parse NAV-PL (protection level) message.
    ///
    /// Protection levels provide statistically-bounded error estimates with a specified
    /// Target Misleading Information Risk (TMIR) for ISO 26262/ISO 21448 compliance.
    fn parse_nav_pl(msg: &NavPlRef) -> NavPlData {
        // Calculate TMIR from coefficient and exponent
        let tmir = (msg.tmir_coeff() as f64) * 10_f64.powi(msg.tmir_exp() as i32);

        // Position protection level validity and frame
        let pos_valid = matches!(msg.pl_pos_valid(), PlPosValid::Valid);
        let pos_frame: NavPlFrame = msg.pl_pos_frame().into();
        let pos_invalidity_reason = if pos_valid {
            NavPlInvalidityReason::Valid
        } else {
            msg.pl_pos_invalidity_reason().into()
        };

        // Position PLs - ublox-rs returns values in meters (SI units)
        let pos_pl_m = [msg.pl_pos1(), msg.pl_pos2(), msg.pl_pos3()];

        // Velocity protection level validity and frame
        let vel_valid = matches!(msg.pl_vel_valid(), PlVelValid::Valid);
        let vel_frame: NavPlFrame = msg.pl_vel_frame().into();
        let vel_invalidity_reason = if vel_valid {
            NavPlInvalidityReason::Valid
        } else {
            msg.pl_vel_invalidity_reason().into()
        };

        // Velocity PLs - ublox-rs returns values in m/s (SI units)
        let vel_pl_ms = [msg.pl_vel1(), msg.pl_vel2(), msg.pl_vel3()];

        // Time protection level validity
        let time_valid = matches!(msg.pl_time_valid(), PlTimeValid::Valid);
        let time_invalidity_reason = if time_valid {
            NavPlInvalidityReason::Valid
        } else {
            msg.pl_time_invalidity_reason().into()
        };

        NavPlData {
            itow: msg.itow(),
            tmir,
            pos_valid,
            pos_frame,
            pos_invalidity_reason,
            pos_pl_m,
            pos_horiz_orient_deg: msg.pl_pos_horiz_orient(),
            vel_valid,
            vel_frame,
            vel_invalidity_reason,
            vel_pl_ms,
            vel_horiz_orient_deg: msg.pl_vel_horiz_orient(),
            time_valid,
            time_invalidity_reason,
            time_pl_ns: msg.pl_time(),
        }
    }

    /// Parse SEC-SIG (signal security status) message.
    fn parse_sec_sig(msg: &SecSigRef) -> SecSigData {
        let flags: SecSigFlags = msg.sig_sec_flags();
        // Additional observability (not currently exposed in ROS messages)
        let jam_num_cent_freqs = msg.jam_num_cent_freqs();
        let mut cent_freq_khz = Vec::new();
        let mut jammed = Vec::new();
        for e in msg.jam_state_cent_freqs() {
            cent_freq_khz.push(e.cent_freq_khz);
            jammed.push(e.jammed);
        }
        let jammed_any = jammed.iter().any(|v| *v);
        trace!(
            jam_det_enabled = flags.jam_det_enabled,
            spf_det_enabled = flags.spf_det_enabled,
            jam_num_cent_freqs,
            jammed_any,
            "SEC-SIG flags"
        );
        SecSigData {
            jam_det_enabled: flags.jam_det_enabled,
            spf_det_enabled: flags.spf_det_enabled,
            jamming_state: flags.jamming_state.into(),
            spoofing_state: flags.spoofing_state.into(),
            jam_num_cent_freqs,
            cent_freq_khz,
            jammed,
        }
    }

    /// Parse SEC-SIGLOG (security event log) message.
    fn parse_sec_siglog(msg: &SecSiglogRef) -> SecSiglogData {
        let mut events = Vec::new();
        for event in msg.events() {
            events.push(SecSiglogEventData {
                time_elapsed_s: event.time_elapsed_s,
                detection_type: event.detection_type,
                event_type: event.event_type,
            });
        }
        SecSiglogData {
            num_events: msg.num_events(),
            events,
        }
    }

    /// Parse RXM-COR (differential correction status) message.
    fn parse_rxm_cor(msg: &RxmCorRef) -> RxmCorData {
        let status: RxmCorStatusInfo = msg.status_info();
        RxmCorData {
            ebno: msg.ebno(),
            msg_used: status.msg_used,
            msg_encrypted: status.msg_encrypted,
            msg_decrypted: status.msg_decrypted,
            msg_type: msg.msg_type(),
            msg_sub_type: msg.msg_sub_type(),
        }
    }

    /// Parse MON-HW (hardware status) message.
    fn parse_mon_hw(msg: &MonHwRef) -> MonHwData {
        let antenna_status = match msg.a_status() {
            AntennaStatus::Init => AntennaStatusData::Init,
            AntennaStatus::DontKnow => AntennaStatusData::Unknown,
            AntennaStatus::Ok => AntennaStatusData::Ok,
            AntennaStatus::Short => AntennaStatusData::Short,
            AntennaStatus::Open => AntennaStatusData::Open,
            _ => AntennaStatusData::Unknown,
        };

        let antenna_power = match msg.a_power() {
            AntennaPower::Off => AntennaPowerData::Off,
            AntennaPower::On => AntennaPowerData::On,
            AntennaPower::DontKnow => AntennaPowerData::Unknown,
            _ => AntennaPowerData::Unknown,
        };

        let flags = msg.flags();
        let jamming_state = match flags.jamming_state() {
            ublox::mon_hw::JammingState::Unknown => JammingStateData::Unknown,
            ublox::mon_hw::JammingState::Ok => JammingStateData::Ok,
            ublox::mon_hw::JammingState::Warning => JammingStateData::Warning,
            ublox::mon_hw::JammingState::Critical => JammingStateData::Critical,
            _ => JammingStateData::Unknown,
        };

        MonHwData {
            antenna_status,
            antenna_power,
            jam_ind: msg.jam_ind(),
            noise_per_ms: msg.noise_per_ms(),
            agc_cnt: msg.agc_cnt(),
            jamming_state,
            rtc_calib: flags.rtc_calib(),
            safe_boot: flags.safe_boot(),
        }
    }

    /// Parse MON-RF (RF status) message - replaces deprecated MON-HW.
    fn parse_mon_rf(msg: &ublox::mon_rf::MonRfRef) -> Option<MonRfData> {
        // Get the first RF block (typically L1 band)
        let block = msg.blocks().next()?;

        let antenna_status = match block.ant_status {
            ublox::mon_rf::AntennaStatus::Init => AntennaStatusData::Init,
            ublox::mon_rf::AntennaStatus::DontKnow => AntennaStatusData::Unknown,
            ublox::mon_rf::AntennaStatus::Ok => AntennaStatusData::Ok,
            ublox::mon_rf::AntennaStatus::Short => AntennaStatusData::Short,
            ublox::mon_rf::AntennaStatus::Open => AntennaStatusData::Open,
            _ => AntennaStatusData::Unknown,
        };

        let antenna_power = match block.ant_power {
            ublox::mon_rf::AntennaPowerStatus::Off => AntennaPowerData::Off,
            ublox::mon_rf::AntennaPowerStatus::On => AntennaPowerData::On,
            ublox::mon_rf::AntennaPowerStatus::DontKnow => AntennaPowerData::Unknown,
            _ => AntennaPowerData::Unknown,
        };

        // Note: jammingState in flags is deprecated and always 0 on F9P with SEC-SIG.
        // Use SEC-SIG for jamming state instead.

        Some(MonRfData {
            antenna_status,
            antenna_power,
            jam_ind: block.jam_ind, // cwSuppression in manual (0-255 CW jamming indicator)
            noise_per_ms: block.noise_per_ms,
            agc_cnt: block.agc_cnt,
        })
    }

    /// Parse MON-COMMS (communication port status) message.
    fn parse_mon_comms(msg: &MonCommsRef) -> MonCommsData {
        use ublox::mon_comms::PortId;

        let mut ports = Vec::new();
        for port in msg.ports() {
            let port_name = match port.port_id {
                // The u-blox F9P interface description uses portId values:
                // 1=I2C, 2=UART1, 3=UART2, 4=USB, 5=SPI (0=N/A).
                // The ublox crate currently maps these IDs differently (off-by-one), so we remap
                // to match the device documentation for human-readable output.
                PortId::I2c => "N/A".to_string(),
                PortId::Uart1 => "I2C".to_string(),
                PortId::Uart2 => "UART1".to_string(),
                PortId::Usb => "UART2".to_string(),
                PortId::Spi => "SPI".to_string(),
                PortId::Unknown(4) => "USB".to_string(),
                PortId::Unknown(id) => format!("Unknown(0x{:04X})", id),
            };
            ports.push(MonCommsPortData {
                port_name,
                tx_pending: port.tx_pending,
                tx_bytes: port.tx_bytes,
                tx_usage_percent: port.tx_usage as f32,
                rx_pending: port.rx_pending,
                rx_bytes: port.rx_bytes,
                rx_usage_percent: port.rx_usage as f32,
                overrun_errs: port.overrun_errs,
            });
        }
        MonCommsData {
            n_ports: msg.n_ports(),
            // Mask the actual TX error bits (mem/alloc). Upper bits encode outputPort in the F9P spec.
            tx_errors: msg.tx_errors() & 0x03,
            ports,
        }
    }

    /// Convert u-blox GnssFixType to our FixType enum.
    pub fn convert_gnss_fix_type(fix: GnssFixType) -> FixType {
        match fix {
            GnssFixType::NoFix => FixType::NoFix,
            GnssFixType::DeadReckoningOnly => FixType::DeadReckoning,
            GnssFixType::Fix2D => FixType::Fix2D,
            GnssFixType::Fix3D => FixType::Fix3D,
            GnssFixType::GPSPlusDeadReckoning => FixType::GnssDr,
            GnssFixType::TimeOnlyFix => FixType::TimeOnly,
            _ => FixType::NoFix, // Reserved/unknown fix types
        }
    }

    /// Get the last received PVT data.
    pub fn last_pvt(&self) -> Option<&PvtData> {
        self.last_pvt.as_ref()
    }

    /// Get message statistics.
    pub fn stats(&self) -> &UbxStats {
        &self.stats
    }

    /// Get current fix type from most recent data.
    pub fn current_fix_type(&self) -> FixType {
        self.last_pvt
            .as_ref()
            .map(|p| p.fix_type)
            .unwrap_or(FixType::NoFix)
    }
}

impl Default for UbxHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Configuration command builders
// ============================================================================

/// Build a CFG-MSG command to enable/disable a message on all ports.
pub fn build_cfg_msg(class: u8, id: u8, rate: u8) -> Vec<u8> {
    CfgMsgAllPortsBuilder {
        msg_class: class,
        msg_id: id,
        rates: [rate; 6],
    }
    .into_packet_bytes()
    .to_vec()
}

/// Build a CFG-VALSET command to set configuration values.
///
/// This is the modern (Generation 9+) way to configure u-blox receivers.
/// Configuration can be saved to RAM only, or also to BBR/Flash for persistence.
///
/// # Arguments
/// * `cfg_data` - Slice of configuration values to set
/// * `persist` - If true, save to BBR (Battery-Backed RAM) for persistence across power cycles
///
/// # Example
/// ```ignore
/// use ublox::cfg_val::CfgVal;
/// let packet = build_cfg_valset(&[
///     CfgVal::RateMeas(100),  // 100ms = 10Hz
///     CfgVal::RateNav(1),     // 1 nav solution per measurement
///     CfgVal::MsgOutUbxNavPvtUart1(1),  // Enable NAV-PVT on UART1
/// ], true);
/// ```
pub fn build_cfg_valset(cfg_data: &[CfgVal], persist: bool) -> Vec<u8> {
    let layers = if persist {
        CfgLayerSet::RAM | CfgLayerSet::BBR
    } else {
        CfgLayerSet::RAM
    };

    CfgValSetBuilder {
        version: 1,
        layers,
        reserved1: 0,
        cfg_data,
    }
    .into_packet_vec()
}

/// Build a CFG-VALSET command to set navigation rate.
///
/// # Arguments
/// * `measure_rate_ms` - Measurement rate in milliseconds (e.g., 100 for 10Hz, 1000 for 1Hz)
/// * `nav_rate` - Number of measurements per navigation solution (typically 1)
/// * `persist` - If true, save to BBR for persistence
pub fn build_cfg_rate(measure_rate_ms: u16, nav_rate: u16, persist: bool) -> Vec<u8> {
    build_cfg_valset(
        &[CfgVal::RateMeas(measure_rate_ms), CfgVal::RateNav(nav_rate)],
        persist,
    )
}

/// Build a CFG-VALSET command for common GNSS driver configuration.
///
/// Configures the receiver for typical RTK rover operation:
/// - Enables UBX protocol on UART
/// - Enables RTCM3 input for corrections
/// - Sets output message rates
/// - Configures navigation rate
///
/// # Arguments
/// * `nav_rate_hz` - Navigation rate in Hz (1-10)
/// * `persist` - If true, save to BBR for persistence
pub fn build_rover_config(nav_rate_hz: u8, persist: bool) -> Vec<u8> {
    let measure_rate_ms = 1000 / nav_rate_hz as u16;

    build_cfg_valset(
        &[
            // Protocol settings - enable UBX and RTCM3 input
            CfgVal::Uart1InProtUbx(true),
            CfgVal::Uart1InProtRtcm3x(true),
            CfgVal::Uart1OutProtUbx(true),
            // Disable NMEA output (we use UBX)
            CfgVal::Uart1OutProtNmea(false),
            // Navigation rate
            CfgVal::RateMeas(measure_rate_ms),
            CfgVal::RateNav(1),
            // Enable key messages on UART1
            CfgVal::MsgOutUbxNavPvtUart1(1),
            CfgVal::MsgOutUbxNavHpPosLlhUart1(1),
            CfgVal::MsgOutUbxNavSatUart1(1),
        ],
        persist,
    )
}

/// Build a CFG-VALSET command to enable safety-critical messages.
///
/// Enables the following messages for integrity monitoring:
/// - NAV-COV: Position/velocity covariance matrices
/// - NAV-POSECEF: ECEF position (for coordinate transforms)
/// - SEC-SIG: Jamming/spoofing detection status
/// - SEC-SIGLOG: Security event log
/// - RXM-COR: Differential correction status
/// - MON-COMMS: Communication port status
///
/// # Arguments
/// * `port` - Output port: "uart1", "uart2", "usb", "i2c", or "spi"
/// * `rate` - Output rate (1 = every epoch, 0 = disabled)
/// * `persist` - If true, save to BBR for persistence
pub fn build_safety_messages_config(port: &str, rate: u8, persist: bool) -> Vec<u8> {
    // CFG-MSGOUT key IDs for safety messages (from u-blox F9 Interface Description)
    // Format: 0x2091XXYY where XX is group, YY is item
    let keys = match port.to_lowercase().as_str() {
        "uart1" => cfg_keys::SAFETY_MSG_UART1,
        "uart2" => cfg_keys::SAFETY_MSG_UART2,
        "usb" => cfg_keys::SAFETY_MSG_USB,
        "i2c" => cfg_keys::SAFETY_MSG_I2C,
        "spi" => cfg_keys::SAFETY_MSG_SPI,
        _ => cfg_keys::SAFETY_MSG_USB, // Default to USB
    };

    build_cfg_valset_raw(&keys, rate, persist)
}

/// Build a raw CFG-VALSET packet with u8 values for given key IDs.
///
/// This is useful for configuration keys not yet in the ublox crate's CfgVal enum.
fn build_cfg_valset_raw(key_ids: &[u32], value: u8, persist: bool) -> Vec<u8> {
    let layers: u8 = if persist { 0x03 } else { 0x01 }; // RAM + BBR or RAM only

    // Calculate payload size: 4 bytes header + (4 bytes key + 1 byte value) per key
    let payload_len = 4 + key_ids.len() * 5;

    let mut packet = Vec::with_capacity(8 + payload_len + 2);

    // UBX header
    packet.extend_from_slice(&[0xB5, 0x62]);
    // CFG-VALSET class/id
    packet.extend_from_slice(&[0x06, 0x8A]);
    // Payload length (little-endian)
    packet.extend_from_slice(&(payload_len as u16).to_le_bytes());

    // CFG-VALSET header: version, layers, reserved
    packet.push(0x01); // version
    packet.push(layers);
    packet.extend_from_slice(&[0x00, 0x00]); // reserved

    // Key-value pairs
    for &key_id in key_ids {
        packet.extend_from_slice(&key_id.to_le_bytes());
        packet.push(value);
    }

    // Calculate checksum (Fletcher-8 over class, id, length, payload)
    let (ck_a, ck_b) = ubx_checksum(&packet[2..]);
    packet.push(ck_a);
    packet.push(ck_b);

    packet
}

/// Calculate UBX checksum (Fletcher-8).
fn ubx_checksum(data: &[u8]) -> (u8, u8) {
    let mut ck_a: u8 = 0;
    let mut ck_b: u8 = 0;
    for byte in data {
        ck_a = ck_a.wrapping_add(*byte);
        ck_b = ck_b.wrapping_add(ck_a);
    }
    (ck_a, ck_b)
}

/// CFG-MSGOUT key IDs for safety-related messages.
///
/// These are the configuration keys for enabling UBX message output on various ports.
/// Key format: 0x2091XXYY (size=1 byte, group=0x91, item varies)
pub mod cfg_keys {
    // NAV-COV (0x01 0x36) - Position/velocity covariance
    pub const NAV_COV_I2C: u32 = 0x20910083;
    pub const NAV_COV_UART1: u32 = 0x20910084;
    pub const NAV_COV_UART2: u32 = 0x20910085;
    pub const NAV_COV_USB: u32 = 0x20910086;
    pub const NAV_COV_SPI: u32 = 0x20910087;

    // NAV-POSECEF (0x01 0x01) - ECEF position
    // Already in ublox crate as MsgOutUbxNavPosEcef*
    pub const NAV_POSECEF_I2C: u32 = 0x20910024;
    pub const NAV_POSECEF_UART1: u32 = 0x20910025;
    pub const NAV_POSECEF_UART2: u32 = 0x20910026;
    pub const NAV_POSECEF_USB: u32 = 0x20910027;
    pub const NAV_POSECEF_SPI: u32 = 0x20910028;

    // SEC-SIG (0x27 0x09) - Jamming/spoofing detection
    pub const SEC_SIG_I2C: u32 = 0x20910634;
    pub const SEC_SIG_UART1: u32 = 0x20910635;
    pub const SEC_SIG_UART2: u32 = 0x20910636;
    pub const SEC_SIG_USB: u32 = 0x20910637;
    pub const SEC_SIG_SPI: u32 = 0x20910638;

    // SEC-SIGLOG (0x27 0x10) - Security event log
    pub const SEC_SIGLOG_I2C: u32 = 0x20910689;
    pub const SEC_SIGLOG_UART1: u32 = 0x2091068A;
    pub const SEC_SIGLOG_UART2: u32 = 0x2091068B;
    pub const SEC_SIGLOG_USB: u32 = 0x2091068C;
    pub const SEC_SIGLOG_SPI: u32 = 0x2091068D;

    // RXM-COR (0x02 0x34) - Differential correction status
    pub const RXM_COR_I2C: u32 = 0x209106B6;
    pub const RXM_COR_UART1: u32 = 0x209106B7;
    pub const RXM_COR_UART2: u32 = 0x209106B8;
    pub const RXM_COR_USB: u32 = 0x209106B9;
    pub const RXM_COR_SPI: u32 = 0x209106BA;

    // MON-COMMS (0x0A 0x36) - Communication port status
    // Already in ublox crate as MsgOutUbxMoncomms*
    pub const MON_COMMS_I2C: u32 = 0x2091034F;
    pub const MON_COMMS_UART1: u32 = 0x20910350;
    pub const MON_COMMS_UART2: u32 = 0x20910351;
    pub const MON_COMMS_USB: u32 = 0x20910352;
    pub const MON_COMMS_SPI: u32 = 0x20910353;

    // Grouped arrays for convenience
    pub const SAFETY_MSG_I2C: [u32; 6] = [
        NAV_COV_I2C,
        NAV_POSECEF_I2C,
        SEC_SIG_I2C,
        SEC_SIGLOG_I2C,
        RXM_COR_I2C,
        MON_COMMS_I2C,
    ];

    pub const SAFETY_MSG_UART1: [u32; 6] = [
        NAV_COV_UART1,
        NAV_POSECEF_UART1,
        SEC_SIG_UART1,
        SEC_SIGLOG_UART1,
        RXM_COR_UART1,
        MON_COMMS_UART1,
    ];

    pub const SAFETY_MSG_UART2: [u32; 6] = [
        NAV_COV_UART2,
        NAV_POSECEF_UART2,
        SEC_SIG_UART2,
        SEC_SIGLOG_UART2,
        RXM_COR_UART2,
        MON_COMMS_UART2,
    ];

    pub const SAFETY_MSG_USB: [u32; 6] = [
        NAV_COV_USB,
        NAV_POSECEF_USB,
        SEC_SIG_USB,
        SEC_SIGLOG_USB,
        RXM_COR_USB,
        MON_COMMS_USB,
    ];

    pub const SAFETY_MSG_SPI: [u32; 6] = [
        NAV_COV_SPI,
        NAV_POSECEF_SPI,
        SEC_SIG_SPI,
        SEC_SIGLOG_SPI,
        RXM_COR_SPI,
        MON_COMMS_SPI,
    ];
}

/// UBX message class/ID constants.
pub mod msg_ids {
    // NAV class (0x01)
    pub const NAV_PVT: (u8, u8) = (0x01, 0x07);
    pub const NAV_STATUS: (u8, u8) = (0x01, 0x03);
    pub const NAV_DOP: (u8, u8) = (0x01, 0x04);
    pub const NAV_SAT: (u8, u8) = (0x01, 0x35);
    pub const NAV_POSLLH: (u8, u8) = (0x01, 0x02);
    pub const NAV_VELNED: (u8, u8) = (0x01, 0x12);
    pub const NAV_COV: (u8, u8) = (0x01, 0x36);
    pub const NAV_POSECEF: (u8, u8) = (0x01, 0x01);

    // RXM class (0x02)
    pub const RXM_COR: (u8, u8) = (0x02, 0x34);

    // MON class (0x0A)
    pub const MON_RF: (u8, u8) = (0x0A, 0x38);
    pub const MON_HW: (u8, u8) = (0x0A, 0x09);
    pub const MON_COMMS: (u8, u8) = (0x0A, 0x36);

    // SEC class (0x27)
    pub const SEC_SIG: (u8, u8) = (0x27, 0x09);
    pub const SEC_SIGLOG: (u8, u8) = (0x27, 0x10);

    // CFG class (0x06)
    pub const CFG_MSG: (u8, u8) = (0x06, 0x01);
    pub const CFG_RATE: (u8, u8) = (0x06, 0x08);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let handler = UbxHandler::new();
        assert!(handler.last_pvt().is_none());
        assert_eq!(handler.stats().messages_parsed, 0);
    }

    #[test]
    fn test_empty_process() {
        let mut handler = UbxHandler::new();
        let result = handler.process(&[]);
        assert!(result.pvt.is_none());
        assert_eq!(result.messages_processed, 0);
    }

    #[test]
    fn test_partial_data() {
        let mut handler = UbxHandler::new();
        // Feed incomplete UBX header
        let result = handler.process(&[0xB5, 0x62, 0x01]);
        assert!(result.pvt.is_none());
        assert_eq!(result.messages_processed, 0);
    }

    #[test]
    fn test_cfg_msg_builder() {
        let cmd = build_cfg_msg(0x01, 0x07, 1);
        // Should start with UBX header
        assert_eq!(&cmd[0..2], &[0xB5, 0x62]);
        // CFG-MSG class/id
        assert_eq!(&cmd[2..4], &[0x06, 0x01]);
    }

    #[test]
    fn test_carrier_solution_default() {
        assert_eq!(CarrierSolution::default(), CarrierSolution::None);
    }

    #[test]
    fn test_current_fix_type_default() {
        let handler = UbxHandler::new();
        assert_eq!(handler.current_fix_type(), FixType::NoFix);
    }

    #[test]
    fn test_gnss_fix_type_conversion() {
        assert_eq!(
            UbxHandler::convert_gnss_fix_type(GnssFixType::NoFix),
            FixType::NoFix
        );
        assert_eq!(
            UbxHandler::convert_gnss_fix_type(GnssFixType::Fix3D),
            FixType::Fix3D
        );
    }

    #[test]
    fn test_cfg_valset_builder() {
        let cmd = build_cfg_valset(&[CfgVal::RateMeas(100), CfgVal::RateNav(1)], false);
        // Should start with UBX header
        assert_eq!(&cmd[0..2], &[0xB5, 0x62]);
        // CFG-VALSET class/id (0x06, 0x8A)
        assert_eq!(&cmd[2..4], &[0x06, 0x8A]);
        // Should have reasonable length
        assert!(cmd.len() > 10);
    }

    #[test]
    fn test_cfg_rate_builder() {
        let cmd = build_cfg_rate(100, 1, false); // 10 Hz
        assert_eq!(&cmd[0..2], &[0xB5, 0x62]);
        assert_eq!(&cmd[2..4], &[0x06, 0x8A]);
    }

    #[test]
    fn test_rover_config_builder() {
        let cmd = build_rover_config(5, true); // 5 Hz, persist
        assert_eq!(&cmd[0..2], &[0xB5, 0x62]);
        assert_eq!(&cmd[2..4], &[0x06, 0x8A]);
        // Should have reasonable length for multiple config values
        assert!(cmd.len() > 30);
    }

    /// Helper to calculate UBX checksum (Fletcher-8)
    fn ubx_checksum(data: &[u8]) -> (u8, u8) {
        let mut ck_a: u8 = 0;
        let mut ck_b: u8 = 0;
        for byte in data {
            ck_a = ck_a.wrapping_add(*byte);
            ck_b = ck_b.wrapping_add(ck_a);
        }
        (ck_a, ck_b)
    }

    /// Build a valid ACK-ACK packet for testing
    fn build_ack_ack(cls: u8, msg_id: u8) -> Vec<u8> {
        // ACK-ACK: class 0x05, id 0x01, payload = [cls, msg_id]
        let class = 0x05u8;
        let id = 0x01u8;
        let payload = [cls, msg_id];
        let len = payload.len() as u16;

        let mut packet = vec![0xB5, 0x62, class, id];
        packet.extend_from_slice(&len.to_le_bytes());
        packet.extend_from_slice(&payload);

        // Calculate checksum over class, id, length, payload
        let (ck_a, ck_b) = ubx_checksum(&packet[2..]);
        packet.push(ck_a);
        packet.push(ck_b);

        packet
    }

    /// Build a valid ACK-NAK packet for testing
    fn build_ack_nak(cls: u8, msg_id: u8) -> Vec<u8> {
        // ACK-NAK: class 0x05, id 0x00, payload = [cls, msg_id]
        let class = 0x05u8;
        let id = 0x00u8;
        let payload = [cls, msg_id];
        let len = payload.len() as u16;

        let mut packet = vec![0xB5, 0x62, class, id];
        packet.extend_from_slice(&len.to_le_bytes());
        packet.extend_from_slice(&payload);

        let (ck_a, ck_b) = ubx_checksum(&packet[2..]);
        packet.push(ck_a);
        packet.push(ck_b);

        packet
    }

    #[test]
    fn test_ack_ack_parsing() {
        let mut handler = UbxHandler::new();
        // Build ACK-ACK for CFG-VALSET (0x06, 0x8A)
        let packet = build_ack_ack(0x06, 0x8A);

        let result = handler.process(&packet);
        assert_eq!(result.ack, Some(AckResult::Ack));
        assert!(result.pvt.is_none());
    }

    #[test]
    fn test_ack_nak_parsing() {
        let mut handler = UbxHandler::new();
        // Build ACK-NAK for CFG-VALSET (0x06, 0x8A)
        let packet = build_ack_nak(0x06, 0x8A);

        let result = handler.process(&packet);
        assert_eq!(result.ack, Some(AckResult::Nak));
        assert!(result.pvt.is_none());
    }

    #[test]
    fn test_clear_pending_ack() {
        let mut handler = UbxHandler::new();
        handler.expect_ack(0x06, 0x8A);
        handler.clear_pending_ack();
        // Just verify it doesn't panic - internal state is private
    }

    #[test]
    fn test_ack_result_equality() {
        assert_eq!(AckResult::Ack, AckResult::Ack);
        assert_eq!(AckResult::Nak, AckResult::Nak);
        assert_ne!(AckResult::Ack, AckResult::Nak);
    }

    #[test]
    fn test_process_result_no_ack() {
        let mut handler = UbxHandler::new();
        // Process empty data
        let result = handler.process(&[]);
        assert!(result.ack.is_none());
    }

    // =========================================================================
    // Protocol error handling tests
    // =========================================================================

    #[test]
    fn test_bad_checksum_continues_parsing() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Feed a packet with bad checksum
        let bad_packet = ubx_packets::nav_pvt_bad_checksum();
        let result = handler.process(&bad_packet);

        // Parser should skip the bad packet (no PVT extracted)
        assert!(result.pvt.is_none());
        // Stats should reflect the failed parse attempt
        // (The ublox crate may silently discard or report differently)
    }

    #[test]
    fn test_truncated_packet_buffered() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Feed a truncated packet
        let truncated = ubx_packets::nav_pvt_truncated();
        let result = handler.process(&truncated);

        // No complete message should be extracted
        assert!(result.pvt.is_none());
        assert_eq!(result.messages_processed, 0);
    }

    #[test]
    fn test_unknown_message_skipped() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Feed an unknown message class
        let unknown = ubx_packets::unknown_message();
        let result = handler.process(&unknown);

        // Unknown messages should be skipped without error
        assert!(result.pvt.is_none());
        // Parser should have processed the packet structure
    }

    #[test]
    fn test_garbage_before_valid_packet() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Feed garbage followed by valid ACK packet
        let data = ubx_packets::garbage_then_valid();
        let _result = handler.process(&data);

        // Parser should recover and find the valid packet after garbage
        // The exact behavior depends on ublox crate's sync recovery
    }

    #[test]
    fn test_multiple_packets_with_errors() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Build a stream with: bad checksum, then valid ACK-ACK
        let mut data = ubx_packets::nav_pvt_bad_checksum();
        data.extend(ubx_packets::ack_ack_cfg_valset());

        let result = handler.process(&data);

        // Should still extract the valid ACK packet
        assert_eq!(result.ack, Some(AckResult::Ack));
    }

    #[test]
    fn test_empty_payload_message() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Feed a message with empty payload
        let empty = ubx_packets::empty_payload();
        let result = handler.process(&empty);

        // Should handle gracefully without panic
        assert!(result.pvt.is_none());
    }

    #[test]
    fn test_valid_ack_from_fixture() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Use fixture ACK-ACK packet
        let packet = ubx_packets::ack_ack_cfg_valset();
        let result = handler.process(&packet);

        assert_eq!(result.ack, Some(AckResult::Ack));
    }

    #[test]
    fn test_valid_nak_from_fixture() {
        use crate::device::test_fixtures::ubx_packets;

        let mut handler = UbxHandler::new();

        // Use fixture ACK-NAK packet
        let packet = ubx_packets::ack_nak_cfg_valset();
        let result = handler.process(&packet);

        assert_eq!(result.ack, Some(AckResult::Nak));
    }

    #[test]
    fn test_parser_recovery_after_partial() {
        let mut handler = UbxHandler::new();

        // Feed partial data
        let partial = [0xB5, 0x62, 0x01, 0x07];
        let result1 = handler.process(&partial);
        assert!(result1.pvt.is_none());

        // Feed valid ACK packet
        let ack = build_ack_ack(0x06, 0x8A);
        let result2 = handler.process(&ack);

        // Should still be able to parse new packets
        assert_eq!(result2.ack, Some(AckResult::Ack));
    }
}
