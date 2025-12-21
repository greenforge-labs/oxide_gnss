//! Supervisor for coordinating device and NTRIP tasks.

use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};

use super::{DeviceState, DiagnosticLevel, FixType, NtripState};

// Re-export GgaData and PvtData from device module for convenience
pub use crate::device::{GgaData, PvtData};

/// Messages that can be sent between tasks.
#[derive(Debug, Clone)]
pub enum GnssMessage {
    /// Full PVT data from device (for ROS publishing)
    Pvt(PvtData),
    /// High-precision position data
    HpPos(crate::device::ubx::HpPosData),
    /// Satellite info
    SatInfo(crate::device::ubx::SatInfo),
    /// Security signal status (SEC-SIG)
    SecSig(crate::device::ubx::SecSigData),
    /// Relative position for moving base/rover (NAV-RELPOSNED)
    RelPosNed(crate::device::ubx::RelPosNedData),
    /// Integrity status
    Integrity(crate::state::GnssIntegrity),
    /// Device state changed
    DeviceStateChanged(DeviceState),
    /// NTRIP state changed
    NtripStateChanged(NtripState),
    /// RTCM data received (bytes count)
    RtcmReceived { bytes: usize },
    /// Shutdown requested
    Shutdown,
}

/// Channel senders/receivers for inter-task communication.
pub struct SupervisorChannels {
    /// RTCM data from NTRIP to device (sender side)
    pub rtcm_tx: mpsc::Sender<Vec<u8>>,
    /// RTCM data from NTRIP to device (receiver side, taken once by device task)
    rtcm_rx: Option<mpsc::Receiver<Vec<u8>>>,

    /// GGA position from device to NTRIP (sender side)
    pub gga_tx: watch::Sender<Option<GgaData>>,
    /// GGA position from device to NTRIP (receiver side)
    pub gga_rx: watch::Receiver<Option<GgaData>>,

    /// Messages from device/NTRIP to ROS publisher (sender side)
    pub msg_tx: mpsc::Sender<GnssMessage>,
    /// Messages from device/NTRIP to ROS publisher (receiver side, taken once by ROS task)
    msg_rx: Option<mpsc::Receiver<GnssMessage>>,

    /// Shutdown signal (sender side)
    pub shutdown_tx: watch::Sender<bool>,
    /// Shutdown signal (receiver side)
    pub shutdown_rx: watch::Receiver<bool>,
}

impl SupervisorChannels {
    /// Create a new set of channels with specified buffer sizes.
    pub fn new(rtcm_buffer: usize, msg_buffer: usize) -> Self {
        let (rtcm_tx, rtcm_rx) = mpsc::channel(rtcm_buffer);
        let (gga_tx, gga_rx) = watch::channel(None);
        let (msg_tx, msg_rx) = mpsc::channel(msg_buffer);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Self {
            rtcm_tx,
            rtcm_rx: Some(rtcm_rx),
            gga_tx,
            gga_rx,
            msg_tx,
            msg_rx: Some(msg_rx),
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Create channels with default buffer sizes.
    pub fn with_defaults() -> Self {
        Self::new(32, 64)
    }
}

/// Shared state for the supervisor.
#[derive(Debug, Default)]
pub struct SupervisorState {
    /// Current device state
    pub device_state: DeviceState,
    /// Current NTRIP state
    pub ntrip_state: NtripState,
    /// Current fix type
    pub fix_type: FixType,
}

impl SupervisorState {
    /// Get the aggregate diagnostic level.
    pub fn diagnostic_level(&self) -> DiagnosticLevel {
        super::aggregate_diagnostic_level(&self.device_state, &self.ntrip_state, self.fix_type)
    }
}

/// Handle for interacting with the supervisor from outside.
#[derive(Clone)]
pub struct SupervisorHandle {
    /// Shared state
    state: Arc<Mutex<SupervisorState>>,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
}

impl SupervisorHandle {
    /// Request shutdown of all tasks.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Get the current diagnostic level.
    pub async fn diagnostic_level(&self) -> DiagnosticLevel {
        self.state.lock().await.diagnostic_level()
    }

    /// Get a snapshot of the current state.
    pub async fn state_snapshot(&self) -> SupervisorState {
        let state = self.state.lock().await;
        SupervisorState {
            device_state: state.device_state.clone(),
            ntrip_state: state.ntrip_state.clone(),
            fix_type: state.fix_type,
        }
    }
}

/// The supervisor coordinates device and NTRIP tasks.
pub struct Supervisor {
    /// Shared state
    state: Arc<Mutex<SupervisorState>>,
    /// Communication channels
    channels: SupervisorChannels,
}

impl Supervisor {
    /// Create a new supervisor with default channel sizes.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SupervisorState::default())),
            channels: SupervisorChannels::with_defaults(),
        }
    }

    /// Create a new supervisor with custom channel sizes.
    pub fn with_channel_sizes(rtcm_buffer: usize, msg_buffer: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(SupervisorState::default())),
            channels: SupervisorChannels::new(rtcm_buffer, msg_buffer),
        }
    }

    /// Get a handle for external interaction.
    pub fn handle(&self) -> SupervisorHandle {
        SupervisorHandle {
            state: Arc::clone(&self.state),
            shutdown_tx: self.channels.shutdown_tx.clone(),
        }
    }

    /// Get a clone of the shutdown receiver.
    pub fn shutdown_rx(&self) -> watch::Receiver<bool> {
        self.channels.shutdown_rx.clone()
    }

    /// Get the RTCM sender for NTRIP task.
    pub fn rtcm_tx(&self) -> mpsc::Sender<Vec<u8>> {
        self.channels.rtcm_tx.clone()
    }

    /// Get the GGA receiver for NTRIP task.
    pub fn gga_rx(&self) -> watch::Receiver<Option<GgaData>> {
        self.channels.gga_rx.clone()
    }

    /// Get the GGA sender for device task.
    pub fn gga_tx(&self) -> watch::Sender<Option<GgaData>> {
        self.channels.gga_tx.clone()
    }

    /// Get the message sender for tasks.
    pub fn msg_tx(&self) -> mpsc::Sender<GnssMessage> {
        self.channels.msg_tx.clone()
    }

    /// Update the device state.
    pub async fn set_device_state(&self, state: DeviceState) {
        let mut s = self.state.lock().await;
        tracing::info!(
            target: crate::logging::category::STATE,
            from = %s.device_state,
            to = %state,
            "Device state transition"
        );
        s.device_state = state;
    }

    /// Update the NTRIP state.
    pub async fn set_ntrip_state(&self, state: NtripState) {
        let mut s = self.state.lock().await;
        tracing::info!(
            target: crate::logging::category::STATE,
            from = %s.ntrip_state,
            to = %state,
            "NTRIP state transition"
        );
        s.ntrip_state = state;
    }

    /// Update the fix type.
    pub async fn set_fix_type(&self, fix_type: FixType) {
        let mut s = self.state.lock().await;
        if s.fix_type != fix_type {
            tracing::debug!(
                target: crate::logging::category::STATE,
                from = %s.fix_type,
                to = %fix_type,
                "Fix type changed"
            );
            s.fix_type = fix_type;
        }
    }

    /// Take ownership of the RTCM receiver (for device task).
    ///
    /// # Panics
    /// Panics if called more than once.
    pub fn take_rtcm_rx(&mut self) -> mpsc::Receiver<Vec<u8>> {
        self.channels
            .rtcm_rx
            .take()
            .expect("take_rtcm_rx called more than once")
    }

    /// Take ownership of the message receiver (for ROS publisher task).
    ///
    /// # Panics
    /// Panics if called more than once.
    pub fn take_msg_rx(&mut self) -> mpsc::Receiver<GnssMessage> {
        self.channels
            .msg_rx
            .take()
            .expect("take_msg_rx called more than once")
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

/// Setup shutdown signal handlers for graceful termination.
#[cfg(unix)]
#[allow(dead_code)] // Will be used when main.rs integrates supervisor
pub async fn setup_shutdown_signals(handle: SupervisorHandle) {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT, initiating shutdown");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, initiating shutdown");
        }
    }

    handle.shutdown();
}

/// Setup shutdown signal handlers for Windows.
#[cfg(windows)]
#[allow(dead_code)] // Will be used when main.rs integrates supervisor
pub async fn setup_shutdown_signals(handle: SupervisorHandle) {
    use tokio::signal::ctrl_c;

    ctrl_c().await.expect("Failed to setup Ctrl+C handler");
    tracing::info!("Received Ctrl+C, initiating shutdown");
    handle.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_creation() {
        let supervisor = Supervisor::new();
        let handle = supervisor.handle();

        let level = handle.diagnostic_level().await;
        // Default state: device waiting (ERROR), ntrip disabled (OK), no fix (ERROR)
        assert_eq!(level, DiagnosticLevel::Error);
    }

    #[tokio::test]
    async fn test_state_updates() {
        let supervisor = Supervisor::new();

        supervisor.set_device_state(DeviceState::Active).await;
        supervisor.set_ntrip_state(NtripState::Streaming).await;
        supervisor.set_fix_type(FixType::RtkFixed).await;

        let handle = supervisor.handle();
        let level = handle.diagnostic_level().await;
        assert_eq!(level, DiagnosticLevel::Ok);
    }

    #[tokio::test]
    async fn test_shutdown() {
        let supervisor = Supervisor::new();
        let handle = supervisor.handle();
        let mut shutdown_rx = supervisor.shutdown_rx();

        assert!(!*shutdown_rx.borrow());

        handle.shutdown();

        shutdown_rx.changed().await.unwrap();
        assert!(*shutdown_rx.borrow());
    }
}
