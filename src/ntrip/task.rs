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

use ntrip_core::{GgaSentence, NtripClient};

use crate::config::NtripConfig;
use crate::device::GgaData;
use crate::error::NtripError;
use crate::state::NtripState;

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
    /// RTCM data received.
    RtcmReceived {
        /// Number of bytes in this RTCM payload.
        bytes: usize,
    },
    /// Connection established.
    Connected,
    /// Connection lost.
    Disconnected {
        /// Human-readable disconnection reason.
        reason: String,
    },
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
                    let delay_secs = crate::util::calculate_backoff(
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
        // Clear streaming start so failed connections don't trigger backoff reset
        // from a stale value. Only successful streaming should set this.
        *last_streaming_start = None;

        self.set_state(NtripState::Connecting).await;

        let core_config = self.config.to_ntrip_core_config();
        let mut client = NtripClient::new(core_config)?;

        // Try to get initial GGA position for Ntrip-GGA header (NTRIP v2 best practice)
        let initial_gga = self.channels.gga_rx.borrow().clone().map(|gga| {
            GgaSentence::new(gga.latitude, gga.longitude, gga.altitude)
                .with_quality(gga.quality)
                .with_satellites(gga.num_satellites)
        });

        client.connect_with_gga(initial_gga.as_ref()).await?;

        // Update state and stats
        {
            let mut state = self.state.lock().await;
            state.connection_count += 1;
        }
        self.set_state(NtripState::Streaming).await;
        *last_streaming_start = Some(Instant::now());
        if self
            .channels
            .msg_tx
            .send(NtripMessage::Connected)
            .await
            .is_err()
        {
            warn!(
                target: crate::logging::category::NTRIP,
                "Failed to send connected message - receiver may be shutting down"
            );
        }

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
                            if self
                                .channels
                                .msg_tx
                                .send(NtripMessage::Disconnected {
                                    reason: e.to_string(),
                                })
                                .await
                                .is_err()
                            {
                                warn!(
                                    target: crate::logging::category::NTRIP,
                                    "Failed to send disconnected message - receiver may be shutting down"
                                );
                            }
                            return Err(e.into());
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
            tls_skip_verify: false,
            ntrip_version: Default::default(),
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

    // ========================================================================
    // Error path and state transition tests
    // ========================================================================

    #[tokio::test]
    async fn test_handle_state_transitions() {
        let state = Arc::new(Mutex::new(NtripTaskState::default()));
        let handle = NtripTaskHandle {
            state: Arc::clone(&state),
        };

        // Simulate state progression: Disabled -> Connecting -> Streaming
        {
            let mut s = state.lock().await;
            s.state = NtripState::Connecting;
        }
        assert!(matches!(handle.state().await, NtripState::Connecting));

        {
            let mut s = state.lock().await;
            s.state = NtripState::Streaming;
            s.connection_count = 1;
        }
        assert!(matches!(handle.state().await, NtripState::Streaming));

        // Verify connection count tracked
        let snapshot = handle.snapshot().await;
        assert_eq!(snapshot.connection_count, 1);
    }

    #[tokio::test]
    async fn test_handle_backoff_state() {
        let state = Arc::new(Mutex::new(NtripTaskState::default()));
        let handle = NtripTaskHandle {
            state: Arc::clone(&state),
        };

        // Simulate backoff state
        {
            let mut s = state.lock().await;
            s.state = NtripState::Backoff {
                attempt: 3,
                delay_secs: 8,
                reason: "Connection refused".to_string(),
            };
        }

        let current = handle.state().await;
        match current {
            NtripState::Backoff {
                attempt,
                delay_secs,
                reason,
            } => {
                assert_eq!(attempt, 3);
                assert_eq!(delay_secs, 8);
                assert!(reason.contains("Connection"));
            }
            _ => panic!("Expected Backoff state"),
        }
    }

    #[tokio::test]
    async fn test_handle_correction_age_none_when_no_data() {
        let state = Arc::new(Mutex::new(NtripTaskState::default()));
        let handle = NtripTaskHandle {
            state: Arc::clone(&state),
        };

        // No data received yet
        assert!(handle.correction_age_secs().await.is_none());

        // After receiving data
        {
            let mut s = state.lock().await;
            s.last_data_time = Some(Instant::now());
        }

        // Correction age should be very small (just set)
        let age = handle.correction_age_secs().await.unwrap();
        assert!(age < 0.1, "Expected very small age, got {}", age);
    }

    #[tokio::test]
    async fn test_handle_snapshot_captures_all_fields() {
        let state = Arc::new(Mutex::new(NtripTaskState::default()));
        let handle = NtripTaskHandle {
            state: Arc::clone(&state),
        };

        // Set up complex state
        {
            let mut s = state.lock().await;
            s.state = NtripState::Streaming;
            s.bytes_received = 123456;
            s.messages_forwarded = 42;
            s.connection_count = 5;
            s.last_data_time = Some(Instant::now());
        }

        let snapshot = handle.snapshot().await;
        assert!(matches!(snapshot.state, NtripState::Streaming));
        assert_eq!(snapshot.bytes_received, 123456);
        assert_eq!(snapshot.messages_forwarded, 42);
        assert_eq!(snapshot.connection_count, 5);
        assert!(snapshot.last_data_time.is_some());
    }

    #[tokio::test]
    async fn test_rtcm_channel_receiver_dropped() {
        // Create channels where receiver is dropped
        let (rtcm_tx, rtcm_rx) = mpsc::channel(32);
        let (_gga_tx, gga_rx) = watch::channel(None);
        let (msg_tx, _msg_rx) = mpsc::channel(64);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        // Drop the receiver before sending
        drop(rtcm_rx);

        let channels = NtripTaskChannels {
            rtcm_tx,
            gga_rx,
            msg_tx,
            shutdown_rx,
        };

        // Sending should fail but not panic
        let result = channels.rtcm_tx.send(vec![0xD3, 0x00, 0x10]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_message_channel_receiver_dropped() {
        // Create channels where message receiver is dropped
        let (rtcm_tx, _rtcm_rx) = mpsc::channel(32);
        let (_gga_tx, gga_rx) = watch::channel(None);
        let (msg_tx, msg_rx) = mpsc::channel(64);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        // Drop the receiver
        drop(msg_rx);

        let channels = NtripTaskChannels {
            rtcm_tx,
            gga_rx,
            msg_tx,
            shutdown_rx,
        };

        // Sending should fail but not panic
        let result = channels.msg_tx.send(NtripMessage::Connected).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_gga_channel_gets_latest_value() {
        let (gga_tx, gga_rx) = watch::channel(None);

        // Send multiple GGA updates
        gga_tx
            .send(Some(GgaData {
                latitude: 47.0,
                longitude: -122.0,
                altitude: 100.0,
                quality: 4,
                num_satellites: 10,
            }))
            .unwrap();

        gga_tx
            .send(Some(GgaData {
                latitude: 48.0,
                longitude: -123.0,
                altitude: 200.0,
                quality: 5,
                num_satellites: 12,
            }))
            .unwrap();

        // Receiver should see latest value
        let latest = gga_rx.borrow().clone();
        assert!(latest.is_some());
        let gga = latest.unwrap();
        assert!((gga.latitude - 48.0).abs() < 0.001);
        assert!((gga.longitude - (-123.0)).abs() < 0.001);
        assert_eq!(gga.num_satellites, 12);
    }

    #[tokio::test]
    async fn test_shutdown_receiver_initial_value() {
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        // Initial value should be false
        assert!(!*shutdown_rx.borrow());
    }

    #[tokio::test]
    async fn test_shutdown_receiver_detects_signal() {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        // Initial value
        assert!(!*shutdown_rx.borrow());

        // Send shutdown signal
        shutdown_tx.send(true).unwrap();

        // Receiver should detect change
        shutdown_rx.changed().await.unwrap();
        assert!(*shutdown_rx.borrow());
    }

    #[test]
    fn test_backoff_calculation() {
        // Verify backoff calculation (also tested in util.rs, but verifying integration)
        let initial = 1;
        let max = 60;

        assert_eq!(crate::util::calculate_backoff(1, initial, max), 1);
        assert_eq!(crate::util::calculate_backoff(2, initial, max), 2);
        assert_eq!(crate::util::calculate_backoff(3, initial, max), 4);
        assert_eq!(crate::util::calculate_backoff(4, initial, max), 8);
        assert_eq!(crate::util::calculate_backoff(5, initial, max), 16);
        assert_eq!(crate::util::calculate_backoff(6, initial, max), 32);
        assert_eq!(crate::util::calculate_backoff(7, initial, max), 60); // Capped
    }

    #[test]
    fn test_ntrip_error_display_messages() {
        use crate::error::NtripError;

        let auth_err = NtripError::auth_failed("caster.example.com", "testuser");
        let msg = auth_err.to_string();
        assert!(msg.contains("authentication"));
        assert!(msg.contains("testuser"));
        assert!(msg.contains("caster.example.com"));

        let mountpoint_err = NtripError::mountpoint_not_found("caster.example.com", "TESTMOUNT");
        let msg = mountpoint_err.to_string();
        assert!(msg.contains("TESTMOUNT"));
        assert!(msg.contains("not found"));

        let timeout_err = NtripError::Timeout { timeout_secs: 30 };
        let msg = timeout_err.to_string();
        assert!(msg.contains("30"));
        assert!(msg.contains("timed out"));

        let http_err = NtripError::HttpError {
            status: 503,
            message: "Service Unavailable".to_string(),
        };
        let msg = http_err.to_string();
        assert!(msg.contains("503"));
        assert!(msg.contains("Service Unavailable"));
    }

    #[tokio::test]
    async fn test_stats_accumulation() {
        let state = Arc::new(Mutex::new(NtripTaskState::default()));
        let handle = NtripTaskHandle {
            state: Arc::clone(&state),
        };

        // Simulate receiving data in chunks
        for _ in 0..10 {
            let mut s = state.lock().await;
            s.bytes_received += 256;
            s.messages_forwarded += 1;
            s.last_data_time = Some(Instant::now());
        }

        let snapshot = handle.snapshot().await;
        assert_eq!(snapshot.bytes_received, 2560);
        assert_eq!(snapshot.messages_forwarded, 10);
    }

    #[tokio::test]
    async fn test_multiple_reconnections_tracked() {
        let state = Arc::new(Mutex::new(NtripTaskState::default()));
        let handle = NtripTaskHandle {
            state: Arc::clone(&state),
        };

        // Simulate multiple reconnection cycles
        for i in 1..=5 {
            let mut s = state.lock().await;
            s.connection_count = i;
            s.state = NtripState::Streaming;
        }

        let snapshot = handle.snapshot().await;
        assert_eq!(snapshot.connection_count, 5);
    }

    #[test]
    fn test_ntrip_message_debug_format() {
        // Verify Debug impl works for all message variants
        let msgs = vec![
            NtripMessage::StateChanged(NtripState::Streaming),
            NtripMessage::RtcmReceived { bytes: 512 },
            NtripMessage::Connected,
            NtripMessage::Disconnected {
                reason: "timeout".to_string(),
            },
        ];

        for msg in msgs {
            let debug_str = format!("{:?}", msg);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_ntrip_message_clone() {
        let msg = NtripMessage::Disconnected {
            reason: "network error".to_string(),
        };
        let cloned = msg.clone();
        match cloned {
            NtripMessage::Disconnected { reason } => {
                assert_eq!(reason, "network error");
            }
            _ => panic!("Clone changed message variant"),
        }
    }
}
