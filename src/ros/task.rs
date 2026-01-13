//! ROS2 publisher task for the GNSS driver.
//!
//! This module provides an async task that receives messages from the
//! supervisor and publishes them to ROS2 topics.

use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn};

use crate::device::ubx::HpPosData;
use crate::state::{DeviceState, FixType, GnssMessage, NtripState, PvtData};

use super::publishers::GnssPublishers;

/// State of the ROS publisher task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosTaskState {
    /// Task is starting up
    Starting,
    /// Task is running and publishing
    Running,
    /// Task is shutting down
    ShuttingDown,
    /// Task has stopped
    Stopped,
}

impl std::fmt::Display for RosTaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::ShuttingDown => write!(f, "ShuttingDown"),
            Self::Stopped => write!(f, "Stopped"),
        }
    }
}

/// Handle for monitoring the ROS task from outside.
#[derive(Clone)]
pub struct RosTaskHandle {
    state_rx: watch::Receiver<RosTaskState>,
}

impl RosTaskHandle {
    /// Get the current task state.
    pub fn state(&self) -> RosTaskState {
        *self.state_rx.borrow()
    }
}

/// Channels required by the ROS publisher task.
pub struct RosTaskChannels {
    /// Receive messages from device/NTRIP tasks
    pub msg_rx: mpsc::Receiver<GnssMessage>,
    /// Receive shutdown signal
    pub shutdown_rx: watch::Receiver<bool>,
}

/// Configuration for the ROS publisher task.
#[derive(Debug, Clone)]
pub struct RosTaskConfig {
    /// Diagnostics publish rate in Hz
    pub diagnostics_rate_hz: f64,
    /// Integrity/operational publish rate in Hz
    pub integrity_rate_hz: f64,
}

impl Default for RosTaskConfig {
    fn default() -> Self {
        Self {
            diagnostics_rate_hz: 1.0,
            integrity_rate_hz: 1.0,
        }
    }
}

/// The ROS publisher task.
///
/// This task receives messages from the supervisor channels and publishes
/// them to ROS2 topics via the GnssPublishers.
pub struct RosTask {
    publishers: GnssPublishers,
    channels: RosTaskChannels,
    config: RosTaskConfig,
    state_tx: watch::Sender<RosTaskState>,
    // Track state for diagnostics
    last_device_state: DeviceState,
    last_ntrip_state: NtripState,
    last_fix_type: FixType,
    last_pvt: Option<PvtData>,
    /// Last received high-precision position for use in ~/fix
    last_hp_pos: Option<HpPosData>,
    /// Timestamp when last RTCM correction was received
    last_correction_received: Option<Instant>,
    /// Latest integrity data for rate-limited publishing
    last_integrity: Option<crate::state::GnssIntegrity>,
}

impl RosTask {
    /// Create a new ROS task.
    pub fn new(
        publishers: GnssPublishers,
        channels: RosTaskChannels,
        config: RosTaskConfig,
    ) -> (Self, RosTaskHandle) {
        let (state_tx, state_rx) = watch::channel(RosTaskState::Starting);
        let handle = RosTaskHandle { state_rx };

        let task = Self {
            publishers,
            channels,
            config,
            state_tx,
            last_device_state: DeviceState::default(),
            last_ntrip_state: NtripState::default(),
            last_fix_type: FixType::NoFix,
            last_pvt: None,
            last_hp_pos: None,
            last_correction_received: None,
            last_integrity: None,
        };

        (task, handle)
    }

    /// Run the ROS publisher task.
    ///
    /// This method runs until shutdown is signaled. It receives messages
    /// from the supervisor and publishes them to ROS2 topics.
    pub async fn run(mut self) {
        info!("ROS publisher task starting");
        let _ = self.state_tx.send(RosTaskState::Running);

        let diagnostics_interval = Duration::from_secs_f64(1.0 / self.config.diagnostics_rate_hz);
        let mut diagnostics_timer = tokio::time::interval(diagnostics_interval);

        let integrity_interval = Duration::from_secs_f64(1.0 / self.config.integrity_rate_hz);
        let mut integrity_timer = tokio::time::interval(integrity_interval);

        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = self.channels.shutdown_rx.changed() => {
                    if *self.channels.shutdown_rx.borrow() {
                        info!("ROS task received shutdown signal");
                        break;
                    }
                }

                // Receive messages from device/NTRIP tasks
                msg = self.channels.msg_rx.recv() => {
                    match msg {
                        Some(message) => self.handle_message(message),
                        None => {
                            warn!("Message channel closed, stopping ROS task");
                            break;
                        }
                    }
                }

                // Periodic diagnostics publishing
                _ = diagnostics_timer.tick() => {
                    self.publish_diagnostics();
                }

                // Periodic integrity publishing (rate-limited)
                _ = integrity_timer.tick() => {
                    self.publish_integrity();
                }
            }
        }

        let _ = self.state_tx.send(RosTaskState::ShuttingDown);
        info!("ROS publisher task shutting down");

        let _ = self.state_tx.send(RosTaskState::Stopped);
        info!("ROS publisher task stopped");
    }

    /// Handle an incoming message.
    fn handle_message(&mut self, msg: GnssMessage) {
        match msg {
            GnssMessage::Pvt(pvt) => {
                debug!("Publishing PVT data");
                self.last_fix_type = pvt.fix_type;
                self.publishers.publish_pvt(&pvt, self.last_hp_pos.as_ref());
                self.last_pvt = Some(pvt);
            }
            GnssMessage::SecSig(_sig) => {
                // SEC_SIG data is used internally for integrity calculation
                // No separate topic - integrity status published via ~/integrity
            }
            GnssMessage::DeviceStateChanged(state) => {
                info!(state = %state, "Device state changed");
                self.last_device_state = state;
            }
            GnssMessage::NtripStateChanged(state) => {
                info!(state = %state, "NTRIP state changed");
                self.last_ntrip_state = state;
            }
            GnssMessage::RtcmReceived { bytes } => {
                debug!(bytes = bytes, "RTCM data received");
                // Record when correction was received for age calculation
                self.last_correction_received = Some(Instant::now());
            }
            GnssMessage::HpPos(hp) => {
                // Store HP position for use in publish_pvt (enhances ~/fix)
                self.last_hp_pos = Some(hp);
            }
            GnssMessage::SatInfo(sat) => {
                self.publishers.publish_sat_info(&sat);
            }
            GnssMessage::Integrity(integrity) => {
                // Store for rate-limited publishing
                self.last_integrity = Some(integrity);
            }
            GnssMessage::RelPosNed(rel_pos) => {
                self.publishers.publish_baseline_pose(&rel_pos);
            }
            GnssMessage::Shutdown => {
                info!("Received shutdown message");
            }
        }
    }

    /// Publish integrity status (rate-limited).
    fn publish_integrity(&self) {
        if let Some(ref integrity) = self.last_integrity {
            self.publishers.publish_integrity(integrity);
        }
    }

    /// Publish diagnostics.
    fn publish_diagnostics(&self) {
        // Calculate correction age from timestamp of last received correction
        let correction_age = self
            .last_correction_received
            .map(|t| t.elapsed().as_secs_f64());

        self.publishers.publish_diagnostics(
            &self.last_device_state,
            &self.last_ntrip_state,
            self.last_fix_type,
            self.last_pvt.as_ref().map(|p| p.num_sv),
            self.last_pvt.as_ref().map(|p| p.p_dop / 100.0),
            correction_age,
        );
    }
}

/// Spawn the ROS publisher task.
///
/// Returns a handle that can be used to monitor the task's state.
pub fn spawn_ros_task(
    publishers: GnssPublishers,
    channels: RosTaskChannels,
    config: RosTaskConfig,
) -> RosTaskHandle {
    let (task, handle) = RosTask::new(publishers, channels, config);

    tokio::spawn(async move {
        task.run().await;
    });

    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ros_task_state_display() {
        assert_eq!(RosTaskState::Starting.to_string(), "Starting");
        assert_eq!(RosTaskState::Running.to_string(), "Running");
        assert_eq!(RosTaskState::ShuttingDown.to_string(), "ShuttingDown");
        assert_eq!(RosTaskState::Stopped.to_string(), "Stopped");
    }

    #[test]
    fn test_ros_task_config_default() {
        let config = RosTaskConfig::default();
        assert!((config.diagnostics_rate_hz - 1.0).abs() < 0.001);
    }
}
