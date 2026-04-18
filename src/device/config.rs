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
    build_cfg_rst_gnss_restart, build_cfg_valget, build_cfg_valset, build_cfg_valset_all_layers,
    AckResult, CfgValGetResponse, UbxHandler, CFG_VALSET_MAX_KEYS_PER_PACKET,
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

/// The four CFG keys that together encode the 32-byte USB serial string on
/// a u-blox ZED-F9P. Each is a `u64` (8 bytes, little-endian).
pub const USB_SERIAL_KEY_IDS: [u32; 4] = [0x5065_0015, 0x5065_0016, 0x5065_0017, 0x5065_0018];

/// Send a CFG-VALGET request for `key_ids` and wait for the matching response.
///
/// `layer` selects the config layer: 0 = RAM, 1 = BBR, 2 = FLASH, 7 = Default.
/// Returns the parsed response (key → little-endian value bytes) or
/// [`DeviceError::Timeout`] if nothing arrives within `timeout_duration`.
pub async fn query_cfg_valget(
    serial: &mut SerialPort,
    ubx: &mut UbxHandler,
    key_ids: &[u32],
    layer: u8,
    timeout_duration: Duration,
) -> Result<CfgValGetResponse, DeviceError> {
    let packet = build_cfg_valget(key_ids, layer);
    serial.write(&packet).await?;

    let mut buf = [0u8; 256];
    let result = timeout(timeout_duration, async {
        loop {
            let n = serial.read(&mut buf).await?;
            if n > 0 {
                let result = ubx.process(&buf[..n]);
                if let Some(resp) = result.valget {
                    return Ok::<CfgValGetResponse, DeviceError>(resp);
                }
            }
        }
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(DeviceError::Timeout {
            operation: "CFG-VALGET response".to_string(),
            timeout_ms: timeout_duration.as_millis() as u64,
        }),
    }
}

/// Assemble the 32-byte USB serial string from a CFG-VALGET response.
///
/// The four `UsbSerialNoStr*` keys each hold 8 bytes. We concatenate them
/// in key order, strip trailing NUL padding, and return as UTF-8. Returns
/// `None` if any chunk is missing or not 8 bytes long, or if the bytes
/// are not valid UTF-8.
pub fn assemble_usb_serial_string(resp: &CfgValGetResponse) -> Option<String> {
    let mut bytes = Vec::with_capacity(32);
    for key_id in USB_SERIAL_KEY_IDS {
        let chunk = resp.get(&key_id)?;
        if chunk.len() != 8 {
            return None;
        }
        bytes.extend_from_slice(chunk);
    }
    let trimmed = bytes.split(|b| *b == 0).next().unwrap_or(&[]);
    String::from_utf8(trimmed.to_vec()).ok()
}

/// Split an ASCII serial string into four `UsbSerialNoStr{0..3}` CfgVal
/// entries, each a little-endian `u64`. Trailing bytes are zero-padded.
///
/// Returns `None` if the string is longer than 32 bytes or non-ASCII —
/// both are rejected by `DeviceConfig::validate`, so this is a belt-and-
/// braces check.
pub fn chunk_usb_serial_to_cfg_vals(serial_str: &str) -> Option<Vec<ublox::cfg_val::CfgVal>> {
    if !serial_str.is_ascii() || serial_str.len() > 32 {
        return None;
    }
    let mut padded = [0u8; 32];
    padded[..serial_str.len()].copy_from_slice(serial_str.as_bytes());

    let mut chunks = [0u64; 4];
    for (i, chunk) in chunks.iter_mut().enumerate() {
        let offset = i * 8;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&padded[offset..offset + 8]);
        *chunk = u64::from_le_bytes(arr);
    }

    Some(vec![
        ublox::cfg_val::CfgVal::UsbSerialNoStr0(chunks[0]),
        ublox::cfg_val::CfgVal::UsbSerialNoStr1(chunks[1]),
        ublox::cfg_val::CfgVal::UsbSerialNoStr2(chunks[2]),
        ublox::cfg_val::CfgVal::UsbSerialNoStr3(chunks[3]),
    ])
}

/// Probe the F9P's USB serial string and restore it to `desired` (persisted
/// to RAM + BBR + FLASH) if the current value is blank.
///
/// This recovers udev symlinks like `/dev/gnss_f9p_rover` after a factory
/// reset without requiring manual `ubxtool` intervention. The change only
/// takes effect on the next USB re-enumeration — the driver continues to
/// talk to the F9P on its current ttyACM until then.
///
/// Best-effort: a failed probe or NAK is logged and then ignored. We never
/// fail startup on this path.
pub async fn maybe_restore_usb_serial(
    serial: &mut SerialPort,
    ubx: &mut UbxHandler,
    desired: &str,
) {
    let current = match query_cfg_valget(
        serial,
        ubx,
        &USB_SERIAL_KEY_IDS,
        /*layer = RAM*/ 0,
        Duration::from_millis(500),
    )
    .await
    {
        Ok(resp) => assemble_usb_serial_string(&resp),
        Err(e) => {
            warn!(error = %e, "CFG-VALGET for USB serial failed — skipping restore (not fatal)");
            return;
        }
    };

    match current.as_deref() {
        Some("") | None => {
            warn!(
                desired = desired,
                "F9P USB serial is blank — restoring. Unplug/replug for USB re-enumeration to take effect."
            );

            let Some(cfg_vals) = chunk_usb_serial_to_cfg_vals(desired) else {
                warn!("usb_serial is not ASCII or >32 bytes — refusing to restore");
                return;
            };

            let packet = build_cfg_valset_all_layers(&cfg_vals);
            ubx.clear_pending_ack();
            ubx.expect_ack(0x06, 0x8A);
            if let Err(e) = serial.write(&packet).await {
                warn!(error = %e, "failed to send CFG-VALSET for USB serial restore");
                return;
            }

            let mut buf = [0u8; 256];
            let ack = timeout(Duration::from_millis(500), async {
                loop {
                    let n = serial.read(&mut buf).await?;
                    if n > 0 {
                        let result = ubx.process(&buf[..n]);
                        if let Some(ack) = result.ack {
                            return Ok::<AckResult, DeviceError>(ack);
                        }
                    }
                }
            })
            .await;

            match ack {
                Ok(Ok(AckResult::Ack)) => {
                    info!(
                        serial = desired,
                        "USB serial string restored (RAM+BBR+FLASH)"
                    );
                }
                Ok(Ok(AckResult::Nak)) => {
                    warn!("F9P NAK'd CFG-VALSET for USB serial restore");
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "read error while awaiting USB serial restore ACK");
                }
                Err(_) => {
                    warn!("timeout waiting for USB serial restore ACK");
                }
            }
        }
        Some(s) => {
            debug!(current = s, "F9P USB serial present — no restore needed");
        }
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
    fn test_usb_serial_chunk_roundtrip_via_valget_bytes() {
        // Chunk a serial string into four u64 CfgVals, encode each value
        // back to bytes, stuff them into a CfgValGetResponse, then
        // reassemble. The reassembled string should equal the original.
        let original = "gnss_f9p_rover_42";
        let vals = chunk_usb_serial_to_cfg_vals(original).expect("valid string");
        assert_eq!(vals.len(), 4);

        let mut resp = CfgValGetResponse::new();
        for (key, val) in USB_SERIAL_KEY_IDS.iter().zip(vals.iter()) {
            let raw = match val {
                ublox::cfg_val::CfgVal::UsbSerialNoStr0(v)
                | ublox::cfg_val::CfgVal::UsbSerialNoStr1(v)
                | ublox::cfg_val::CfgVal::UsbSerialNoStr2(v)
                | ublox::cfg_val::CfgVal::UsbSerialNoStr3(v) => *v,
                _ => panic!("unexpected CfgVal variant"),
            };
            resp.insert(*key, raw.to_le_bytes().to_vec());
        }

        let reassembled = assemble_usb_serial_string(&resp).expect("reassembles");
        assert_eq!(reassembled, original);
    }

    #[test]
    fn test_usb_serial_blank_response_assembles_to_empty() {
        let mut resp = CfgValGetResponse::new();
        for key in USB_SERIAL_KEY_IDS {
            resp.insert(key, vec![0u8; 8]);
        }
        assert_eq!(assemble_usb_serial_string(&resp).as_deref(), Some(""));
    }

    #[test]
    fn test_usb_serial_rejects_non_ascii_and_oversize() {
        assert!(chunk_usb_serial_to_cfg_vals("café").is_none());
        assert!(chunk_usb_serial_to_cfg_vals(&"x".repeat(33)).is_none());
        assert!(chunk_usb_serial_to_cfg_vals("rover").is_some());
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
