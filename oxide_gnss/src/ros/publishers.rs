//! ROS2 topic publishers for GNSS data.

use rclrs::{IntoPrimitiveOptions, Node, Publisher, QoSProfile};
use tracing::{debug, error};

use crate::config::CoordinateFrame;
use crate::device::ubx::{HpPosData, PvtData, SatInfo, SecSigData};
use crate::state::{DeviceState, FixType, GnssIntegrity, IntegrityLevel, NtripState};

use super::conversions::{now_timestamp, pvt_to_twist, ToRosMessage};

/// Collection of ROS2 publishers for GNSS data.
#[derive(Clone)]
pub struct GnssPublishers {
    /// NavSatFix publisher (~/fix)
    fix_pub: Publisher<sensor_msgs::msg::NavSatFix>,
    /// TwistWithCovarianceStamped publisher (~/velocity)
    velocity_pub: Publisher<geometry_msgs::msg::TwistWithCovarianceStamped>,
    /// TimeReference publisher (~/time_reference)
    time_ref_pub: Publisher<sensor_msgs::msg::TimeReference>,
    /// DiagnosticArray publisher (/diagnostics)
    diagnostics_pub: Publisher<diagnostic_msgs::msg::DiagnosticArray>,
    /// Node name for diagnostics
    node_name: String,
    /// Coordinate frame for velocity output
    frame: CoordinateFrame,
    /// High Precision Position publisher (~/hp_pos)
    hp_pos_pub: Publisher<sensor_msgs::msg::NavSatFix>,
    /// Satellite Info publisher (~/satellites)
    sat_pub: Publisher<std_msgs::msg::String>,
    /// Integrity status publisher (~/integrity) - typed message
    integrity_pub: Publisher<oxide_gnss_msgs::msg::OxideIntegrity>,
    /// Operational go/no-go publisher (~/operational)
    operational_pub: Publisher<std_msgs::msg::Bool>,

    /// Detailed SEC-SIG per-center-frequency publisher (~/sec_sig_details)
    sec_sig_details_pub: Option<Publisher<oxide_gnss_msgs::msg::SecSigDetails>>,
}

impl GnssPublishers {
    /// Create publishers on the given node.
    ///
    /// # Arguments
    /// * `node` - The ROS2 node to create publishers on
    /// * `frame` - Coordinate frame for velocity output (ENU or NED)
    pub fn new(
        node: &Node,
        frame: CoordinateFrame,
        publish_sec_sig_details: bool,
    ) -> Result<Self, rclrs::RclrsError> {
        // QoS profiles for different topic types:
        // - Sensor data: Best effort, keep last (high-rate position/velocity)
        // - Reliable: For safety-critical topics that must not be lost
        let sensor_qos = QoSProfile::sensor_data_default();
        let reliable_qos = QoSProfile::topics_default().reliable();

        // High-rate sensor data topics - best effort, latest sample matters
        let fix_pub = node.create_publisher("~/fix".qos(sensor_qos))?;
        let velocity_pub = node.create_publisher("~/velocity".qos(sensor_qos))?;
        let hp_pos_pub = node
            .create_publisher("~/hp_pos".qos(sensor_qos))
            .expect("Failed to create hp_pos publisher");

        // Standard topics
        let time_ref_pub = node.create_publisher("~/time_reference")?;
        let diagnostics_pub = node.create_publisher("/diagnostics")?;
        let sat_pub = node
            .create_publisher("~/satellites")
            .expect("Failed to create satellites publisher");

        // Safety-critical topics - reliable delivery
        let integrity_pub = node
            .create_publisher("~/integrity".qos(reliable_qos))
            .expect("Failed to create integrity publisher");
        let operational_pub = node
            .create_publisher("~/operational".qos(reliable_qos))
            .expect("Failed to create operational publisher");

        let sec_sig_details_pub = if publish_sec_sig_details {
            Some(
                node.create_publisher("~/sec_sig_details".qos(reliable_qos))
                    .expect("Failed to create sec_sig_details publisher"),
            )
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
            hp_pos_pub,
            sat_pub,
            integrity_pub,
            operational_pub,
            sec_sig_details_pub,
        })
    }

    /// Publish detailed SEC-SIG per-center-frequency data to ~/sec_sig_details (if enabled).
    pub fn publish_sec_sig_details(&self, sig: &SecSigData) {
        let Some(pub_) = &self.sec_sig_details_pub else {
            return;
        };

        let mut msg: oxide_gnss_msgs::msg::SecSigDetails = sig.to_ros_msg();
        msg.header.stamp = now_timestamp();
        msg.header.frame_id = "gnss".to_string();

        if let Err(e) = pub_.publish(msg) {
            error!(error = %e, "Failed to publish SecSigDetails");
        }
    }

    /// Publish PVT data to all relevant topics.
    pub fn publish_pvt(&self, pvt: &PvtData) {
        let stamp = now_timestamp();

        // Publish NavSatFix
        let mut fix_msg: sensor_msgs::msg::NavSatFix = pvt.to_ros_msg();
        fix_msg.header.stamp = stamp.clone();
        if let Err(e) = self.fix_pub.publish(fix_msg) {
            error!(error = %e, "Failed to publish NavSatFix");
        } else {
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

    /// Publish HP Position
    pub fn publish_hp_pos(&self, data: &HpPosData) {
        let stamp = now_timestamp();
        let mut msg: sensor_msgs::msg::NavSatFix = data.to_ros_msg();
        msg.header.stamp = stamp;
        if let Err(e) = self.hp_pos_pub.publish(msg) {
            error!(error = %e, "Failed to publish HP Position");
        }
    }

    /// Publish Satellite Info
    pub fn publish_sat_info(&self, info: &SatInfo) {
        let msg = std_msgs::msg::String {
            data: format!(
                "{{\"num_svs\": {}, \"active_svs\": {}}}",
                info.num_sats,
                info.sats.iter().filter(|s| (s.flags & 0x1) == 0x1).count() // Example flag check for 'used'
            ),
        };
        let _ = self.sat_pub.publish(msg);
    }

    /// Publish integrity status to ~/integrity and ~/operational (Bool).
    pub fn publish_integrity(&self, integrity: &GnssIntegrity) {
        // Publish operational status (true if Ok or Degraded)
        let operational = matches!(
            integrity.level,
            IntegrityLevel::Ok | IntegrityLevel::Degraded
        );
        let op_msg = std_msgs::msg::Bool { data: operational };
        if let Err(e) = self.operational_pub.publish(op_msg) {
            error!(error = %e, "Failed to publish operational status");
        }

        // Publish integrity as typed message
        let msg = integrity.to_ros_msg(operational);
        if let Err(e) = self.integrity_pub.publish(msg) {
            error!(error = %e, "Failed to publish integrity");
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
