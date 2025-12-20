//! GNSS integrity monitoring and aggregation.
//!
//! This module implements safety-critical integrity monitoring as described in
//! `docs/INTEGRITY_AND_TOPICS.md`. It aggregates quality metrics
//! from multiple UBX messages to determine overall GNSS solution integrity.

use std::time::Instant;

use crate::device::{
    AntennaStatusData, CovData, JammingStateData, MonCommsData, MonHwData, MonRfData, PosEcefData,
    RxmCorData, SecSigData, SecSiglogData, SpoofingStateData,
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
    /// Maximum correction age (s) for RTK applications
    pub max_correction_age_s: f32,
    /// Maximum time (seconds) without PVT before declaring integrity FAILED
    pub max_pvt_age_s: f32,
    /// IntegrityLevel at which operational becomes false
    /// 0 = only Ok is operational (strictest)
    /// 1 = Ok or Degraded is operational (default)
    /// 2 = Ok/Degraded/Critical is operational (permissive)
    pub operational_threshold: u8,
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
            max_pvt_age_s: 2.0,       // 2 seconds without data = Failed
            operational_threshold: 1, // Ok or Degraded = operational
        }
    }
}

/// Aggregated GNSS integrity data.
///
/// This structure combines all quality metrics from various UBX messages
/// into a single integrity assessment, as proposed in Section 6.4.
#[derive(Debug, Clone, Default)]
pub struct GnssIntegrity {
    /// Overall integrity level
    pub level: IntegrityLevel,
    /// Human-readable status message
    pub status_message: String,

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
    /// Position covariance matrix (ENU, row-major, 3x3)
    pub position_covariance: [f32; 9],
    /// Velocity covariance matrix (ENU, row-major, 3x3)
    pub velocity_covariance: [f32; 9],
    /// Covariance data valid
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
    pub fn update_covariance(&mut self, cov: &CovData) {
        self.last_cov = Some(cov.clone());

        self.current.covariance_valid = cov.pos_cov_valid && cov.vel_cov_valid;

        if cov.pos_cov_valid {
            // Convert NED covariance to ENU (swap N<->E, negate D->U)
            // NED: [NN, NE, ND, EE, ED, DD] -> ENU: [EE, EN, EU, NN, NU, UU]
            self.current.position_covariance = [
                cov.pos_cov[3],  // EE
                cov.pos_cov[1],  // EN (same as NE)
                -cov.pos_cov[4], // EU (negated ED)
                cov.pos_cov[1],  // NE (same as EN)
                cov.pos_cov[0],  // NN
                -cov.pos_cov[2], // NU (negated ND)
                -cov.pos_cov[4], // UE (negated DE)
                -cov.pos_cov[2], // UN (negated DN)
                cov.pos_cov[5],  // UU (same as DD)
            ];
        }

        if cov.vel_cov_valid {
            // Same NED->ENU conversion for velocity covariance
            self.current.velocity_covariance = [
                cov.vel_cov[3],  // EE
                cov.vel_cov[1],  // EN
                -cov.vel_cov[4], // EU
                cov.vel_cov[1],  // NE
                cov.vel_cov[0],  // NN
                -cov.vel_cov[2], // NU
                -cov.vel_cov[4], // UE
                -cov.vel_cov[2], // UN
                cov.vel_cov[5],  // UU
            ];
        }
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

        // Infer correction status from NAV-PVT differential flag
        // This is more reliable than RXM-COR which only reports SPARTN, not RTCM
        if differential_applied {
            self.current.correction_received = true;
            self.current.correction_used = true;
        }
    }

    /// Set the correction age from NAV-PVT (proto27+).
    pub fn set_correction_age(&mut self, age_s: f32) {
        self.current.correction_age_s = age_s;
    }

    /// Compute the overall integrity level based on current data.
    ///
    /// Returns the computed `GnssIntegrity` with updated level and status message.
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
                self.current.level = IntegrityLevel::Failed;
                self.current.status_message = format!(
                    "GNSS data stale ({:.1}s > {:.1}s threshold)",
                    age, self.thresholds.max_pvt_age_s
                );
                return self.current.clone();
            }
        } else {
            // Never received PVT data
            self.current.level = IntegrityLevel::Failed;
            self.current.status_message = "Waiting for GNSS data".to_string();
            return self.current.clone();
        }

        // =========================================================================
        // LEVEL 0: CRITICAL CHECKS (Operation MUST stop if failed)
        // =========================================================================

        // Check fix type
        match self.current.fix_type {
            FixType::NoFix => {
                level = IntegrityLevel::Critical;
                issues.push("No GNSS fix");
            }
            FixType::Fix2D | FixType::DeadReckoning | FixType::TimeOnly => {
                level = level.max(IntegrityLevel::Critical);
                issues.push("Insufficient fix type");
            }
            _ => {}
        }

        // Check minimum satellites (critical threshold)
        if self.current.num_satellites < self.thresholds.min_satellites_critical {
            level = level.max(IntegrityLevel::Critical);
            issues.push("Too few satellites");
        }

        // Check jamming state
        if matches!(self.current.jamming_state, JammingStateData::Critical) {
            level = level.max(IntegrityLevel::Critical);
            issues.push("Critical jamming detected");
        }

        // Check spoofing state
        if matches!(self.current.spoofing_state, SpoofingStateData::Multiple) {
            level = level.max(IntegrityLevel::Critical);
            issues.push("Multiple spoofers detected");
        }

        // Check antenna status (critical if short or open)
        match self.current.antenna_status {
            AntennaStatus::Short => {
                level = level.max(IntegrityLevel::Critical);
                issues.push("Antenna short circuit");
            }
            AntennaStatus::Open => {
                level = level.max(IntegrityLevel::Critical);
                issues.push("Antenna open circuit");
            }
            _ => {}
        }

        // =========================================================================
        // LEVEL 1: HIGH QUALITY CHECKS (Degraded operation if failed)
        // =========================================================================

        // Only check Level 1 if we passed Level 0
        if level < IntegrityLevel::Critical {
            // Check carrier solution for RTK applications
            if self.current.carrier_solution < 2 && self.current.differential_applied {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("RTK not fixed");
            }

            // Check horizontal accuracy
            if self.current.h_accuracy_m > self.thresholds.max_h_accuracy_m {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("Horizontal accuracy exceeded");
            }

            // Check vertical accuracy
            if self.current.v_accuracy_m > self.thresholds.max_v_accuracy_m {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("Vertical accuracy exceeded");
            }

            // Check PDOP
            if self.current.pdop > self.thresholds.max_pdop {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("PDOP too high");
            }

            // Check satellite count (high quality threshold)
            if self.current.num_satellites < self.thresholds.min_satellites_high {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("Low satellite count");
            }

            // Check correction age for RTK
            if self.current.differential_applied
                && self.current.correction_age_s > self.thresholds.max_correction_age_s
            {
                level = level.max(IntegrityLevel::Degraded);
                issues.push("Correction age exceeded");
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
        agg.update_pvt(FixType::NoFix, 0, false, 0, 0.0, 0.0, 0.0);
        let result = agg.compute();
        assert_eq!(result.level, IntegrityLevel::Critical);
        assert!(result.status_message.contains("No GNSS fix"));
    }

    #[test]
    fn test_aggregator_critical_jamming() {
        let mut agg = IntegrityAggregator::new();
        agg.update_pvt(FixType::Fix3D, 0, false, 8, 0.05, 0.08, 1.5);
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
        agg.update_pvt(FixType::Fix3D, 0, false, 8, 0.5, 0.8, 1.5); // Poor accuracy
        let result = agg.compute();
        assert_eq!(result.level, IntegrityLevel::Degraded);
        assert!(result.status_message.contains("accuracy"));
    }

    #[test]
    fn test_aggregator_ok_rtk_fixed() {
        let mut agg = IntegrityAggregator::new();
        agg.update_pvt(FixType::RtkFixed, 2, true, 12, 0.02, 0.03, 1.2);
        agg.update_sec_sig(&SecSigData {
            jam_det_enabled: true,
            spf_det_enabled: true,
            jamming_state: JammingStateData::Ok,
            spoofing_state: SpoofingStateData::Ok,
            jam_num_cent_freqs: 0,
            cent_freq_khz: vec![],
            jammed: vec![],
        });
        agg.set_correction_age(2.0);
        let result = agg.compute();
        assert_eq!(result.level, IntegrityLevel::Ok);
        assert!(agg.is_operational());
        assert!(agg.is_full_quality());
    }

    #[test]
    fn test_covariance_ned_to_enu() {
        let mut agg = IntegrityAggregator::new();
        let cov = CovData {
            itow: 0,
            pos_cov_valid: true,
            vel_cov_valid: true,
            pos_cov: [1.0, 0.5, 0.1, 2.0, 0.2, 3.0], // NN, NE, ND, EE, ED, DD
            vel_cov: [0.1, 0.05, 0.01, 0.2, 0.02, 0.3],
        };
        agg.update_covariance(&cov);

        // Check ENU conversion: EE should be at [0], NN at [4], UU at [8]
        assert_eq!(agg.current.position_covariance[0], 2.0); // EE
        assert_eq!(agg.current.position_covariance[4], 1.0); // NN
        assert_eq!(agg.current.position_covariance[8], 3.0); // UU
    }
}
