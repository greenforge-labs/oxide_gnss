//! Device configuration sequence for u-blox GNSS receivers.
//!
//! Handles the configuration sequence on device startup:
//! - Set navigation rate
//! - Enable required UBX messages
//! - Configure RTCM input for corrections
//! - Verify configuration applied via ACK/NAK responses

use std::time::Duration;

use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::config::UbloxConfig;
use crate::error::DeviceError;

use super::serial::SerialPort;
use super::ubx::{
    build_cfg_rst_gnss_restart, build_cfg_valset, AckResult, UbxHandler,
    CFG_VALSET_MAX_KEYS_PER_PACKET,
};

/// Configuration step for tracking progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigStep {
    /// Setting navigation rate
    NavigationRate,
    /// Enabling NAV-PVT message
    EnableNavPvt,
    /// Enabling protocol settings (UBX, RTCM3 input)
    ProtocolSettings,
    /// Configuration complete
    Complete,
}

impl ConfigStep {
    /// Get the step number (1-indexed for display).
    pub fn number(&self) -> u8 {
        match self {
            Self::NavigationRate => 1,
            Self::EnableNavPvt => 2,
            Self::ProtocolSettings => 3,
            Self::Complete => 4,
        }
    }

    /// Get the total number of configuration steps.
    pub const fn total() -> u8 {
        3
    }

    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::NavigationRate => "Setting navigation rate",
            Self::EnableNavPvt => "Enabling NAV-PVT message",
            Self::ProtocolSettings => "Configuring protocols",
            Self::Complete => "Configuration complete",
        }
    }
}

/// Configuration options for the configurator.
#[derive(Debug, Clone)]
pub struct ConfiguratorOptions {
    /// Timeout for waiting for ACK/NAK response
    pub ack_timeout: Duration,
    /// Number of retries per configuration step
    pub max_retries: u8,
    /// Delay between retries
    pub retry_delay: Duration,
    /// Whether to persist configuration to BBR
    pub persist: bool,
}

impl Default for ConfiguratorOptions {
    fn default() -> Self {
        Self {
            ack_timeout: Duration::from_millis(500),
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
            persist: false, // Don't persist by default, configure on each startup
        }
    }
}

/// Handles device configuration sequence.
pub struct DeviceConfigurator {
    options: ConfiguratorOptions,
}

impl DeviceConfigurator {
    /// Create a new configurator with default options.
    pub fn new() -> Self {
        Self {
            options: ConfiguratorOptions::default(),
        }
    }

    /// Create a configurator with custom options.
    pub fn with_options(options: ConfiguratorOptions) -> Self {
        Self { options }
    }

    /// Run the full configuration sequence.
    ///
    /// Takes a resolved UbloxConfig (from mode + features or legacy config).
    /// If `enabled_topics` is provided, validation only checks those topics.
    ///
    /// Returns Ok on success, or an error describing what failed.
    pub async fn configure(
        &self,
        serial: &mut SerialPort,
        ubx: &mut UbxHandler,
        ublox_config: &UbloxConfig,
        enabled_topics: Option<&[&str]>,
    ) -> Result<(), DeviceError> {
        info!("Starting device configuration sequence");

        // Validate config and emit warnings (mode-aware if enabled_topics provided)
        let validation = ublox_config.validate(enabled_topics);

        // Log topic availability warnings (only for enabled topics)
        for (topic, missing_msgs) in ublox_config.check_topic_availability(enabled_topics) {
            warn!(
                topic = topic,
                missing = ?missing_msgs,
                "ROS topic may be unavailable or degraded due to missing UBX messages"
            );
        }

        // Fail on missing essential messages (driver won't work correctly)
        if !validation.is_valid() {
            return Err(DeviceError::ConfigurationFailed {
                step: format!(
                    "Essential UBX messages missing: {:?}",
                    validation
                        .missing_essential
                        .iter()
                        .map(|m| m.message)
                        .collect::<Vec<_>>()
                ),
            });
        }

        // Build CfgVal list from the structured config
        let cfg_vals = super::cfg_key_mapping::build_cfg_vals_from_config(ublox_config);

        info!(
            "Using u-blox config from YAML ({} settings)",
            cfg_vals.len()
        );

        // F9P's CFG-VALSET limit is 64 keys per packet; send in chunks and
        // await ACK for each. Most modes fit in one packet; enumerate-and-zero
        // runs can push us over.
        let chunks: Vec<&[ublox::cfg_val::CfgVal]> =
            cfg_vals.chunks(CFG_VALSET_MAX_KEYS_PER_PACKET).collect();
        let n_chunks = chunks.len();
        if n_chunks > 1 {
            debug!(chunks = n_chunks, "CFG-VALSET split into multiple packets");
        }
        for (i, chunk) in chunks.iter().enumerate() {
            debug!(
                chunk = i + 1,
                of = n_chunks,
                keys = chunk.len(),
                "Sending CFG-VALSET chunk"
            );
            self.configure_step(serial, ubx, ConfigStep::EnableNavPvt, chunk)
                .await?;
        }

        info!(
            "Device configuration complete ({} values set in {} packet{})",
            cfg_vals.len(),
            n_chunks,
            if n_chunks == 1 { "" } else { "s" }
        );

        // Signal/constellation changes in CFG-SIGNAL-* don't take effect on the
        // F9P until the GNSS engine restarts. Without this, the receiver keeps
        // tracking sats from constellations we just disabled and can't hit the
        // configured nav rate. CFG-RST is fire-and-forget (no ACK from F9P),
        // then the receiver needs a few seconds to reacquire before we mark it
        // Active.
        let rst_packet = build_cfg_rst_gnss_restart();
        debug!(
            packet_len = rst_packet.len(),
            "Sending CFG-RST (GNSS-only software reset)"
        );
        serial.write(&rst_packet).await?;
        info!("Issued UBX-CFG-RST (GNSS-only); waiting 3s for receiver to reacquire");
        tokio::time::sleep(Duration::from_secs(3)).await;

        Ok(())
    }

    /// Execute a single configuration step with retry logic.
    async fn configure_step(
        &self,
        serial: &mut SerialPort,
        ubx: &mut UbxHandler,
        step: ConfigStep,
        cfg_values: &[ublox::cfg_val::CfgVal],
    ) -> Result<(), DeviceError> {
        debug!(step = ?step, "Executing configuration step: {}", step.description());

        for attempt in 0..=self.options.max_retries {
            if attempt > 0 {
                warn!(
                    step = ?step,
                    attempt = attempt,
                    "Retrying configuration step"
                );
                tokio::time::sleep(self.options.retry_delay).await;
            }

            // Clear any pending ACK state and set expectation for CFG-VALSET response
            ubx.clear_pending_ack();
            // CFG-VALSET: class = 0x06, msg_id = 0x8A
            ubx.expect_ack(0x06, 0x8A);

            // Build and send the configuration packet
            let packet = build_cfg_valset(cfg_values, self.options.persist);
            debug!(step = ?step, packet_len = packet.len(), "Sending CFG-VALSET packet");
            serial.write(&packet).await?;

            // Wait for ACK/NAK
            match self.wait_for_ack(serial, ubx).await {
                Ok(AckResult::Ack) => {
                    debug!(step = ?step, "Configuration step acknowledged");
                    return Ok(());
                }
                Ok(AckResult::Nak) => {
                    // NAK is a deterministic rejection - do not retry
                    error!(step = ?step, "Configuration step NAK'd by device - failing immediately (no retry)");
                    return Err(DeviceError::ConfigurationFailed {
                        step: format!("{} (NAK received)", step.description()),
                    });
                }
                Err(e) => {
                    warn!(step = ?step, error = %e, attempt = attempt + 1, max = self.options.max_retries + 1, "Timeout waiting for ACK, will retry");
                    // Continue to retry on timeout
                }
            }
        }

        Err(DeviceError::ConfigurationFailed {
            step: format!(
                "{} (timeout after {} retries)",
                step.description(),
                self.options.max_retries
            ),
        })
    }

    /// Wait for an ACK or NAK response from the device.
    async fn wait_for_ack(
        &self,
        serial: &mut SerialPort,
        ubx: &mut UbxHandler,
    ) -> Result<AckResult, DeviceError> {
        let mut buf = [0u8; 256];

        let result = timeout(self.options.ack_timeout, async {
            loop {
                // Read data from serial
                let n = serial.read(&mut buf).await?;
                if n > 0 {
                    debug!(bytes_received = n, "Data received during ACK wait");
                    // Process through UBX handler
                    let result = ubx.process(&buf[..n]);

                    // Check if we got an ACK/NAK
                    if let Some(ack) = result.ack {
                        debug!(ack = ?ack, "ACK result received");
                        return Ok::<AckResult, DeviceError>(ack);
                    }
                }
            }
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_) => Err(DeviceError::Timeout {
                operation: "waiting for ACK".to_string(),
                timeout_ms: self.options.ack_timeout.as_millis() as u64,
            }),
        }
    }
}

impl Default for DeviceConfigurator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_step_numbers() {
        assert_eq!(ConfigStep::NavigationRate.number(), 1);
        assert_eq!(ConfigStep::EnableNavPvt.number(), 2);
        assert_eq!(ConfigStep::ProtocolSettings.number(), 3);
        assert_eq!(ConfigStep::Complete.number(), 4);
    }

    #[test]
    fn test_config_step_total() {
        assert_eq!(ConfigStep::total(), 3);
    }

    #[test]
    fn test_configurator_options_default() {
        let opts = ConfiguratorOptions::default();
        assert_eq!(opts.ack_timeout, Duration::from_millis(500));
        assert_eq!(opts.max_retries, 3);
        assert!(!opts.persist);
    }

    #[test]
    fn test_configurator_creation() {
        let _conf = DeviceConfigurator::new();
        let _conf2 = DeviceConfigurator::with_options(ConfiguratorOptions {
            persist: true,
            ..Default::default()
        });
    }
}
