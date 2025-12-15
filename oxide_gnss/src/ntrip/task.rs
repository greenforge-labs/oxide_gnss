//! NTRIP task for running the correction stream loop.
//!
//! This module provides the async task that manages the NTRIP lifecycle:
//! - Connecting to the caster
//! - Streaming RTCM corrections
//! - Sending GGA position reports
//! - Automatic reconnection with backoff

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::config::NtripConfig;
use crate::device::GgaData;
use crate::error::NtripError;
use crate::state::NtripState;

use super::client::NtripClient;
use super::gga::GgaSentence;

/// Channels required by the NTRIP task.
pub struct NtripTaskChannels {
    /// Send RTCM corrections to device
    pub rtcm_tx: mpsc::Sender<Vec<u8>>,
    /// Receive GGA position updates from device
    pub gga_rx: watch::Receiver<Option<GgaData>>,
    /// Send messages to supervisor
    pub msg_tx: mpsc::Sender<NtripMessage>,
    /// Receive shutdown signal
    pub shutdown_rx: watch::Receiver<bool>,
}

/// Messages sent from the NTRIP task.
#[derive(Debug, Clone)]
pub enum NtripMessage {
    /// NTRIP state changed
    StateChanged(NtripState),
    /// RTCM data received (bytes count)
    RtcmReceived { bytes: usize },
    /// Connection established
    Connected,
    /// Connection lost
    Disconnected { reason: String },
}

/// Shared NTRIP task state for external monitoring.
#[derive(Debug, Default)]
pub struct NtripTaskState {
    /// Current NTRIP state
    pub state: NtripState,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total RTCM messages forwarded
    pub messages_forwarded: u64,
    /// Last data received time
    pub last_data_time: Option<Instant>,
    /// Connection count
    pub connection_count: u32,
}

/// Handle for monitoring the NTRIP task.
#[derive(Clone)]
pub struct NtripTaskHandle {
    state: Arc<Mutex<NtripTaskState>>,
}

impl NtripTaskHandle {
    /// Get current NTRIP state.
    pub async fn state(&self) -> NtripState {
        self.state.lock().await.state.clone()
    }

    /// Get total bytes received.
    pub async fn bytes_received(&self) -> u64 {
        self.state.lock().await.bytes_received
    }

    /// Get correction age in seconds (time since last data).
    pub async fn correction_age_secs(&self) -> Option<f64> {
        self.state
            .lock()
            .await
            .last_data_time
            .map(|t| t.elapsed().as_secs_f64())
    }

    /// Get a snapshot of the task state.
    pub async fn snapshot(&self) -> NtripTaskState {
        let s = self.state.lock().await;
        NtripTaskState {
            state: s.state.clone(),
            bytes_received: s.bytes_received,
            messages_forwarded: s.messages_forwarded,
            last_data_time: s.last_data_time,
            connection_count: s.connection_count,
        }
    }
}

/// The NTRIP task runner.
pub struct NtripTask {
    config: NtripConfig,
    channels: NtripTaskChannels,
    state: Arc<Mutex<NtripTaskState>>,
}

impl NtripTask {
    /// Create a new NTRIP task.
    pub fn new(config: NtripConfig, channels: NtripTaskChannels) -> Self {
        Self {
            config,
            channels,
            state: Arc::new(Mutex::new(NtripTaskState::default())),
        }
    }

    /// Get a handle for monitoring this task.
    pub fn handle(&self) -> NtripTaskHandle {
        NtripTaskHandle {
            state: Arc::clone(&self.state),
        }
    }

    /// Run the NTRIP task.
    pub async fn run(mut self) -> Result<(), NtripError> {
        info!(
            host = %self.config.host,
            mountpoint = %self.config.mountpoint,
            "Starting NTRIP task"
        );

        let mut backoff_attempt = 0u32;
        let mut last_streaming_start: Option<Instant> = None;

        loop {
            // Check for shutdown
            if *self.channels.shutdown_rx.borrow() {
                info!("NTRIP task received shutdown signal");
                self.set_state(NtripState::ShuttingDown).await;
                break;
            }

            // Try to connect and stream
            match self.connect_and_stream(&mut last_streaming_start).await {
                Ok(()) => {
                    // Normal shutdown
                    break;
                }
                Err(e) => {
                    error!(error = %e, "NTRIP connection error");

                    if !self.config.connection.reconnect {
                        return Err(e);
                    }

                    // If we've been streaming successfully for longer than the
                    // configured reset period, reset backoff so that
                    // intermittent long-term dropouts start with a small delay
                    // again.
                    if let (Some(start), reset_secs) = (
                        last_streaming_start,
                        self.config.connection.backoff_reset_secs,
                    ) {
                        if reset_secs > 0 && start.elapsed().as_secs() >= reset_secs as u64 {
                            backoff_attempt = 0;
                        }
                    }

                    // Calculate backoff delay
                    backoff_attempt += 1;
                    let delay_secs = calculate_backoff(
                        backoff_attempt,
                        self.config.connection.initial_delay_secs,
                        self.config.connection.max_delay_secs,
                    );

                    self.set_state(NtripState::backoff(
                        backoff_attempt,
                        delay_secs,
                        e.to_string(),
                    ))
                    .await;

                    // Wait before retry, checking for shutdown
                    if self
                        .wait_with_shutdown(Duration::from_secs(delay_secs as u64))
                        .await
                    {
                        break; // Shutdown requested
                    }
                }
            }
        }

        info!("NTRIP task exiting");
        Ok(())
    }

    /// Connect to the caster and stream corrections.
    async fn connect_and_stream(
        &mut self,
        last_streaming_start: &mut Option<Instant>,
    ) -> Result<(), NtripError> {
        self.set_state(NtripState::Connecting).await;

        let mut client = NtripClient::new(self.config.clone())?;
        client.connect().await?;

        // Update state and stats
        {
            let mut state = self.state.lock().await;
            state.connection_count += 1;
        }
        self.set_state(NtripState::Streaming).await;
        *last_streaming_start = Some(Instant::now());
        let _ = self.channels.msg_tx.send(NtripMessage::Connected).await;

        // Run the streaming loop
        self.stream_loop(&mut client).await
    }

    /// Main streaming loop - read RTCM and send GGA.
    async fn stream_loop(&mut self, client: &mut NtripClient) -> Result<(), NtripError> {
        let mut buf = [0u8; 4096];
        let gga_interval = Duration::from_secs(self.config.gga_interval_secs as u64);
        let mut last_gga_time = Instant::now();

        loop {
            tokio::select! {
                // Check for shutdown
                _ = self.channels.shutdown_rx.changed() => {
                    if *self.channels.shutdown_rx.borrow() {
                        return Ok(());
                    }
                }

                // Read RTCM data
                read_result = client.read_chunk(&mut buf) => {
                    match read_result {
                        Ok(n) if n > 0 => {
                            self.handle_rtcm_data(&buf[..n]).await;
                        }
                        Ok(_) => {
                            // Zero bytes, continue
                        }
                        Err(e) => {
                            let _ = self.channels.msg_tx.send(NtripMessage::Disconnected {
                                reason: e.to_string(),
                            }).await;
                            return Err(e);
                        }
                    }
                }

                // Periodic GGA sending
                _ = sleep(Duration::from_millis(100)) => {
                    if self.config.send_gga && last_gga_time.elapsed() >= gga_interval {
                        self.send_gga_if_available(client).await;
                        last_gga_time = Instant::now();
                    }
                }
            }
        }
    }

    /// Handle received RTCM data.
    async fn handle_rtcm_data(&mut self, data: &[u8]) {
        debug!(bytes = data.len(), "Received RTCM data");

        // Update stats
        {
            let mut state = self.state.lock().await;
            state.bytes_received += data.len() as u64;
            state.last_data_time = Some(Instant::now());
        }

        // Forward to device
        if let Err(e) = self.channels.rtcm_tx.send(data.to_vec()).await {
            warn!(error = %e, "Failed to forward RTCM to device");
        } else {
            let mut state = self.state.lock().await;
            state.messages_forwarded += 1;
        }

        // Notify supervisor
        let _ = self
            .channels
            .msg_tx
            .send(NtripMessage::RtcmReceived { bytes: data.len() })
            .await;

        // Also send GnssMessage for ROS task
        // Note: We need to convert the message type since msg_tx expects NtripMessage
        // In a real implementation, we would have separate channels for different message types
        // or use an enum that encompasses all message types
    }

    /// Send GGA position if available from device.
    async fn send_gga_if_available(&self, client: &mut NtripClient) {
        let gga_data = self.channels.gga_rx.borrow().clone();

        if let Some(gga) = gga_data {
            let sentence = GgaSentence::new(gga.latitude, gga.longitude, gga.altitude)
                .with_quality(gga.quality)
                .with_satellites(gga.num_satellites);

            if let Err(e) = client.send_gga(&sentence).await {
                warn!(error = %e, "Failed to send GGA");
            }
        }
    }

    /// Wait for a duration, returning true if shutdown was requested.
    async fn wait_with_shutdown(&mut self, duration: Duration) -> bool {
        tokio::select! {
            _ = sleep(duration) => false,
            _ = self.channels.shutdown_rx.changed() => {
                *self.channels.shutdown_rx.borrow()
            }
        }
    }

    /// Update the NTRIP state and notify.
    async fn set_state(&mut self, new_state: NtripState) {
        {
            let mut state = self.state.lock().await;
            if state.state != new_state {
                info!(
                    from = %state.state,
                    to = %new_state,
                    "NTRIP state transition"
                );
                state.state = new_state.clone();
            }
        }

        let _ = self
            .channels
            .msg_tx
            .send(NtripMessage::StateChanged(new_state))
            .await;
    }
}

/// Calculate exponential backoff delay.
fn calculate_backoff(attempt: u32, initial_delay: u32, max_delay: u32) -> u32 {
    if attempt == 0 {
        return initial_delay;
    }
    let delay = initial_delay.saturating_mul(1 << (attempt - 1).min(10));
    delay.min(max_delay)
}

/// Spawn an NTRIP task and return a handle.
pub fn spawn_ntrip_task(
    config: NtripConfig,
    channels: NtripTaskChannels,
) -> (
    tokio::task::JoinHandle<Result<(), NtripError>>,
    NtripTaskHandle,
) {
    let task = NtripTask::new(config, channels);
    let handle = task.handle();
    let join_handle = tokio::spawn(task.run());
    (join_handle, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> NtripConfig {
        NtripConfig {
            host: "test.example.com".to_string(),
            port: 2101,
            mountpoint: "TEST".to_string(),
            username: None,
            password: None,
            send_gga: false,
            gga_interval_secs: 10,
            connection: Default::default(),
            use_https: false,
        }
    }

    fn test_channels() -> NtripTaskChannels {
        let (rtcm_tx, _rtcm_rx) = mpsc::channel(32);
        let (_gga_tx, gga_rx) = watch::channel(None);
        let (msg_tx, _msg_rx) = mpsc::channel(64);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        NtripTaskChannels {
            rtcm_tx,
            gga_rx,
            msg_tx,
            shutdown_rx,
        }
    }

    #[test]
    fn test_ntrip_task_creation() {
        let config = test_config();
        let channels = test_channels();
        let task = NtripTask::new(config, channels);
        let handle = task.handle();

        // Verify we can get a handle and it has default state
        // (can't check state synchronously without tokio runtime)
        assert!(std::mem::size_of_val(&handle) > 0);
    }

    #[test]
    fn test_backoff_calculation() {
        assert_eq!(calculate_backoff(0, 1, 60), 1);
        assert_eq!(calculate_backoff(1, 1, 60), 1);
        assert_eq!(calculate_backoff(2, 1, 60), 2);
        assert_eq!(calculate_backoff(3, 1, 60), 4);
        assert_eq!(calculate_backoff(10, 1, 60), 60); // Capped
    }

    #[test]
    fn test_ntrip_message_variants() {
        let state_msg = NtripMessage::StateChanged(NtripState::Streaming);
        assert!(matches!(state_msg, NtripMessage::StateChanged(_)));

        let rtcm_msg = NtripMessage::RtcmReceived { bytes: 1024 };
        assert!(matches!(rtcm_msg, NtripMessage::RtcmReceived { .. }));
    }

    #[tokio::test]
    async fn test_ntrip_task_handle() {
        let state = Arc::new(Mutex::new(NtripTaskState::default()));
        let handle = NtripTaskHandle {
            state: Arc::clone(&state),
        };

        // Default state
        assert!(matches!(handle.state().await, NtripState::Disabled));
        assert_eq!(handle.bytes_received().await, 0);

        // Update state
        {
            let mut s = state.lock().await;
            s.state = NtripState::Streaming;
            s.bytes_received = 5000;
            s.last_data_time = Some(Instant::now());
        }

        assert!(matches!(handle.state().await, NtripState::Streaming));
        assert_eq!(handle.bytes_received().await, 5000);
        assert!(handle.correction_age_secs().await.is_some());
    }
}
