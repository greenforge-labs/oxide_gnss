//! Device task for running the GNSS receiver communication loop.
//!
//! This module provides the async task that manages the device lifecycle:
//! - Opening the serial port
//! - Running the configuration sequence
//! - Processing incoming UBX messages
//! - Injecting RTCM corrections
//! - Reporting position data for NTRIP GGA

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::config::DeviceConfig;
use crate::error::DeviceError;
use crate::state::{DeviceState, FixType, GnssIntegrity, IntegrityAggregator};

use super::config::{ConfigStep, ConfiguratorOptions, DeviceConfigurator};
use super::serial::SerialPortBuilder;
use super::ubx::{
    CovData, HpPosData, MonCommsData, MonHwData, MonRfData, PosEcefData, PvtData, RxmCorData,
    SatInfo, SecSigData, SecSiglogData, UbxHandler,
};

/// Channels required by the device task.
pub struct DeviceTaskChannels {
    /// Receive RTCM corrections from NTRIP
    pub rtcm_rx: mpsc::Receiver<Vec<u8>>,
    /// Send GGA position updates to NTRIP
    pub gga_tx: watch::Sender<Option<GgaData>>,
    /// Send messages to supervisor/ROS
    pub msg_tx: mpsc::Sender<DeviceMessage>,
    /// Receive shutdown signal
    pub shutdown_rx: watch::Receiver<bool>,
}

/// GGA position data for NTRIP reporting.
#[derive(Debug, Clone, Default)]
pub struct GgaData {
    /// Latitude in degrees
    pub latitude: f64,
    /// Longitude in degrees
    pub longitude: f64,
    /// Altitude in meters (MSL)
    pub altitude: f64,
    /// Fix quality (0=invalid, 1=GPS, 2=DGPS, 4=RTK fixed, 5=RTK float)
    pub quality: u8,
    /// Number of satellites used
    pub num_satellites: u8,
}

impl GgaData {
    /// Create GGA data from PVT data.
    pub fn from_pvt(pvt: &PvtData) -> Self {
        let quality = match pvt.fix_type {
            FixType::NoFix => 0,
            FixType::Fix2D | FixType::Fix3D | FixType::GnssDr => 1,
            FixType::RtkFloat => 5,
            FixType::RtkFixed => 4,
            _ => 1,
        };

        Self {
            latitude: pvt.lat,
            longitude: pvt.lon,
            altitude: pvt.height_msl,
            quality,
            num_satellites: pvt.num_sv,
        }
    }
}

/// Messages sent from the device task.
#[derive(Debug, Clone)]
pub enum DeviceMessage {
    /// Device state changed
    StateChanged(DeviceState),
    /// New PVT data available
    Pvt(PvtData),
    /// Fix type changed
    FixTypeChanged(FixType),
    /// High precision position
    HpPos(HpPosData),
    /// Satellite status
    SatInfo(SatInfo),
    /// Integrity status update
    Integrity(GnssIntegrity),
    /// Position covariance
    Covariance(CovData),
    /// ECEF position
    PosEcef(PosEcefData),
    /// Security signal status
    SecSig(SecSigData),
    /// Security event log
    SecSiglog(SecSiglogData),
    /// Correction status
    RxmCor(RxmCorData),
    /// Communication port status
    MonComms(MonCommsData),
    /// Hardware status (antenna, jamming indicator) - deprecated
    MonHw(MonHwData),
    /// RF status (antenna, jamming indicator) - replaces MonHw
    MonRf(MonRfData),
}

impl DeviceMessage {
    /// Convert to a GnssMessage for forwarding to the supervisor/ROS task.
    ///
    /// Returns `None` for internal messages that are processed by the
    /// IntegrityAggregator and don't need to be forwarded (Covariance,
    /// PosEcef, SecSiglog, RxmCor, MonComms, MonHw, MonRf, FixTypeChanged).
    pub fn into_gnss_message(self) -> Option<crate::state::GnssMessage> {
        use crate::state::GnssMessage;
        match self {
            DeviceMessage::Pvt(pvt) => Some(GnssMessage::Pvt(pvt)),
            DeviceMessage::StateChanged(state) => Some(GnssMessage::DeviceStateChanged(state)),
            DeviceMessage::HpPos(hp) => Some(GnssMessage::HpPos(hp)),
            DeviceMessage::SatInfo(sat) => Some(GnssMessage::SatInfo(sat)),
            DeviceMessage::SecSig(sig) => Some(GnssMessage::SecSig(sig)),
            DeviceMessage::Integrity(integrity) => Some(GnssMessage::Integrity(integrity)),
            // Internal messages processed by IntegrityAggregator - not forwarded
            DeviceMessage::FixTypeChanged(_)
            | DeviceMessage::Covariance(_)
            | DeviceMessage::PosEcef(_)
            | DeviceMessage::SecSiglog(_)
            | DeviceMessage::RxmCor(_)
            | DeviceMessage::MonComms(_)
            | DeviceMessage::MonHw(_)
            | DeviceMessage::MonRf(_) => None,
        }
    }
}

/// Shared device task state for external monitoring.
#[derive(Debug, Default)]
pub struct DeviceTaskState {
    /// Current device state
    pub state: DeviceState,
    /// Current fix type
    pub fix_type: FixType,
    /// Messages received count
    pub messages_received: u64,
    /// RTCM bytes injected
    pub rtcm_bytes_injected: u64,
}

/// Handle for monitoring the device task.
#[derive(Clone)]
pub struct DeviceTaskHandle {
    state: Arc<Mutex<DeviceTaskState>>,
}

impl DeviceTaskHandle {
    /// Get current device state.
    pub async fn state(&self) -> DeviceState {
        self.state.lock().await.state.clone()
    }

    /// Get current fix type.
    pub async fn fix_type(&self) -> FixType {
        self.state.lock().await.fix_type
    }

    /// Get a snapshot of the task state.
    pub async fn snapshot(&self) -> DeviceTaskState {
        let s = self.state.lock().await;
        DeviceTaskState {
            state: s.state.clone(),
            fix_type: s.fix_type,
            messages_received: s.messages_received,
            rtcm_bytes_injected: s.rtcm_bytes_injected,
        }
    }
}

/// The device task runner.
pub struct DeviceTask {
    config: DeviceConfig,
    channels: DeviceTaskChannels,
    state: Arc<Mutex<DeviceTaskState>>,
    configurator_options: ConfiguratorOptions,
    /// Integrity aggregator for safety monitoring
    integrity: IntegrityAggregator,
}

impl DeviceTask {
    /// Create a new device task.
    pub fn new(config: DeviceConfig, channels: DeviceTaskChannels) -> Self {
        Self {
            config,
            channels,
            state: Arc::new(Mutex::new(DeviceTaskState::default())),
            configurator_options: ConfiguratorOptions::default(),
            integrity: IntegrityAggregator::new(),
        }
    }

    /// Create a device task with custom configurator options.
    pub fn with_configurator_options(
        config: DeviceConfig,
        channels: DeviceTaskChannels,
        configurator_options: ConfiguratorOptions,
    ) -> Self {
        Self {
            config,
            channels,
            state: Arc::new(Mutex::new(DeviceTaskState::default())),
            configurator_options,
            integrity: IntegrityAggregator::new(),
        }
    }

    /// Get a handle for monitoring this task.
    pub fn handle(&self) -> DeviceTaskHandle {
        DeviceTaskHandle {
            state: Arc::clone(&self.state),
        }
    }

    /// Run the device task.
    ///
    /// This is the main entry point - call this with `tokio::spawn`.
    pub async fn run(mut self) -> Result<(), DeviceError> {
        info!(port = %self.config.port, "Starting device task");

        // Counts consecutive failures of the state machine (connect/config/active).
        let mut connect_attempt: u32 = 0;
        // Tracks when we entered Active state for backoff reset logic.
        let mut last_active_start: Option<Instant> = None;

        loop {
            // Check for shutdown
            if *self.channels.shutdown_rx.borrow() {
                info!("Device task received shutdown signal");
                self.set_state(DeviceState::ShuttingDown).await;
                break;
            }

            // Run the state machine
            match self.run_state_machine(&mut last_active_start).await {
                Ok(()) => {
                    // Normal exit (shutdown).
                    break;
                }
                Err(e) => {
                    let reconnect_cfg = &self.config.reconnect;

                    // If we've been running successfully for longer than the
                    // configured reset period, reset backoff so that
                    // intermittent long-term dropouts start with a small delay.
                    if let Some(start) = last_active_start {
                        let reset_secs = reconnect_cfg.backoff_reset_secs;
                        if reset_secs > 0 && start.elapsed().as_secs() >= reset_secs as u64 {
                            connect_attempt = 0;
                        }
                    }

                    connect_attempt = connect_attempt.saturating_add(1);

                    // If automatic reconnection is disabled, enter a stable waiting state
                    // and stop retrying. Higher-level supervision can decide what to do.
                    if !reconnect_cfg.enabled {
                        error!(
                            port = %self.config.port,
                            error = %e,
                            "Device task error and reconnect disabled; entering Waiting state"
                        );
                        self.set_state(DeviceState::waiting(format!(
                            "Connection failed and reconnect disabled: {}",
                            e
                        )))
                        .await;
                        break;
                    }

                    // Respect max_attempts when non-zero: after exceeding the limit, enter a
                    // stable Waiting state and stop retrying.
                    if reconnect_cfg.max_attempts > 0
                        && connect_attempt > reconnect_cfg.max_attempts
                    {
                        error!(
                            port = %self.config.port,
                            attempts = connect_attempt,
                            error = %e,
                            "Max connection attempts reached; giving up"
                        );
                        self.set_state(DeviceState::waiting(format!(
                            "Max connection attempts ({}) reached: {}",
                            reconnect_cfg.max_attempts, e
                        )))
                        .await;
                        break;
                    }

                    // Calculate backoff delay for this attempt.
                    let delay_secs = crate::util::calculate_backoff(
                        connect_attempt,
                        reconnect_cfg.initial_delay_secs,
                        reconnect_cfg.max_delay_secs,
                    );

                    self.set_state(DeviceState::reconnecting(
                        connect_attempt,
                        reconnect_cfg.max_attempts,
                        format!("Connection failed: {}", e),
                    ))
                    .await;

                    error!(
                        port = %self.config.port,
                        attempts = connect_attempt,
                        delay_secs = delay_secs,
                        error = %e,
                        "Device task error, will retry after backoff"
                    );

                    sleep(Duration::from_secs(delay_secs as u64)).await;
                }
            }
        }

        info!("Device task exiting");
        Ok(())
    }

    /// Run the device state machine.
    async fn run_state_machine(
        &mut self,
        last_active_start: &mut Option<Instant>,
    ) -> Result<(), DeviceError> {
        // Clear active start so failed connections don't trigger backoff reset
        // from a stale value. Only successful entry to Active state should set this.
        *last_active_start = None;

        // Phase 1: Connect
        self.set_state(DeviceState::Connecting).await;
        let mut serial = self.connect().await?;

        // Phase 2: Configure
        let total_steps = ConfigStep::total();
        self.set_state(DeviceState::configuring(1, total_steps))
            .await;

        let mut ubx = UbxHandler::new();
        let configurator = DeviceConfigurator::with_options(self.configurator_options.clone());

        if let Err(e) = configurator
            .configure(&mut serial, &mut ubx, &self.config)
            .await
        {
            error!(error = %e, "Device configuration failed");
            return Err(e);
        }

        // Phase 3: Active - main read loop
        self.set_state(DeviceState::Active).await;
        *last_active_start = Some(Instant::now());
        self.run_active_loop(&mut serial, &mut ubx).await
    }

    /// Attempt to connect to the serial port.
    async fn connect(&mut self) -> Result<super::SerialPort, DeviceError> {
        let builder = SerialPortBuilder::new(&self.config.port)
            .baud_rate(self.config.baud_rate)
            .reconnect_config(self.config.reconnect.clone());

        match builder.open().await {
            Ok(port) => {
                info!(port = %self.config.port, "Connected to device");
                Ok(port)
            }
            Err(e) => {
                self.set_state(DeviceState::waiting(format!("Connection failed: {}", e)))
                    .await;
                Err(e)
            }
        }
    }

    /// Run the main active loop - reading messages and injecting RTCM.
    async fn run_active_loop(
        &mut self,
        serial: &mut super::SerialPort,
        ubx: &mut UbxHandler,
    ) -> Result<(), DeviceError> {
        let mut read_buf = [0u8; 1024];

        loop {
            tokio::select! {
                // Check for shutdown
                _ = self.channels.shutdown_rx.changed() => {
                    if *self.channels.shutdown_rx.borrow() {
                        info!("Shutdown signal received in active loop");
                        return Ok(());
                    }
                }

                // Read from serial port
                read_result = serial.read(&mut read_buf) => {
                    match read_result {
                        Ok(n) if n > 0 => {
                            self.process_serial_data(&read_buf[..n], ubx).await;
                        }
                        Ok(_) => {
                            // Zero bytes read, continue
                        }
                        Err(e) => {
                            error!(error = %e, "Serial read error");
                            return Err(e);
                        }
                    }
                }

                // Receive RTCM data to inject
                rtcm = self.channels.rtcm_rx.recv() => {
                    if let Some(data) = rtcm {
                        self.inject_rtcm(serial, &data).await;
                    }
                }
            }
        }
    }

    /// Process data received from serial port.
    async fn process_serial_data(&mut self, data: &[u8], ubx: &mut UbxHandler) {
        let result = ubx.process(data);

        if result.messages_processed > 0 {
            let mut state = self.state.lock().await;
            state.messages_received += result.messages_processed as u64;
        }

        // Track if we need to recompute integrity
        let mut integrity_updated = false;

        // Handle PVT data
        if let Some(pvt) = result.pvt {
            self.handle_pvt(pvt).await;
            integrity_updated = true;
        }

        // Handle HP Position
        if let Some(hp) = result.hp_pos {
            let _ = self.channels.msg_tx.send(DeviceMessage::HpPos(hp)).await;
        }

        // Handle Satellite Info
        if let Some(sat) = result.sat_info {
            let _ = self.channels.msg_tx.send(DeviceMessage::SatInfo(sat)).await;
        }

        // Handle NAV-COV (position/velocity covariance)
        if let Some(ref cov) = result.cov {
            self.integrity.update_covariance(cov);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::Covariance(cov.clone()))
                .await;
            integrity_updated = true;
        }

        // Handle NAV-POSECEF
        if let Some(ref pos) = result.pos_ecef {
            self.integrity.update_pos_ecef(pos);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::PosEcef(pos.clone()))
                .await;
        }

        // Handle SEC-SIG (jamming/spoofing detection)
        if let Some(ref sig) = result.sec_sig {
            self.integrity.update_sec_sig(sig);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::SecSig(sig.clone()))
                .await;
            integrity_updated = true;
        }

        // Handle SEC-SIGLOG (security event log)
        if let Some(ref siglog) = result.sec_siglog {
            self.integrity.update_sec_siglog(siglog);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::SecSiglog(siglog.clone()))
                .await;
            integrity_updated = true;
        }

        // Handle RXM-COR (correction status)
        if let Some(ref cor) = result.rxm_cor {
            self.integrity.update_rxm_cor(cor);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::RxmCor(cor.clone()))
                .await;
            integrity_updated = true;
        }

        // Handle MON-COMMS (communication port status)
        if let Some(ref comms) = result.mon_comms {
            self.integrity.update_mon_comms(comms);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::MonComms(comms.clone()))
                .await;
        }

        // Handle MON-HW (hardware status) - deprecated, prefer MON-RF
        if let Some(ref hw) = result.mon_hw {
            self.integrity.update_mon_hw(hw);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::MonHw(hw.clone()))
                .await;
            integrity_updated = true;
        }

        // Handle MON-RF (RF status) - replaces MON-HW
        if let Some(ref rf) = result.mon_rf {
            self.integrity.update_mon_rf(rf);
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::MonRf(rf.clone()))
                .await;
            integrity_updated = true;
        }

        // Compute and send integrity update if any relevant data changed
        if integrity_updated {
            let integrity = self.integrity.compute();
            let _ = self
                .channels
                .msg_tx
                .send(DeviceMessage::Integrity(integrity))
                .await;
        }
    }

    /// Handle received PVT data.
    ///
    /// Takes ownership of `pvt` to avoid cloning when forwarding.
    async fn handle_pvt(&mut self, pvt: PvtData) {
        debug!(
            lat = pvt.lat,
            lon = pvt.lon,
            alt = pvt.height_msl,
            fix = ?pvt.fix_type,
            sats = pvt.num_sv,
            "PVT received"
        );

        // Update integrity aggregator with PVT data
        let carrier_solution = match pvt.carr_soln {
            super::ubx::CarrierSolution::None => 0,
            super::ubx::CarrierSolution::Float => 1,
            super::ubx::CarrierSolution::Fixed => 2,
        };
        let differential = matches!(pvt.fix_type, FixType::RtkFloat | FixType::RtkFixed);
        self.integrity.update_pvt(
            pvt.fix_type,
            carrier_solution,
            differential,
            pvt.num_sv,
            pvt.h_acc,
            pvt.v_acc,
            pvt.p_dop,
        );

        // Update fix type
        {
            let mut state = self.state.lock().await;
            if state.fix_type != pvt.fix_type {
                state.fix_type = pvt.fix_type;
                // Send fix type change message
                let _ = self
                    .channels
                    .msg_tx
                    .send(DeviceMessage::FixTypeChanged(pvt.fix_type))
                    .await;
            }
        }

        // Update GGA for NTRIP (uses reference, pvt still owned)
        let gga = GgaData::from_pvt(&pvt);
        let _ = self.channels.gga_tx.send(Some(gga));

        // Send PVT message - ownership transferred, no clone needed
        // Note: Integrity is published separately in process_serial_data() via the
        // integrity_updated flag, avoiding duplicate publishing.
        let _ = self.channels.msg_tx.send(DeviceMessage::Pvt(pvt)).await;
    }

    /// Inject RTCM correction data to the device.
    async fn inject_rtcm(&mut self, serial: &mut super::SerialPort, data: &[u8]) {
        match serial.write(data).await {
            Ok(n) => {
                debug!(bytes = n, "RTCM data injected");
                let mut state = self.state.lock().await;
                state.rtcm_bytes_injected += n as u64;
            }
            Err(e) => {
                warn!(error = %e, "Failed to inject RTCM data");
            }
        }
    }

    /// Update the device state and notify.
    async fn set_state(&mut self, new_state: DeviceState) {
        {
            let mut state = self.state.lock().await;
            if state.state != new_state {
                info!(
                    from = %state.state,
                    to = %new_state,
                    "Device state transition"
                );
                state.state = new_state.clone();
            }
        }

        // Send state change message
        let _ = self
            .channels
            .msg_tx
            .send(DeviceMessage::StateChanged(new_state))
            .await;
    }
}

/// Spawn a device task and return a handle.
///
/// This is a convenience function for spawning the task.
pub fn spawn_device_task(
    config: DeviceConfig,
    channels: DeviceTaskChannels,
) -> (
    tokio::task::JoinHandle<Result<(), DeviceError>>,
    DeviceTaskHandle,
) {
    let task = DeviceTask::new(config, channels);
    let handle = task.handle();
    let join_handle = tokio::spawn(task.run());
    (join_handle, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gga_from_pvt() {
        use std::time::Instant;

        let pvt = PvtData {
            itow: 0,
            year: 2024,
            month: 12,
            day: 6,
            hour: 12,
            min: 0,
            sec: 0,
            nano: 0,
            fix_type: FixType::RtkFixed,
            num_sv: 15,
            lon: 153.0,
            lat: -27.5,
            height: 100.0,
            height_msl: 95.0,
            h_acc: 0.01,
            v_acc: 0.02,
            vel_n: 0.0,
            vel_e: 0.0,
            vel_d: 0.0,
            g_speed: 0.0,
            head_mot: 0.0,
            s_acc: 0.01,
            head_acc: 1.0,
            p_dop: 1.2,
            carr_soln: super::super::ubx::CarrierSolution::Fixed,
            received_at: Instant::now(),
        };

        let gga = GgaData::from_pvt(&pvt);
        assert_eq!(gga.latitude, -27.5);
        assert_eq!(gga.longitude, 153.0);
        assert_eq!(gga.altitude, 95.0);
        assert_eq!(gga.quality, 4); // RTK Fixed
        assert_eq!(gga.num_satellites, 15);
    }

    #[test]
    fn test_gga_quality_mapping() {
        use std::time::Instant;

        let make_pvt = |fix_type: FixType| PvtData {
            itow: 0,
            year: 2024,
            month: 12,
            day: 6,
            hour: 12,
            min: 0,
            sec: 0,
            nano: 0,
            fix_type,
            num_sv: 10,
            lon: 0.0,
            lat: 0.0,
            height: 0.0,
            height_msl: 0.0,
            h_acc: 0.0,
            v_acc: 0.0,
            vel_n: 0.0,
            vel_e: 0.0,
            vel_d: 0.0,
            g_speed: 0.0,
            head_mot: 0.0,
            s_acc: 0.0,
            head_acc: 0.0,
            p_dop: 0.0,
            carr_soln: super::super::ubx::CarrierSolution::None,
            received_at: Instant::now(),
        };

        assert_eq!(GgaData::from_pvt(&make_pvt(FixType::NoFix)).quality, 0);
        assert_eq!(GgaData::from_pvt(&make_pvt(FixType::Fix3D)).quality, 1);
        assert_eq!(GgaData::from_pvt(&make_pvt(FixType::RtkFloat)).quality, 5);
        assert_eq!(GgaData::from_pvt(&make_pvt(FixType::RtkFixed)).quality, 4);
    }

    #[test]
    fn test_device_message_variants() {
        let state_msg = DeviceMessage::StateChanged(DeviceState::Active);
        assert!(matches!(state_msg, DeviceMessage::StateChanged(_)));

        let fix_msg = DeviceMessage::FixTypeChanged(FixType::RtkFixed);
        assert!(matches!(fix_msg, DeviceMessage::FixTypeChanged(_)));
    }

    #[tokio::test]
    async fn test_device_task_handle() {
        let state = Arc::new(Mutex::new(DeviceTaskState::default()));
        let handle = DeviceTaskHandle {
            state: Arc::clone(&state),
        };

        // Default state
        assert!(matches!(handle.state().await, DeviceState::Waiting { .. }));
        assert_eq!(handle.fix_type().await, FixType::NoFix);

        // Update state
        {
            let mut s = state.lock().await;
            s.state = DeviceState::Active;
            s.fix_type = FixType::RtkFixed;
            s.messages_received = 100;
        }

        assert!(matches!(handle.state().await, DeviceState::Active));
        assert_eq!(handle.fix_type().await, FixType::RtkFixed);

        let snapshot = handle.snapshot().await;
        assert_eq!(snapshot.messages_received, 100);
    }
}
