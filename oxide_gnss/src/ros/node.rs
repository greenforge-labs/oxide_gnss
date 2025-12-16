//! Main ROS2 node for oxide_gnss.

use rclrs::{Context, Node, RclrsError};
use tracing::info;

use crate::config::{DeviceConfig, NtripConfig};
use crate::device::{DeviceMessage, PvtData};
use crate::ntrip::NtripMessage;
use crate::state::{FixType, Supervisor};

use super::publishers::GnssPublishers;

/// Configuration for the GNSS ROS2 node.
#[derive(Debug, Clone)]
pub struct GnssNodeConfig {
    /// Node name
    pub node_name: String,
    /// Node namespace (e.g., "gnss")
    pub namespace: String,
    /// Device configuration
    pub device: DeviceConfig,
    /// NTRIP configuration (optional)
    pub ntrip: Option<NtripConfig>,
    /// Diagnostics publish rate in Hz
    pub diagnostics_rate_hz: f64,

    /// Publish detailed SEC-SIG per-center-frequency data on ~/sec_sig_details.
    pub publish_sec_sig_details: bool,
}

impl Default for GnssNodeConfig {
    fn default() -> Self {
        Self {
            node_name: "oxide_gnss".to_string(),
            namespace: "".to_string(),
            device: DeviceConfig {
                port: "/dev/ttyACM0".to_string(),
                baud_rate: 460800,
                frame: crate::config::CoordinateFrame::ENU,
                navigation: Default::default(),
                reconnect: Default::default(),
                ublox: None,
            },
            ntrip: None,
            diagnostics_rate_hz: 1.0,
            publish_sec_sig_details: false,
        }
    }
}

/// The main GNSS ROS2 node.
pub struct GnssNode {
    /// ROS2 node handle
    node: Node,
    /// Publishers
    publishers: GnssPublishers,
    /// Supervisor for device and NTRIP tasks
    supervisor: Supervisor,
    /// Configuration
    config: GnssNodeConfig,
    /// Last known PVT data
    last_pvt: Option<PvtData>,
    /// Last known fix type
    last_fix_type: FixType,
}

impl GnssNode {
    /// Create a new GNSS driver instance using an existing ROS2 node.
    pub fn new(node: Node, config: GnssNodeConfig) -> Result<Self, RclrsError> {
        let publishers =
            GnssPublishers::new(&node, config.device.frame, config.publish_sec_sig_details)?;

        Ok(Self {
            node,
            publishers,
            supervisor: Supervisor::new(),
            config,
            last_pvt: None,
            last_fix_type: FixType::NoFix,
        })
    }

    /// Get the ROS2 node handle.
    pub fn node(&self) -> &Node {
        &self.node
    }

    /// Get the supervisor handle.
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    /// Get a mutable reference to the supervisor.
    pub fn supervisor_mut(&mut self) -> &mut Supervisor {
        &mut self.supervisor
    }

    /// Handle a device message.
    pub fn handle_device_message(&mut self, msg: DeviceMessage) {
        match msg {
            DeviceMessage::Pvt(pvt) => {
                self.last_fix_type = pvt.fix_type;
                self.publishers.publish_pvt(&pvt);
                self.last_pvt = Some(pvt);
            }
            DeviceMessage::StateChanged(state) => {
                info!(state = %state, "Device state changed");
            }
            DeviceMessage::FixTypeChanged(fix) => {
                self.last_fix_type = fix;
                info!(fix = %fix, "Fix type changed");
            }
            DeviceMessage::HpPos(hp) => {
                self.publishers.publish_hp_pos(&hp);
            }
            DeviceMessage::SatInfo(sat) => {
                self.publishers.publish_sat_info(&sat);
            }
            DeviceMessage::Integrity(integrity) => {
                tracing::debug!(
                    level = ?integrity.level,
                    "Integrity update: {}",
                    integrity.status_message
                );
                self.publishers.publish_integrity(&integrity);
            }
            DeviceMessage::Covariance(_cov) => {
                // Covariance data is aggregated into integrity
            }
            DeviceMessage::PosEcef(_pos) => {
                // ECEF position available for coordinate transforms
            }
            DeviceMessage::SecSig(sig) => {
                self.publishers.publish_sec_sig_details(&sig);
                tracing::debug!(
                    jamming = ?sig.jamming_state,
                    spoofing = ?sig.spoofing_state,
                    "Security signal status"
                );
            }
            DeviceMessage::SecSiglog(siglog) => {
                if siglog.num_events > 0 {
                    tracing::warn!(events = siglog.num_events, "Security events logged");
                }
            }
            DeviceMessage::RxmCor(cor) => {
                tracing::trace!(
                    msg_used = ?cor.msg_used,
                    msg_type = cor.msg_type,
                    "Correction status"
                );
            }
            DeviceMessage::MonComms(_comms) => {
                // Communication port status available for diagnostics
            }
            DeviceMessage::MonHw(hw) => {
                tracing::debug!(
                    antenna = ?hw.antenna_status,
                    jam_ind = hw.jam_ind,
                    "Hardware status (deprecated)"
                );
            }
            DeviceMessage::MonRf(rf) => {
                tracing::debug!(
                    antenna = ?rf.antenna_status,
                    jam_ind = rf.jam_ind,
                    "RF status"
                );
            }
        }
    }

    /// Handle an NTRIP message.
    pub fn handle_ntrip_message(&mut self, msg: NtripMessage) {
        match msg {
            NtripMessage::StateChanged(state) => {
                info!(state = %state, "NTRIP state changed");
            }
            NtripMessage::Connected => {
                info!("NTRIP connected");
            }
            NtripMessage::Disconnected { reason } => {
                info!(reason = %reason, "NTRIP disconnected");
            }
            NtripMessage::RtcmReceived { bytes } => {
                tracing::trace!(bytes = bytes, "RTCM received");
            }
        }
    }

    /// Publish current diagnostics.
    pub async fn publish_diagnostics(&self) {
        let state = self.supervisor.handle().state_snapshot().await;

        self.publishers.publish_diagnostics(
            &state.device_state,
            &state.ntrip_state,
            self.last_fix_type,
            self.last_pvt.as_ref().map(|p| p.num_sv),
            self.last_pvt.as_ref().map(|p| p.p_dop / 100.0), // PDOP, approximate HDOP
            None, // No correction age available in this context
        );
    }

    /// Get the node configuration.
    pub fn config(&self) -> &GnssNodeConfig {
        &self.config
    }

    /// Get a reference to the publishers.
    pub fn publishers(&self) -> &GnssPublishers {
        &self.publishers
    }
}

/// Create a ROS2 context from environment.
pub fn create_context() -> Result<Context, RclrsError> {
    Context::default_from_env()
}
