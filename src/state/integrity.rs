//! GNSS integrity monitoring and aggregation.
//!
//! This module implements safety-critical integrity monitoring as described in
//! `docs/INTEGRITY.md`. It aggregates quality metrics from multiple UBX messages
//! to determine overall GNSS solution integrity.

use std::time::Instant;

use crate::device::{
    AntennaStatusData, CovData, JammingStateData, MonCommsData, MonHwData, MonRfData, NavPlData,
    NavPlFrame, NavPlInvalidityReason, PosEcefData, RxmCorData, SecSigData, SecSiglogData,
    SignalQuality, SpoofingStateData,
};
use crate::state::FixType;
use ublox::rxm_cor::CorrectionMsgUsed;

/// Overall integrity level for the GNSS solution.
///
/// Based on Section 5.3 of the reference document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum IntegrityLevel {
    /// All checks pass, full operation permitted
    Ok = 0,
    /// Some checks failed, degraded operation (reduced speed/capability)
    Degraded = 1,
    /// Critical checks failed, operation should stop
    Critical = 2,
    /// System failed or data unavailable
    #[default]
    Failed = 3,
}

impl std::fmt::Display for IntegrityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Degraded => write!(f, "DEGRADED"),
            Self::Critical => write!(f, "CRITICAL"),
            Self::Failed => write!(f, "FAILED"),
        }
    }
}

/// Antenna status from hardware monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AntennaStatus {
    #[default]
    Unknown = 0,
    Ok = 1,
    Open = 2,
    Short = 3,
}

/// Configurable thresholds for integrity checks.
#[derive(Debug, Clone)]
pub struct IntegrityThresholds {
    /// Minimum satellites for Level 0 (critical)
    pub min_satellites_critical: u8,
    /// Minimum satellites for Level 1 (high quality)
    pub min_satellites_high: u8,
    /// Maximum horizontal accuracy (m) for Level 1
    pub max_h_accuracy_m: f32,
    /// Maximum vertical accuracy (m) for Level 1
    pub max_v_accuracy_m: f32,
    /// Maximum PDOP for Level 1
    pub max_pdop: f32,
    /// Maximum correction age (s) for RTK applications (device-reported)
    pub max_correction_age_s: f32,
    /// Maximum time (seconds) without PVT before declaring integrity FAILED
    pub max_pvt_age_s: f32,
    /// IntegrityLevel at which operational becomes false
    /// 0 = only Ok is operational (strictest)
    /// 1 = Ok or Degraded is operational (default)
    /// 2 = Ok/Degraded/Critical is operational (permissive)
    pub operational_threshold: u8,
    /// Minimum C/N0 (dB-Hz) for weakest satellite before DEGRADED
    pub min_cno_degraded: u8,
    /// Minimum mean C/N0 (dB-Hz) across used satellites before DEGRADED
    pub min_mean_cno_degraded: f32,

    // Protection Level (NAV-PL) thresholds
    /// Maximum horizontal protection level (m) before DEGRADED
    pub max_horizontal_pl_m: f32,
    /// Maximum vertical protection level (m) before DEGRADED
    pub max_vertical_pl_m: f32,
    /// Maximum velocity protection level (m/s) before DEGRADED
    pub max_velocity_pl_ms: f32,
    /// Maximum TMIR (Target Misleading Information Risk) per epoch
    pub max_tmir_per_epoch: f64,
    /// Require valid protection level for operational status
    /// If true, invalid PL will result in CRITICAL level
    pub require_valid_pl: bool,
}

impl Default for IntegrityThresholds {
    fn default() -> Self {
        Self {
            min_satellites_critical: 4,
            min_satellites_high: 6,
            max_h_accuracy_m: 0.1,  // 10cm
            max_v_accuracy_m: 0.15, // 15cm
            max_pdop: 3.0,
            max_correction_age_s: 10.0,
            max_pvt_age_s: 2.0,          // 2 seconds without data = Failed
            operational_threshold: 1,    // Ok or Degraded = operational
            min_cno_degraded: 25,        // Weakest satellite < 25 dB-Hz = Degraded
            min_mean_cno_degraded: 35.0, // Mean C/N0 < 35 dB-Hz = Degraded
            // Protection Level thresholds
            max_horizontal_pl_m: 0.50, // 50cm horizontal PL alert limit
            max_vertical_pl_m: 1.00,   // 1m vertical PL alert limit
            max_velocity_pl_ms: 0.10,  // 10cm/s velocity PL alert limit
            max_tmir_per_epoch: 6.0,   // 6 %MI/epoch (device default is typically 5e0 = 5%)
            require_valid_pl: false,   // Don't require PL by default (backward compat)
        }
    }
}

/// Aggregated GNSS integrity data.
///
/// This structure combines all quality metrics from various UBX messages
/// into a single integrity assessment, as proposed in Section 6.4.
#[derive(Debug, Clone)]
pub struct GnssIntegrity {
    /// Overall integrity level
    pub level: IntegrityLevel,
    /// Human-readable status message
    pub status_message: String,

    // Individual check results (true = passed)
    // These enable pass/fail grid visualization for diagnosing integrity state changes
    /// Fix type is acceptable (not NoFix/2D/DR/TimeOnly)
    pub check_fix_type_ok: bool,
    /// Satellite count meets threshold
    pub check_satellites_ok: bool,
    /// Horizontal accuracy within limit
    pub check_h_accuracy_ok: bool,
    /// Vertical accuracy within limit
    pub check_v_accuracy_ok: bool,
    /// PDOP within limit
    pub check_pdop_ok: bool,
    /// Carrier solution is RTK Fixed (when RTK expected)
    pub check_carrier_ok: bool,
    /// Correction age within limit
    pub check_correction_age_ok: bool,
    /// Signal quality (C/N0) within limits
    pub check_signal_quality_ok: bool,
    /// No critical jamming detected
    pub check_jamming_ok: bool,
    /// No multiple spoofers detected
    pub check_spoofing_ok: bool,
    /// Antenna status is OK (not open/short)
    pub check_antenna_ok: bool,
    /// Horizontal PL within alert limit
    pub check_pl_horizontal_ok: bool,
    /// Vertical PL within alert limit
    pub check_pl_vertical_ok: bool,
    /// Velocity PL within alert limit
    pub check_pl_velocity_ok: bool,
    /// Protection level validity check passed
    pub check_pl_valid_ok: bool,

    // Position quality
    /// Fix type (0=none, 1=dead-reck, 2=2D, 3=3D, 4=GNSS+DR, 5=time-only)
    pub fix_type: FixType,
    /// Carrier solution (0=none, 1=float, 2=fixed)
    pub carrier_solution: u8,
    /// Differential corrections applied
    pub differential_applied: bool,
    /// Number of satellites used
    pub num_satellites: u8,
    /// Horizontal accuracy (m)
    pub h_accuracy_m: f32,
    /// Vertical accuracy (m)
    pub v_accuracy_m: f32,
    /// Position DOP
    pub pdop: f32,
    /// Covariance data valid (matrices available in NavSatFix/TwistWithCovarianceStamped)
    pub covariance_valid: bool,

    // RTK status
    /// Age of differential corrections (-1 if no RTK)
    pub correction_age_s: f32,
    /// RTCM/correction data received
    pub correction_received: bool,
    /// Correction data was used by receiver
    pub correction_used: bool,

    // Security
    /// Jamming state (0=unknown, 1=ok, 2=warning, 3=critical)
    pub jamming_state: JammingStateData,
    /// Spoofing state (0=unknown, 1=ok, 2=indicated, 3=multiple)
    pub spoofing_state: SpoofingStateData,
    /// Number of security events logged
    pub security_events: u32,

    // Hardware
    /// Antenna status
    pub antenna_status: AntennaStatus,
    /// Jamming indicator (0-255 scale from MON-HW)
    pub jamming_indicator: u8,

    // Communication
    /// Number of communication ports
    pub comm_ports: u8,
    /// TX errors detected
    pub comm_tx_errors: u8,

    // Signal quality (from NAV-SAT)
    /// Mean C/N0 of satellites used in solution (dB-Hz)
    pub mean_cno: f32,
    /// Minimum C/N0 among used satellites (dB-Hz)
    pub min_cno: u8,
    /// Number of satellites with C/N0 >= 30 dB-Hz
    pub sats_above_cno_threshold: u8,

    // Protection Level (from NAV-PL)
    /// Protection level data valid and current
    pub protection_level_valid: bool,
    /// Horizontal protection level (m) - computed from pos_pl axes
    pub horizontal_pl_m: f32,
    /// Vertical protection level (m)
    pub vertical_pl_m: f32,
    /// Velocity protection level magnitude (m/s)
    pub velocity_pl_ms: f32,
    /// Target Misleading Information Risk [%MI/epoch]
    pub target_mir: f64,
    /// Protection level reference frame (0=invalid, 1=NED, 2=LLV, 3=ellipse)
    pub pl_frame: u8,
    /// Protection level invalidity reason (0=valid, 1=not available, 2=not trustworthy, 3=not verified)
    pub pl_invalidity_reason: u8,
}

impl Default for GnssIntegrity {
    fn default() -> Self {
        Self {
            level: IntegrityLevel::Failed, // Default to failed until data received
            status_message: "Initializing".to_string(),

            // Check results default to false (not passed) until computed
            check_fix_type_ok: false,
            check_satellites_ok: false,
            check_h_accuracy_ok: false,
            check_v_accuracy_ok: false,
            check_pdop_ok: false,
            check_carrier_ok: false,
            check_correction_age_ok: false,
            check_signal_quality_ok: false,
            check_jamming_ok: false,
            check_spoofing_ok: false,
            check_antenna_ok: false,
            check_pl_horizontal_ok: false,
            check_pl_vertical_ok: false,
            check_pl_velocity_ok: false,
            check_pl_valid_ok: false,

            // Position quality
            fix_type: FixType::default(),
            carrier_solution: 0,
            differential_applied: false,
            num_satellites: 0,
            h_accuracy_m: 0.0,
            v_accuracy_m: 0.0,
            pdop: 0.0,
            covariance_valid: false,

            // RTK status
            correction_age_s: -1.0,
            correction_received: false,
            correction_used: false,

            // Security
            jamming_state: JammingStateData::default(),
            spoofing_state: SpoofingStateData::default(),
            security_events: 0,

            // Hardware
            antenna_status: AntennaStatus::default(),
            jamming_indicator: 0,

            // Communication
            comm_ports: 0,
            comm_tx_errors: 0,

            // Signal quality
            mean_cno: 0.0,
            min_cno: 0,
            sats_above_cno_threshold: 0,

            // Protection Level
            protection_level_valid: false,
            horizontal_pl_m: 0.0,
            vertical_pl_m: 0.0,
            velocity_pl_ms: 0.0,
            target_mir: 0.0,
            pl_frame: 0,
            pl_invalidity_reason: 0, // Default to "Not Available" (0)
        }
    }
}

/// Integrity aggregator that combines quality metrics from multiple sources.
#[derive(Debug, Default)]
pub struct IntegrityAggregator {
    /// Configurable thresholds
    pub thresholds: IntegrityThresholds,
    /// Current integrity state
    current: GnssIntegrity,
    /// Last covariance data
    last_cov: Option<CovData>,
    /// Last ECEF position
    last_pos_ecef: Option<PosEcefData>,
    /// Last security signal status
    last_sec_sig: Option<SecSigData>,
    /// Last security event log
    last_sec_siglog: Option<SecSiglogData>,
    /// Last correction status
    last_rxm_cor: Option<RxmCorData>,
    /// Last communication status
    last_mon_comms: Option<MonCommsData>,
    /// Last hardware status
    last_mon_hw: Option<MonHwData>,
    /// Timestamp of last PVT update (None = never received)
    last_pvt_update: Option<Instant>,
    /// Last protection level data (NAV-PL)
    last_nav_pl: Option<NavPlData>,
}

impl IntegrityAggregator {
    /// Create a new integrity aggregator with default thresholds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new integrity aggregator with custom thresholds.
    pub fn with_thresholds(thresholds: IntegrityThresholds) -> Self {
        Self {
            thresholds,
            ..Default::default()
        }
    }

    /// Update with position/velocity covariance data (NAV-COV).
    ///
    /// Note: Covariance matrices are available in NavSatFix/TwistWithCovarianceStamped.
    /// This only tracks validity for integrity monitoring.
    pub fn update_covariance(&mut self, cov: &CovData) {
        self.last_cov = Some(cov.clone());
        self.current.covariance_valid = cov.pos_cov_valid && cov.vel_cov_valid;
    }

    /// Update with ECEF position data (NAV-POSECEF).
    pub fn update_pos_ecef(&mut self, pos: &PosEcefData) {
        self.last_pos_ecef = Some(pos.clone());
        // ECEF position is stored for potential coordinate transforms
        // but doesn't directly affect integrity level
    }

    /// Update with security signal status (SEC-SIG).
    pub fn update_sec_sig(&mut self, sig: &SecSigData) {
        self.last_sec_sig = Some(sig.clone());
        self.current.jamming_state = sig.jamming_state;
        self.current.spoofing_state = sig.spoofing_state;
    }

    /// Update with security event log (SEC-SIGLOG).
    pub fn update_sec_siglog(&mut self, siglog: &SecSiglogData) {
        self.last_sec_siglog = Some(siglog.clone());
        self.current.security_events = siglog.num_events as u32;
    }

    /// Update with differential correction status (RXM-COR).
    pub fn update_rxm_cor(&mut self, cor: &RxmCorData) {
        self.last_rxm_cor = Some(cor.clone());
        self.current.correction_received = true;
        self.current.correction_used = cor.msg_used == CorrectionMsgUsed::Used;
    }

    /// Update with communication port status (MON-COMMS).
    pub fn update_mon_comms(&mut self, comms: &MonCommsData) {
        self.last_mon_comms = Some(comms.clone());
        self.current.comm_ports = comms.n_ports;
        self.current.comm_tx_errors = comms.tx_errors;
    }

    /// Update with hardware status (MON-HW).
    pub fn update_mon_hw(&mut self, hw: &MonHwData) {
        self.last_mon_hw = Some(hw.clone());

        // Map antenna status
        self.current.antenna_status = match hw.antenna_status {
            AntennaStatusData::Init => AntennaStatus::Unknown,
            AntennaStatusData::Unknown => AntennaStatus::Unknown,
            AntennaStatusData::Ok => AntennaStatus::Ok,
            AntennaStatusData::Short => AntennaStatus::Short,
            AntennaStatusData::Open => AntennaStatus::Open,
        };

        // Store jamming indicator (0-255 scale)
        self.current.jamming_indicator = hw.jam_ind;
    }

    /// Update with RF status (MON-RF) - for antenna status only.
    /// Note: jammingState in MON-RF is deprecated and always 0 on firmware supporting SEC-SIG.
    /// Use SEC-SIG for jamming/spoofing state instead.
    pub fn update_mon_rf(&mut self, rf: &MonRfData) {
        // Map antenna status (MON-RF is the authoritative source for this)
        self.current.antenna_status = match rf.antenna_status {
            AntennaStatusData::Init => AntennaStatus::Unknown,
            AntennaStatusData::Unknown => AntennaStatus::Unknown,
            AntennaStatusData::Ok => AntennaStatus::Ok,
            AntennaStatusData::Short => AntennaStatus::Short,
            AntennaStatusData::Open => AntennaStatus::Open,
        };

        // Store jamming indicator (0-255 CW jamming scale) from MON-RF
        // Note: This is the only source for the numeric indicator; SEC-SIG only provides enum state
        self.current.jamming_indicator = rf.jam_ind;

        // Note: Do NOT update jamming_state from MON-RF - it's deprecated and always 0.
        // Jamming state comes from SEC-SIG instead.
    }

    /// Update with PVT (position/velocity/time) data.
    ///
    /// This is typically called every epoch with the main navigation solution.
    /// The `diff_corr_age_s` parameter is the device-reported correction age from NAV-PVT flags3
    /// (None if not available, Some(1-15) seconds otherwise).
    #[allow(clippy::too_many_arguments)]
    pub fn update_pvt(
        &mut self,
        fix_type: FixType,
        carrier_solution: u8,
        differential_applied: bool,
        num_satellites: u8,
        h_accuracy_m: f32,
        v_accuracy_m: f32,
        pdop: f32,
        diff_corr_age_s: Option<u8>,
    ) {
        // Record timestamp for staleness tracking
        self.last_pvt_update = Some(Instant::now());

        self.current.fix_type = fix_type;
        self.current.carrier_solution = carrier_solution;
        self.current.differential_applied = differential_applied;
        self.current.num_satellites = num_satellites;
        self.current.h_accuracy_m = h_accuracy_m;
        self.current.v_accuracy_m = v_accuracy_m;
        self.current.pdop = pdop;

        // Use device-reported correction age (authoritative source)
        // -1.0 indicates no correction age available (either not in RTK mode or device not reporting)
        self.current.correction_age_s = diff_corr_age_s.map(|a| a as f32).unwrap_or(-1.0);

        // Infer correction status from NAV-PVT differential flag
        // This is more reliable than RXM-COR which only reports SPARTN, not RTCM
        if differential_applied {
            self.current.correction_received = true;
            self.current.correction_used = true;
        }
    }

    /// Update with signal quality metrics from NAV-SAT.
    pub fn update_signal_quality(&mut self, quality: &SignalQuality) {
        self.current.mean_cno = quality.mean_cno;
        self.current.min_cno = quality.min_cno;
        self.current.sats_above_cno_threshold = quality.sats_above_threshold;
    }

    /// Update with protection level data (NAV-PL).
    ///
    /// Protection levels provide statistically-bounded error estimates with a specified
    /// Target Misleading Information Risk (TMIR) for ISO 26262/ISO 21448 compliance.
    pub fn update_nav_pl(&mut self, pl: &NavPlData) {
        self.last_nav_pl = Some(pl.clone());

        self.current.protection_level_valid = pl.pos_valid;
        self.current.target_mir = pl.tmir;

        // Convert frame enum to u8 for ROS message
        self.current.pl_frame = match pl.pos_frame {
            NavPlFrame::Invalid => 0,
            NavPlFrame::Ned => 1,
            NavPlFrame::LongLatVert => 2,
            NavPlFrame::Ellipse => 3,
        };

        // Convert invalidity reason enum to u8 for ROS message
        // Mapping aligned with UBX-NAV-PL manual:
        // 0: Not available (also used for Valid as there is no invalidity reason)
        // 1: Solution not trustworthy (1-29)
        // 30: PL not verified for this receiver configuration (30-100)
        self.current.pl_invalidity_reason = match pl.pos_invalidity_reason {
            NavPlInvalidityReason::Valid => 0,
            NavPlInvalidityReason::NotAvailable => 0,
            NavPlInvalidityReason::SolutionNotTrustworthy => 1,
            NavPlInvalidityReason::NotVerifiedForConfig => 30,
        };

        if pl.pos_valid {
            // Compute horizontal PL as 2D magnitude of first two axes
            // For NED frame: sqrt(N^2 + E^2), for Ellipse: semi-major axis
            let horizontal_pl = if matches!(pl.pos_frame, NavPlFrame::Ellipse) {
                // For ellipse frame, pos_pl_m[0] is semi-major axis (worst case horizontal)
                pl.pos_pl_m[0] as f32
            } else {
                // For NED/LLV, compute 2D magnitude
                ((pl.pos_pl_m[0].powi(2) + pl.pos_pl_m[1].powi(2)).sqrt()) as f32
            };
            self.current.horizontal_pl_m = horizontal_pl;
            self.current.vertical_pl_m = pl.pos_pl_m[2] as f32;
        } else {
            self.current.horizontal_pl_m = 0.0;
            self.current.vertical_pl_m = 0.0;
        }

        if pl.vel_valid {
            // Compute velocity PL magnitude (3D)
            let vel_pl =
                ((pl.vel_pl_ms[0].powi(2) + pl.vel_pl_ms[1].powi(2) + pl.vel_pl_ms[2].powi(2))
                    .sqrt()) as f32;
            self.current.velocity_pl_ms = vel_pl;
        } else {
            self.current.velocity_pl_ms = 0.0;
        }
    }

    /// Compute the overall integrity level based on current data.
    ///
    /// Returns the computed `GnssIntegrity` with updated level, status message,
    /// and individual check results for diagnostic visibility.
    pub fn compute(&mut self) -> GnssIntegrity {
        let mut issues: Vec<&str> = Vec::new();
        let mut level = IntegrityLevel::Ok;

        // =========================================================================
        // STALENESS CHECK: Return Failed if data is stale or never received
        // =========================================================================
        let now = Instant::now();
        if let Some(last_pvt) = self.last_pvt_update {
            let age = now.duration_since(last_pvt).as_secs_f32();
            if age > self.thresholds.max_pvt_age_s {
                // Reset all checks to false on stale data
                self.reset_check_results();
                self.current.level = IntegrityLevel::Failed;
                self.current.status_message = format!(
                    "GNSS data stale ({:.1}s > {:.1}s threshold)",
                    age, self.thresholds.max_pvt_age_s
                );
                return self.current.clone();
            }
        } else {
            // Never received PVT data - reset all checks
            self.reset_check_results();
            self.current.level = IntegrityLevel::Failed;
            self.current.status_message = "Waiting for GNSS data".to_string();
            return self.current.clone();
        }

        // =========================================================================
        // EVALUATE ALL CHECKS (set boolean results for diagnostic visibility)
        // =========================================================================

        // Fix type check
        let fix_type_ok = !matches!(
            self.current.fix_type,
            FixType::NoFix | FixType::Fix2D | FixType::DeadReckoning | FixType::TimeOnly
        );
        self.current.check_fix_type_ok = fix_type_ok;
        if !fix_type_ok {
            level = level.max(IntegrityLevel::Critical);
            if matches!(self.current.fix_type, FixType::NoFix) {
                issues.push("No GNSS fix");
            } else {
                issues.push("Insufficient fix type");
            }
        }

        // Satellite count check (critical threshold)
        let sats_critical_ok =
            self.current.num_satellites >= self.thresholds.min_satellites_critical;
        // Satellite count check includes both critical and high thresholds
        let sats_high_ok = self.current.num_satellites >= self.thresholds.min_satellites_high;
        self.current.check_satellites_ok = sats_critical_ok && sats_high_ok;
        if !sats_critical_ok {
            level = level.max(IntegrityLevel::Critical);
            issues.push("Too few satellites");
        }

        // Jamming check
        let jamming_ok = !matches!(self.current.jamming_state, JammingStateData::Critical);
        self.current.check_jamming_ok = jamming_ok;
        if !jamming_ok {
            level = level.max(IntegrityLevel::Critical);
            issues.push("Critical jamming detected");
        }

        // Spoofing check
        let spoofing_ok = !matches!(self.current.spoofing_state, SpoofingStateData::Multiple);
        self.current.check_spoofing_ok = spoofing_ok;
        if !spoofing_ok {
            level = level.max(IntegrityLevel::Critical);
            issues.push("Multiple spoofers detected");
        }

        // Antenna check
        let antenna_ok = !matches!(
            self.current.antenna_status,
            AntennaStatus::Short | AntennaStatus::Open
        );
        self.current.check_antenna_ok = antenna_ok;
        if !antenna_ok {
            level = level.max(IntegrityLevel::Critical);
            match self.current.antenna_status {
                AntennaStatus::Short => issues.push("Antenna short circuit"),
                AntennaStatus::Open => issues.push("Antenna open circuit"),
                _ => {}
            }
        }

        // Protection level validity check (if required)
        let pl_valid_ok = if self.thresholds.require_valid_pl {
            if let Some(ref pl) = self.last_nav_pl {
                let valid = pl.pos_valid
                    && !matches!(
                        pl.pos_invalidity_reason,
                        NavPlInvalidityReason::SolutionNotTrustworthy
                    );
                if !valid {
                    level = level.max(IntegrityLevel::Critical);
                    if !pl.pos_valid {
                        issues.push("Protection level invalid");
                    } else {
                        issues.push("Solution not trustworthy");
                    }
                }
                valid
            } else {
                level = level.max(IntegrityLevel::Critical);
                issues.push("Protection level not available");
                false
            }
        } else {
            // Not required, so always passes
            true
        };
        self.current.check_pl_valid_ok = pl_valid_ok;

        // =========================================================================
        // LEVEL 1: HIGH QUALITY CHECKS (Degraded operation if failed)
        // Only evaluate if not already CRITICAL
        // =========================================================================

        // Carrier solution check (for RTK applications)
        let carrier_ok = !(self.current.carrier_solution < 2 && self.current.differential_applied);
        self.current.check_carrier_ok = carrier_ok;
        if level < IntegrityLevel::Critical && !carrier_ok {
            level = level.max(IntegrityLevel::Degraded);
            issues.push("RTK not fixed");
        }

        // Horizontal accuracy check
        let h_accuracy_ok = self.current.h_accuracy_m <= self.thresholds.max_h_accuracy_m;
        self.current.check_h_accuracy_ok = h_accuracy_ok;
        if level < IntegrityLevel::Critical && !h_accuracy_ok {
            level = level.max(IntegrityLevel::Degraded);
            issues.push("Horizontal accuracy exceeded");
        }

        // Vertical accuracy check
        let v_accuracy_ok = self.current.v_accuracy_m <= self.thresholds.max_v_accuracy_m;
        self.current.check_v_accuracy_ok = v_accuracy_ok;
        if level < IntegrityLevel::Critical && !v_accuracy_ok {
            level = level.max(IntegrityLevel::Degraded);
            issues.push("Vertical accuracy exceeded");
        }

        // PDOP check
        let pdop_ok = self.current.pdop <= self.thresholds.max_pdop;
        self.current.check_pdop_ok = pdop_ok;
        if level < IntegrityLevel::Critical && !pdop_ok {
            level = level.max(IntegrityLevel::Degraded);
            issues.push("PDOP too high");
        }

        // Satellite count (high quality threshold) - already computed above
        if level < IntegrityLevel::Critical && sats_critical_ok && !sats_high_ok {
            level = level.max(IntegrityLevel::Degraded);
            issues.push("Low satellite count");
        }

        // Correction age check
        let correction_age_ok = !(self.current.differential_applied
            && self.current.correction_age_s >= 0.0
            && self.current.correction_age_s > self.thresholds.max_correction_age_s);
        self.current.check_correction_age_ok = correction_age_ok;
        if level < IntegrityLevel::Critical && !correction_age_ok {
            level = level.max(IntegrityLevel::Degraded);
            issues.push("Correction age exceeded");
        }

        // Signal quality check (combines min and mean C/N0)
        let min_cno_ok =
            self.current.min_cno == 0 || self.current.min_cno >= self.thresholds.min_cno_degraded;
        let mean_cno_ok = self.current.mean_cno == 0.0
            || self.current.mean_cno >= self.thresholds.min_mean_cno_degraded;
        let signal_quality_ok = min_cno_ok && mean_cno_ok;
        self.current.check_signal_quality_ok = signal_quality_ok;
        if level < IntegrityLevel::Critical {
            if !min_cno_ok {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("Weak satellite signal");
            }
            if !mean_cno_ok {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("Low mean signal quality");
            }
        }

        // =========================================================================
        // PROTECTION LEVEL CHECKS (ISO 26262/SOTIF compliant bounds)
        // =========================================================================

        // Horizontal PL check
        let pl_horizontal_ok = if let Some(ref pl) = self.last_nav_pl {
            if pl.pos_valid {
                let ok = self.current.horizontal_pl_m <= self.thresholds.max_horizontal_pl_m;
                if level < IntegrityLevel::Critical && !ok {
                    level = level.max(IntegrityLevel::Degraded);
                    issues.push("Horizontal PL exceeds alert limit");
                }
                ok
            } else {
                true // Not available, don't fail
            }
        } else {
            true // Not available, don't fail
        };
        self.current.check_pl_horizontal_ok = pl_horizontal_ok;

        // Vertical PL check
        let pl_vertical_ok = if let Some(ref pl) = self.last_nav_pl {
            if pl.pos_valid {
                let ok = self.current.vertical_pl_m <= self.thresholds.max_vertical_pl_m;
                if level < IntegrityLevel::Critical && !ok {
                    level = level.max(IntegrityLevel::Degraded);
                    issues.push("Vertical PL exceeds alert limit");
                }
                ok
            } else {
                true // Not available, don't fail
            }
        } else {
            true // Not available, don't fail
        };
        self.current.check_pl_vertical_ok = pl_vertical_ok;

        // Velocity PL check
        let pl_velocity_ok = if let Some(ref pl) = self.last_nav_pl {
            if pl.vel_valid {
                let ok = self.current.velocity_pl_ms <= self.thresholds.max_velocity_pl_ms;
                if level < IntegrityLevel::Critical && !ok {
                    level = level.max(IntegrityLevel::Degraded);
                    issues.push("Velocity PL exceeds alert limit");
                }
                ok
            } else {
                true // Not available, don't fail
            }
        } else {
            true // Not available, don't fail
        };
        self.current.check_pl_velocity_ok = pl_velocity_ok;

        // TMIR check (included in PL validity conceptually)
        if let Some(ref pl) = self.last_nav_pl {
            if pl.tmir > self.thresholds.max_tmir_per_epoch && level < IntegrityLevel::Critical {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("TMIR exceeds threshold");
            }
        }

        // =========================================================================
        // LEVEL 2: MONITOR CHECKS (Log and alert, no operational impact)
        // =========================================================================

        // These don't change the level but are noted in the status
        if matches!(self.current.jamming_state, JammingStateData::Warning) {
            issues.push("Jamming warning");
        }

        if matches!(self.current.spoofing_state, SpoofingStateData::Indicated) {
            issues.push("Spoofing indicated");
        }

        if self.current.security_events > 0 {
            issues.push("Security events logged");
        }

        // Build status message
        let status_message = if issues.is_empty() {
            "All integrity checks passed".to_string()
        } else {
            issues.join("; ")
        };

        self.current.level = level;
        self.current.status_message = status_message;

        self.current.clone()
    }

    /// Reset all check results to false (used when data is stale/unavailable)
    fn reset_check_results(&mut self) {
        self.current.check_fix_type_ok = false;
        self.current.check_satellites_ok = false;
        self.current.check_h_accuracy_ok = false;
        self.current.check_v_accuracy_ok = false;
        self.current.check_pdop_ok = false;
        self.current.check_carrier_ok = false;
        self.current.check_correction_age_ok = false;
        self.current.check_signal_quality_ok = false;
        self.current.check_jamming_ok = false;
        self.current.check_spoofing_ok = false;
        self.current.check_antenna_ok = false;
        self.current.check_pl_horizontal_ok = false;
        self.current.check_pl_vertical_ok = false;
        self.current.check_pl_velocity_ok = false;
        self.current.check_pl_valid_ok = false;
    }

    /// Get the current integrity state without recomputing.
    pub fn current(&self) -> &GnssIntegrity {
        &self.current
    }

    /// Check if the system is operational based on configurable threshold.
    ///
    /// By default (threshold=1), operational is true for `Ok` and `Degraded`.
    /// With threshold=0 (strictest), only `Ok` is operational.
    /// With threshold=2, `Ok`, `Degraded`, and `Critical` are all operational.
    pub fn is_operational(&self) -> bool {
        (self.current.level as u8) <= self.thresholds.operational_threshold
    }

    /// Check if the system is at full quality (level is Ok).
    pub fn is_full_quality(&self) -> bool {
        self.current.level == IntegrityLevel::Ok
    }
}

impl IntegrityLevel {
    fn max(self, other: Self) -> Self {
        if (other as u8) > (self as u8) {
            other
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_integrity_level() {
        let integrity = GnssIntegrity::default();
        assert_eq!(integrity.level, IntegrityLevel::Failed);
    }

    #[test]
    fn test_aggregator_critical_no_fix() {
        let mut agg = IntegrityAggregator::new();
        agg.update_pvt(FixType::NoFix, 0, false, 0, 0.0, 0.0, 0.0, None);
        let result = agg.compute();
        assert_eq!(result.level, IntegrityLevel::Critical);
        assert!(result.status_message.contains("No GNSS fix"));
    }

    #[test]
    fn test_aggregator_critical_jamming() {
        let mut agg = IntegrityAggregator::new();
        agg.update_pvt(FixType::Fix3D, 0, false, 8, 0.05, 0.08, 1.5, None);
        agg.update_sec_sig(&SecSigData {
            jam_det_enabled: true,
            spf_det_enabled: true,
            jamming_state: JammingStateData::Critical,
            spoofing_state: SpoofingStateData::Ok,
            jam_num_cent_freqs: 0,
            cent_freq_khz: vec![],
            jammed: vec![],
        });
        let result = agg.compute();
        assert_eq!(result.level, IntegrityLevel::Critical);
        assert!(result.status_message.contains("jamming"));
    }

    #[test]
    fn test_aggregator_degraded_accuracy() {
        let mut agg = IntegrityAggregator::new();
        agg.update_pvt(FixType::Fix3D, 0, false, 8, 0.5, 0.8, 1.5, None); // Poor accuracy
        let result = agg.compute();
        assert_eq!(result.level, IntegrityLevel::Degraded);
        assert!(result.status_message.contains("accuracy"));
    }

    #[test]
    fn test_aggregator_ok_rtk_fixed() {
        let mut agg = IntegrityAggregator::new();
        agg.update_pvt(FixType::RtkFixed, 2, true, 12, 0.02, 0.03, 1.2, Some(2));
        agg.update_sec_sig(&SecSigData {
            jam_det_enabled: true,
            spf_det_enabled: true,
            jamming_state: JammingStateData::Ok,
            spoofing_state: SpoofingStateData::Ok,
            jam_num_cent_freqs: 0,
            cent_freq_khz: vec![],
            jammed: vec![],
        });
        // Correction age is now set via update_pvt's diff_corr_age_s parameter
        let result = agg.compute();
        assert_eq!(result.level, IntegrityLevel::Ok);
        assert!(agg.is_operational());
        assert!(agg.is_full_quality());
    }

    #[test]
    fn test_covariance_valid_flag() {
        let mut agg = IntegrityAggregator::new();
        let cov = CovData {
            itow: 0,
            pos_cov_valid: true,
            vel_cov_valid: true,
            pos_cov: [1.0, 0.5, 0.1, 2.0, 0.2, 3.0],
            vel_cov: [0.1, 0.05, 0.01, 0.2, 0.02, 0.3],
        };
        agg.update_covariance(&cov);

        // Covariance matrices are now in NavSatFix/Twist; we only track validity
        assert!(agg.current().covariance_valid);

        // Test partial validity
        let cov_partial = CovData {
            itow: 0,
            pos_cov_valid: true,
            vel_cov_valid: false,
            pos_cov: [1.0; 6],
            vel_cov: [0.0; 6],
        };
        agg.update_covariance(&cov_partial);
        assert!(!agg.current().covariance_valid); // Both must be valid
    }
}
