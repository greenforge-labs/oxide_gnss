//! ROS2 topic publishers for GNSS data.

use rclrs::{IntoPrimitiveOptions, Node, Publisher, QoSProfile};
use tracing::{debug, error};

use crate::config::CoordinateFrame;
use crate::device::ubx::{HpPosData, PvtData, RelPosNedData, SatInfo};
use crate::state::{DeviceState, FixType, GnssIntegrity, IntegrityLevel, NtripState};

use super::conversions::{now_timestamp, pvt_to_twist, ToRosMessage};

/// Collection of ROS2 publishers for GNSS data.
#[derive(Clone)]
pub struct GnssPublishers {
    /// NavSatFix publisher (~/fix) - always enabled
    fix_pub: Publisher<sensor_msgs::msg::NavSatFix>,
    /// TwistWithCovarianceStamped publisher (~/velocity) - always enabled
    velocity_pub: Publisher<geometry_msgs::msg::TwistWithCovarianceStamped>,
    /// TimeReference publisher (~/time_reference) - always enabled
    time_ref_pub: Publisher<sensor_msgs::msg::TimeReference>,
    /// DiagnosticArray publisher (/diagnostics) - always enabled
    diagnostics_pub: Publisher<diagnostic_msgs::msg::DiagnosticArray>,
    /// Node name for diagnostics
    node_name: String,
    /// Coordinate frame for velocity output
    frame: CoordinateFrame,

    // Optional publishers - only created if topic is enabled
    /// Satellite Info publisher (~/satellites) - requires satellites feature
    sat_pub: Option<Publisher<std_msgs::msg::String>>,
    /// Integrity status publisher (~/integrity) - requires integrity feature
    integrity_pub: Option<Publisher<oxide_gnss_msgs::msg::OxideIntegrity>>,
    /// Operational go/no-go publisher (~/operational) - requires integrity feature
    operational_pub: Option<Publisher<std_msgs::msg::Bool>>,
    /// Baseline pose publisher for moving base/rover (~/baseline_pose) - moving_base_rover only
    baseline_pose_pub: Option<Publisher<geometry_msgs::msg::PoseWithCovarianceStamped>>,

    /// Use high-precision position data for ~/fix when available
    use_hp_for_fix: bool,
}

impl GnssPublishers {
    /// Create publishers on the given node.
    ///
    /// Only creates publishers for topics that are in `enabled_topics`.
    /// Core topics (fix, velocity, time_reference, diagnostics) are always created.
    ///
    /// # Arguments
    /// * `node` - The ROS2 node to create publishers on
    /// * `frame` - Coordinate frame for velocity output (ENU or NED)
    /// * `use_hp_for_fix` - Use high-precision position for ~/fix when available
    /// * `enabled_topics` - List of enabled topic names (from Config::enabled_topics())
    pub fn new(
        node: &Node,
        frame: CoordinateFrame,
        use_hp_for_fix: bool,
        enabled_topics: &[String],
    ) -> Result<Self, rclrs::RclrsError> {
        use tracing::info;

        // QoS profiles for different topic types:
        // - Sensor data: Best effort, keep last (high-rate position/velocity)
        // - Reliable: For safety-critical topics that must not be lost
        let sensor_qos = QoSProfile::sensor_data_default();
        let reliable_qos = QoSProfile::topics_default().reliable();

        // Helper to check if a topic is enabled
        let is_enabled = |topic: &str| enabled_topics.iter().any(|t| t == topic);

        // Core topics - always created
        let fix_pub = node.create_publisher("~/fix".qos(sensor_qos))?;
        let velocity_pub = node.create_publisher("~/velocity".qos(sensor_qos))?;
        let time_ref_pub = node.create_publisher("~/time_reference")?;
        let diagnostics_pub = node.create_publisher("/diagnostics")?;

        // Optional topics - only created if enabled
        let sat_pub = if is_enabled("~/satellites") {
            info!("Creating ~/satellites publisher");
            Some(node.create_publisher("~/satellites")?)
        } else {
            None
        };

        let integrity_pub = if is_enabled("~/integrity") {
            info!("Creating ~/integrity publisher");
            Some(node.create_publisher("~/integrity".qos(reliable_qos))?)
        } else {
            None
        };

        let operational_pub = if is_enabled("~/operational") {
            info!("Creating ~/operational publisher");
            Some(node.create_publisher("~/operational".qos(reliable_qos))?)
        } else {
            None
        };

        let baseline_pose_pub = if is_enabled("~/baseline_pose") {
            info!("Creating ~/baseline_pose publisher");
            Some(node.create_publisher("~/baseline_pose".qos(sensor_qos))?)
        } else {
            None
        };

        Ok(Self {
            fix_pub,
            velocity_pub,
            time_ref_pub,
            diagnostics_pub,
            node_name: node.name().to_string(),
            frame,
            sat_pub,
            integrity_pub,
            operational_pub,
            baseline_pose_pub,
            use_hp_for_fix,
        })
    }

    /// Publish baseline pose from NAV-RELPOSNED for moving base/rover.
    ///
    /// Converts the relative position (N/E/D baseline vector) and heading
    /// to a PoseWithCovarianceStamped message.
    ///
    /// Only publishes if the ~/baseline_pose topic is enabled.
    pub fn publish_baseline_pose(&self, rel_pos: &RelPosNedData) {
        let Some(ref pub_) = self.baseline_pose_pub else {
            return; // Topic not enabled
        };

        // Only publish if relative position is valid
        if !rel_pos.flags.rel_pos_valid {
            debug!("Skipping baseline_pose publish: rel_pos not valid");
            return;
        }

        let stamp = now_timestamp();

        // Build pose message
        let mut msg = geometry_msgs::msg::PoseWithCovarianceStamped::default();
        msg.header.stamp = stamp;
        msg.header.frame_id = "gnss_base".to_string();

        // Position: baseline vector in meters (NED frame)
        // Convert to ENU for ROS convention: x=East, y=North, z=Up
        msg.pose.pose.position.x = rel_pos.rel_pos_e;
        msg.pose.pose.position.y = rel_pos.rel_pos_n;
        msg.pose.pose.position.z = -rel_pos.rel_pos_d;

        // Orientation: heading from baseline
        // Heading is valid when baseline length is > 0 and we have a good solution
        let heading_valid = rel_pos.rel_pos_length > 0.1 && rel_pos.flags.carr_soln > 0;
        if heading_valid {
            // Convert heading (radians, from North) to quaternion
            // Heading is clockwise from North, we need to convert to ENU yaw (CCW from East)
            let yaw = std::f64::consts::FRAC_PI_2 - rel_pos.rel_pos_heading;
            let (sin_half, cos_half) = (yaw / 2.0).sin_cos();
            msg.pose.pose.orientation.x = 0.0;
            msg.pose.pose.orientation.y = 0.0;
            msg.pose.pose.orientation.z = sin_half;
            msg.pose.pose.orientation.w = cos_half;
        } else {
            // Identity quaternion if heading not valid
            msg.pose.pose.orientation.w = 1.0;
        }

        // Covariance (6x6 row-major: x, y, z, roll, pitch, yaw)
        // Position covariance from accuracy estimates (variance = acc^2)
        let var_e = rel_pos.acc_e * rel_pos.acc_e;
        let var_n = rel_pos.acc_n * rel_pos.acc_n;
        let var_d = rel_pos.acc_d * rel_pos.acc_d;
        let var_yaw = if heading_valid {
            rel_pos.acc_heading * rel_pos.acc_heading
        } else {
            999.0 // Large variance if heading not valid
        };

        // Set diagonal elements (row-major 6x6)
        msg.pose.covariance[0] = var_e; // x (East)
        msg.pose.covariance[7] = var_n; // y (North)
        msg.pose.covariance[14] = var_d; // z (Up)
        msg.pose.covariance[21] = 999.0; // roll (unknown)
        msg.pose.covariance[28] = 999.0; // pitch (unknown)
        msg.pose.covariance[35] = var_yaw; // yaw

        if let Err(e) = pub_.publish(msg) {
            error!(error = %e, "Failed to publish baseline_pose");
        } else {
            debug!(
                length = rel_pos.rel_pos_length,
                heading_valid = heading_valid,
                carr_soln = rel_pos.flags.carr_soln,
                "Published baseline_pose"
            );
        }
    }

    /// Publish PVT data to all relevant topics.
    ///
    /// If `hp_pos` is provided and `use_hp_for_fix` is enabled, the ~/fix topic
    /// will use the high-precision position (lat/lon/alt and accuracy) from
    /// NAV-HPPOSLLH instead of standard NAV-PVT coordinates.
    pub fn publish_pvt(&self, pvt: &PvtData, hp_pos: Option<&HpPosData>) {
        let stamp = now_timestamp();

        // Publish NavSatFix - use HP position if available and configured
        let mut fix_msg: sensor_msgs::msg::NavSatFix = pvt.to_ros_msg();
        if self.use_hp_for_fix {
            if let Some(hp) = hp_pos {
                // Override position with high-precision data
                fix_msg.latitude = hp.lat;
                fix_msg.longitude = hp.lon;
                fix_msg.altitude = hp.height;
                // Override covariance with HP accuracy
                let h_var = (hp.h_acc as f64).powi(2);
                let v_var = (hp.v_acc as f64).powi(2);
                fix_msg.position_covariance = [
                    h_var, 0.0, 0.0, // East
                    0.0, h_var, 0.0, // North
                    0.0, 0.0, v_var, // Up
                ];
                debug!("Published NavSatFix with HP position");
            }
        }
        fix_msg.header.stamp = stamp.clone();
        if let Err(e) = self.fix_pub.publish(fix_msg) {
            error!(error = %e, "Failed to publish NavSatFix");
        } else if !self.use_hp_for_fix || hp_pos.is_none() {
            debug!("Published NavSatFix");
        }

        // Publish velocity with configured coordinate frame
        let mut vel_msg = pvt_to_twist(pvt, self.frame);
        vel_msg.header.stamp = stamp.clone();
        if let Err(e) = self.velocity_pub.publish(vel_msg) {
            error!(error = %e, "Failed to publish velocity");
        }

        // Publish time reference
        let mut time_msg: sensor_msgs::msg::TimeReference = pvt.to_ros_msg();
        time_msg.header.stamp = stamp;
        if let Err(e) = self.time_ref_pub.publish(time_msg) {
            error!(error = %e, "Failed to publish TimeReference");
        }
    }

    /// Publish Satellite Info (only if ~/satellites topic is enabled)
    pub fn publish_sat_info(&self, info: &SatInfo) {
        let Some(ref pub_) = self.sat_pub else {
            return; // Topic not enabled
        };

        let msg = std_msgs::msg::String {
            data: format!(
                "{{\"num_svs\": {}, \"active_svs\": {}}}",
                info.num_sats,
                info.sats.iter().filter(|s| (s.flags & 0x1) == 0x1).count()
            ),
        };
        let _ = pub_.publish(msg);
    }

    /// Publish integrity status to ~/integrity and ~/operational (Bool).
    /// Only publishes if the integrity topics are enabled.
    pub fn publish_integrity(&self, integrity: &GnssIntegrity) {
        // Calculate operational status
        let operational = matches!(
            integrity.level,
            IntegrityLevel::Ok | IntegrityLevel::Degraded
        );

        // Publish operational status if topic is enabled
        if let Some(ref pub_) = self.operational_pub {
            let op_msg = std_msgs::msg::Bool { data: operational };
            if let Err(e) = pub_.publish(op_msg) {
                error!(error = %e, "Failed to publish operational status");
            }
        }

        // Publish integrity as typed message if topic is enabled
        if let Some(ref pub_) = self.integrity_pub {
            let msg = integrity.to_ros_msg(operational);
            if let Err(e) = pub_.publish(msg) {
                error!(error = %e, "Failed to publish integrity");
            }
        }
    }

    /// Publish diagnostics.
    pub fn publish_diagnostics(
        &self,
        device_state: &DeviceState,
        ntrip_state: &NtripState,
        fix_type: FixType,
        num_satellites: Option<u8>,
        hdop: Option<f32>,
        correction_age_secs: Option<f64>,
    ) {
        let mut diag = diagnostic_msgs::msg::DiagnosticArray::default();
        diag.header.stamp = now_timestamp();

        // Device status
        let mut device_status = diagnostic_msgs::msg::DiagnosticStatus {
            name: format!("{}: Device", self.node_name),
            hardware_id: "gnss_receiver".to_string(),
            level: state_to_diagnostic_level(device_state),
            message: device_state.to_string(),
            ..Default::default()
        };

        device_status.values.push(diagnostic_msgs::msg::KeyValue {
            key: "state".to_string(),
            value: device_state.name().to_string(),
        });

        // NTRIP status
        let mut ntrip_status = diagnostic_msgs::msg::DiagnosticStatus {
            name: format!("{}: NTRIP", self.node_name),
            hardware_id: "ntrip_client".to_string(),
            level: ntrip_state_to_level(ntrip_state),
            message: ntrip_state.to_string(),
            ..Default::default()
        };

        ntrip_status.values.push(diagnostic_msgs::msg::KeyValue {
            key: "state".to_string(),
            value: ntrip_state.name().to_string(),
        });

        // Add correction age if available
        if let Some(age) = correction_age_secs {
            ntrip_status.values.push(diagnostic_msgs::msg::KeyValue {
                key: "correction_age".to_string(),
                value: format!("{:.2} sec", age),
            });
        }

        // Fix status
        let mut fix_status = diagnostic_msgs::msg::DiagnosticStatus {
            name: format!("{}: Fix", self.node_name),
            hardware_id: "gnss_receiver".to_string(),
            level: fix_type_to_level(fix_type),
            message: fix_type.to_string(),
            ..Default::default()
        };

        fix_status.values.push(diagnostic_msgs::msg::KeyValue {
            key: "fix_type".to_string(),
            value: fix_type.to_string(),
        });

        if let Some(sats) = num_satellites {
            fix_status.values.push(diagnostic_msgs::msg::KeyValue {
                key: "satellites".to_string(),
                value: sats.to_string(),
            });
        }

        if let Some(h) = hdop {
            fix_status.values.push(diagnostic_msgs::msg::KeyValue {
                key: "hdop".to_string(),
                value: format!("{:.2}", h),
            });
        }

        diag.status.push(device_status);
        diag.status.push(ntrip_status);
        diag.status.push(fix_status);

        if let Err(e) = self.diagnostics_pub.publish(diag) {
            error!(error = %e, "Failed to publish diagnostics");
        }
    }
}

/// Convert DeviceState to diagnostic level.
fn state_to_diagnostic_level(state: &DeviceState) -> u8 {
    match state.diagnostic_level() {
        crate::state::DiagnosticLevel::Ok => diagnostic_msgs::msg::DiagnosticStatus::OK,
        crate::state::DiagnosticLevel::Warn => diagnostic_msgs::msg::DiagnosticStatus::WARN,
        crate::state::DiagnosticLevel::Error => diagnostic_msgs::msg::DiagnosticStatus::ERROR,
        crate::state::DiagnosticLevel::Stale => diagnostic_msgs::msg::DiagnosticStatus::STALE,
    }
}

/// Convert NtripState to diagnostic level.
fn ntrip_state_to_level(state: &NtripState) -> u8 {
    match state.diagnostic_level() {
        crate::state::DiagnosticLevel::Ok => diagnostic_msgs::msg::DiagnosticStatus::OK,
        crate::state::DiagnosticLevel::Warn => diagnostic_msgs::msg::DiagnosticStatus::WARN,
        crate::state::DiagnosticLevel::Error => diagnostic_msgs::msg::DiagnosticStatus::ERROR,
        crate::state::DiagnosticLevel::Stale => diagnostic_msgs::msg::DiagnosticStatus::STALE,
    }
}

/// Convert FixType to diagnostic level.
fn fix_type_to_level(fix: FixType) -> u8 {
    match fix.diagnostic_level() {
        crate::state::DiagnosticLevel::Ok => diagnostic_msgs::msg::DiagnosticStatus::OK,
        crate::state::DiagnosticLevel::Warn => diagnostic_msgs::msg::DiagnosticStatus::WARN,
        crate::state::DiagnosticLevel::Error => diagnostic_msgs::msg::DiagnosticStatus::ERROR,
        crate::state::DiagnosticLevel::Stale => diagnostic_msgs::msg::DiagnosticStatus::STALE,
    }
}
