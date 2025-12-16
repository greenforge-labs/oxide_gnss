//! Conversions from internal types to ROS2 message types.

use crate::config::CoordinateFrame;
use crate::device::ubx::{HpPosData, PvtData, SecSigData};
use crate::device::{JammingStateData, SpoofingStateData};
use crate::state::{AntennaStatus, FixType, GnssIntegrity};
use crate::transform;

/// Trait for converting to ROS2 messages.
pub trait ToRosMessage<T> {
    /// Convert to a ROS2 message type.
    fn to_ros_msg(&self) -> T;
}

/// Convert SecSigData to oxide_gnss_msgs/SecSigDetails.
impl ToRosMessage<oxide_gnss_msgs::msg::SecSigDetails> for SecSigData {
    fn to_ros_msg(&self) -> oxide_gnss_msgs::msg::SecSigDetails {
        oxide_gnss_msgs::msg::SecSigDetails {
            jam_det_enabled: self.jam_det_enabled,
            spf_det_enabled: self.spf_det_enabled,
            jamming_state: self.jamming_state as u8,
            spoofing_state: self.spoofing_state as u8,
            jam_num_cent_freqs: self.jam_num_cent_freqs,
            cent_freq_khz: self.cent_freq_khz.clone(),
            jammed: self.jammed.clone(),
            ..Default::default()
        }
    }
}

/// Convert PvtData to sensor_msgs/NavSatFix.
impl ToRosMessage<sensor_msgs::msg::NavSatFix> for PvtData {
    fn to_ros_msg(&self) -> sensor_msgs::msg::NavSatFix {
        let mut msg = sensor_msgs::msg::NavSatFix::default();

        // Header - timestamp will be set by publisher
        msg.header.frame_id = "gnss".to_string();

        // Status
        msg.status.status = fix_type_to_status(self.fix_type);
        msg.status.service = sensor_msgs::msg::NavSatStatus::SERVICE_GPS
            | sensor_msgs::msg::NavSatStatus::SERVICE_GLONASS
            | sensor_msgs::msg::NavSatStatus::SERVICE_GALILEO;

        // Position
        msg.latitude = self.lat;
        msg.longitude = self.lon;
        msg.altitude = self.height_msl;

        // Covariance (diagonal, in ENU order: east, north, up)
        // Convert horizontal/vertical accuracy to variance (accuracy is 1-sigma)
        let h_var = (self.h_acc as f64).powi(2);
        let v_var = (self.v_acc as f64).powi(2);

        msg.position_covariance = [
            h_var, 0.0, 0.0, // East
            0.0, h_var, 0.0, // North
            0.0, 0.0, v_var, // Up
        ];
        msg.position_covariance_type = sensor_msgs::msg::NavSatFix::COVARIANCE_TYPE_DIAGONAL_KNOWN;

        msg
    }
}

/// Convert PvtData to geometry_msgs/TwistWithCovarianceStamped.
/// Default implementation uses ENU (ROS convention).
impl ToRosMessage<geometry_msgs::msg::TwistWithCovarianceStamped> for PvtData {
    fn to_ros_msg(&self) -> geometry_msgs::msg::TwistWithCovarianceStamped {
        pvt_to_twist(self, CoordinateFrame::ENU)
    }
}

/// Convert HpPosData to sensor_msgs/NavSatFix.
impl ToRosMessage<sensor_msgs::msg::NavSatFix> for HpPosData {
    fn to_ros_msg(&self) -> sensor_msgs::msg::NavSatFix {
        let mut msg = sensor_msgs::msg::NavSatFix::default();
        msg.header.frame_id = "gnss".to_string();

        // High-precision messages imply RTK; set GBAS status accordingly
        msg.status.status = sensor_msgs::msg::NavSatStatus::STATUS_GBAS_FIX;

        msg.latitude = self.lat;
        msg.longitude = self.lon;
        msg.altitude = self.height; // WGS84 ellipsoid height (UBX NAV-HPPOSLLH)

        // Covariance
        // h_acc is in meters (after our parsing logic applied scaling)
        let h_var = (self.h_acc as f64).powi(2);
        let v_var = (self.v_acc as f64).powi(2);

        msg.position_covariance = [h_var, 0.0, 0.0, 0.0, h_var, 0.0, 0.0, 0.0, v_var];
        msg.position_covariance_type = sensor_msgs::msg::NavSatFix::COVARIANCE_TYPE_DIAGONAL_KNOWN;

        msg
    }
}

/// Convert PvtData to TwistWithCovarianceStamped with configurable coordinate frame.
///
/// The u-blox receiver outputs velocity in NED (North, East, Down).
/// This function transforms to the requested output frame.
///
/// # Arguments
/// * `pvt` - The PVT data from the receiver
/// * `frame` - The desired output coordinate frame (ENU or NED)
pub fn pvt_to_twist(
    pvt: &PvtData,
    frame: CoordinateFrame,
) -> geometry_msgs::msg::TwistWithCovarianceStamped {
    let mut msg = geometry_msgs::msg::TwistWithCovarianceStamped::default();

    msg.header.frame_id = match frame {
        CoordinateFrame::ENU => "gnss_enu".to_string(),
        CoordinateFrame::NED => "gnss_ned".to_string(),
    };

    // u-blox outputs NED velocities, transform to requested frame
    let (x, y, z) =
        transform::ned_to_frame(pvt.vel_n as f64, pvt.vel_e as f64, pvt.vel_d as f64, frame);

    msg.twist.twist.linear.x = x;
    msg.twist.twist.linear.y = y;
    msg.twist.twist.linear.z = z;

    // Angular velocity not available from NAV-PVT
    msg.twist.twist.angular.x = 0.0;
    msg.twist.twist.angular.y = 0.0;
    msg.twist.twist.angular.z = 0.0;

    // Covariance for linear velocity (diagonal)
    let s_var = (pvt.s_acc as f64).powi(2);
    // 6x6 covariance matrix stored row-major
    msg.twist.covariance[0] = s_var; // linear.x
    msg.twist.covariance[7] = s_var; // linear.y
    msg.twist.covariance[14] = s_var; // linear.z
                                      // Angular covariances set to high uncertainty (not measured)
    msg.twist.covariance[21] = 1e6;
    msg.twist.covariance[28] = 1e6;
    msg.twist.covariance[35] = 1e6;

    msg
}

/// Convert PvtData to sensor_msgs/TimeReference.
impl ToRosMessage<sensor_msgs::msg::TimeReference> for PvtData {
    fn to_ros_msg(&self) -> sensor_msgs::msg::TimeReference {
        let mut msg = sensor_msgs::msg::TimeReference::default();

        msg.header.frame_id = "gnss".to_string();
        msg.source = "GPS".to_string();

        // Convert UTC time from PVT to ROS time
        // Note: This creates a rough approximation - proper implementation
        // would use the nano field for sub-second precision
        let secs =
            chrono::NaiveDate::from_ymd_opt(self.year as i32, self.month as u32, self.day as u32)
                .and_then(|d| d.and_hms_opt(self.hour as u32, self.min as u32, self.sec as u32))
                .map(|dt| dt.and_utc().timestamp())
                .unwrap_or(0);

        msg.time_ref.sec = secs as i32;
        msg.time_ref.nanosec = self.nano.max(0) as u32;

        msg
    }
}

/// Convert our FixType to NavSatStatus status value.
fn fix_type_to_status(fix: FixType) -> i8 {
    match fix {
        FixType::NoFix => sensor_msgs::msg::NavSatStatus::STATUS_NO_FIX,
        FixType::Fix2D | FixType::Fix3D | FixType::GnssDr | FixType::TimeOnly => {
            sensor_msgs::msg::NavSatStatus::STATUS_FIX
        }
        FixType::RtkFloat => sensor_msgs::msg::NavSatStatus::STATUS_SBAS_FIX, // Use SBAS as proxy for float
        FixType::RtkFixed => sensor_msgs::msg::NavSatStatus::STATUS_GBAS_FIX, // Use GBAS as proxy for RTK
        FixType::DeadReckoning => sensor_msgs::msg::NavSatStatus::STATUS_NO_FIX,
    }
}

/// Create a ROS2 timestamp from the current time.
pub fn now_timestamp() -> builtin_interfaces::msg::Time {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    builtin_interfaces::msg::Time {
        sec: now.as_secs() as i32,
        nanosec: now.subsec_nanos(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn sample_pvt() -> PvtData {
        PvtData {
            itow: 0,
            year: 2024,
            month: 12,
            day: 6,
            hour: 12,
            min: 30,
            sec: 45,
            nano: 123456789,
            fix_type: FixType::RtkFixed,
            num_sv: 15,
            lon: 153.0251,
            lat: -27.4698,
            height: 55.0,
            height_msl: 50.0,
            h_acc: 0.015,
            v_acc: 0.025,
            vel_n: 1.5,
            vel_e: 2.0,
            vel_d: -0.1,
            g_speed: 2.5,
            head_mot: 53.1,
            s_acc: 0.05,
            head_acc: 2.0,
            p_dop: 1.2,
            carr_soln: crate::device::CarrierSolution::Fixed,
            received_at: Instant::now(),
        }
    }

    #[test]
    fn test_pvt_to_navsat_fix() {
        let pvt = sample_pvt();
        let msg: sensor_msgs::msg::NavSatFix = pvt.to_ros_msg();

        assert_eq!(msg.latitude, -27.4698);
        assert_eq!(msg.longitude, 153.0251);
        assert_eq!(msg.altitude, 50.0);
        assert_eq!(
            msg.status.status,
            sensor_msgs::msg::NavSatStatus::STATUS_GBAS_FIX
        );
    }

    #[test]
    fn test_pvt_to_twist_default_enu() {
        let pvt = sample_pvt();
        let msg: geometry_msgs::msg::TwistWithCovarianceStamped = pvt.to_ros_msg();

        // Default should be ENU
        assert_eq!(msg.header.frame_id, "gnss_enu");
        // Check NED to ENU conversion: vel_n=1.5, vel_e=2.0, vel_d=-0.1
        assert_eq!(msg.twist.twist.linear.x, 2.0); // East (from vel_e)
        assert_eq!(msg.twist.twist.linear.y, 1.5); // North (from vel_n)
        assert!((msg.twist.twist.linear.z - 0.1).abs() < 0.001); // Up (-vel_d)
    }

    #[test]
    fn test_pvt_to_twist_enu_frame() {
        let pvt = sample_pvt();
        let msg = pvt_to_twist(&pvt, CoordinateFrame::ENU);

        assert_eq!(msg.header.frame_id, "gnss_enu");
        // NED(1.5, 2.0, -0.1) -> ENU(2.0, 1.5, 0.1)
        assert_eq!(msg.twist.twist.linear.x, 2.0); // East
        assert_eq!(msg.twist.twist.linear.y, 1.5); // North
        assert!((msg.twist.twist.linear.z - 0.1).abs() < 0.001); // Up
    }

    #[test]
    fn test_pvt_to_twist_ned_frame() {
        let pvt = sample_pvt();
        let msg = pvt_to_twist(&pvt, CoordinateFrame::NED);

        assert_eq!(msg.header.frame_id, "gnss_ned");
        // NED passthrough: vel_n=1.5, vel_e=2.0, vel_d=-0.1
        assert_eq!(msg.twist.twist.linear.x, 1.5); // North
        assert_eq!(msg.twist.twist.linear.y, 2.0); // East
        assert!((msg.twist.twist.linear.z - (-0.1)).abs() < 0.001); // Down
    }

    #[test]
    fn test_fix_type_status_conversion() {
        assert_eq!(
            fix_type_to_status(FixType::NoFix),
            sensor_msgs::msg::NavSatStatus::STATUS_NO_FIX
        );
        assert_eq!(
            fix_type_to_status(FixType::Fix3D),
            sensor_msgs::msg::NavSatStatus::STATUS_FIX
        );
        assert_eq!(
            fix_type_to_status(FixType::RtkFixed),
            sensor_msgs::msg::NavSatStatus::STATUS_GBAS_FIX
        );
    }
}

impl GnssIntegrity {
    /// Convert GnssIntegrity to oxide_gnss_msgs/GnssIntegrity message.
    pub fn to_ros_msg(&self, operational: bool) -> oxide_gnss_msgs::msg::GnssIntegrity {
        use oxide_gnss_msgs::msg::GnssIntegrity as Msg;

        let mut msg = Msg::default();

        // Header
        msg.header.stamp = now_timestamp();
        msg.header.frame_id = "gnss".to_string();

        // Integrity level
        msg.level = self.level as u8;
        msg.status_message = self.status_message.clone();
        msg.operational = operational;

        // Position quality
        msg.fix_type = self.fix_type as u8;
        msg.carrier_solution = self.carrier_solution;
        msg.differential_applied = self.differential_applied;
        msg.num_satellites = self.num_satellites;
        msg.h_accuracy_m = self.h_accuracy_m;
        msg.v_accuracy_m = self.v_accuracy_m;
        msg.pdop = self.pdop;

        // Covariance (convert f32 to f64)
        msg.covariance_valid = self.covariance_valid;
        msg.position_covariance = self.position_covariance.map(|v| v as f64);
        msg.velocity_covariance = self.velocity_covariance.map(|v| v as f64);

        // RTK/Correction status
        msg.correction_age_s = self.correction_age_s;
        msg.correction_received = self.correction_received;
        msg.correction_used = self.correction_used;

        // Security status
        msg.jamming_state = match self.jamming_state {
            JammingStateData::Unknown => Msg::JAMMING_UNKNOWN,
            JammingStateData::Ok => Msg::JAMMING_OK,
            JammingStateData::Warning => Msg::JAMMING_WARNING,
            JammingStateData::Critical => Msg::JAMMING_CRITICAL,
        };
        msg.spoofing_state = match self.spoofing_state {
            SpoofingStateData::Unknown => Msg::SPOOFING_UNKNOWN,
            SpoofingStateData::Ok => Msg::SPOOFING_OK,
            SpoofingStateData::Indicated => Msg::SPOOFING_INDICATED,
            SpoofingStateData::Multiple => Msg::SPOOFING_MULTIPLE,
        };
        msg.security_events = self.security_events;
        msg.jamming_indicator = self.jamming_indicator;

        // Hardware status
        msg.antenna_status = match self.antenna_status {
            AntennaStatus::Unknown => Msg::ANTENNA_UNKNOWN,
            AntennaStatus::Ok => Msg::ANTENNA_OK,
            AntennaStatus::Open => Msg::ANTENNA_OPEN,
            AntennaStatus::Short => Msg::ANTENNA_SHORT,
        };

        // Communication status
        msg.comm_ports = self.comm_ports;
        msg.comm_tx_errors = self.comm_tx_errors;

        msg
    }
}
