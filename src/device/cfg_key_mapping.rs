//! Mapping from structured u-blox config to ublox crate CfgVal enum variants.
//!
//! This module provides translation between the structured YAML configuration
//! and the `ublox` crate's `CfgVal` enum variants.

use tracing::warn;
use ublox::cfg_val::CfgVal;

use crate::config::UbloxConfig;

/// Build a list of CfgVal from the structured UbloxConfig.
pub fn build_cfg_vals_from_config(config: &UbloxConfig) -> Vec<CfgVal> {
    let mut vals = Vec::new();

    // Rate settings
    vals.push(CfgVal::RateMeas(config.rate.measurement_ms));
    vals.push(CfgVal::RateNav(config.rate.nav_ratio));

    // Protocol settings - USB
    build_port_protocol_cfg_vals("usb", &config.protocols.usb, &mut vals);

    // Protocol settings - UART1
    build_port_protocol_cfg_vals("uart1", &config.protocols.uart1, &mut vals);

    // Protocol settings - UART2
    build_port_protocol_cfg_vals("uart2", &config.protocols.uart2, &mut vals);

    // Protocol settings - I2C
    build_port_protocol_cfg_vals("i2c", &config.protocols.i2c, &mut vals);

    // Protocol settings - SPI
    build_port_protocol_cfg_vals("spi", &config.protocols.spi, &mut vals);

    // Port settings - UART1
    if let Some(enabled) = config.ports.uart1.enabled {
        vals.push(CfgVal::Uart1Enabled(enabled));
    }
    if let Some(baudrate) = config.ports.uart1.baudrate {
        vals.push(CfgVal::Uart1Baudrate(baudrate));
    }

    // Port settings - UART2
    if let Some(enabled) = config.ports.uart2.enabled {
        vals.push(CfgVal::Uart2Enabled(enabled));
    }
    if let Some(baudrate) = config.ports.uart2.baudrate {
        vals.push(CfgVal::Uart2Baudrate(baudrate));
    }

    // Port settings - I2C
    if let Some(enabled) = config.ports.i2c_enabled {
        vals.push(CfgVal::I2cEnabled(enabled));
    }

    // Port settings - SPI
    if let Some(enabled) = config.ports.spi_enabled {
        vals.push(CfgVal::SpiEnabled(enabled));
    }

    // Signal/constellation settings
    build_signal_cfg_vals(&config.signals, &mut vals);

    // Navigation engine settings (CFG-NAVSPG-*)
    if let Some(pl_ena) = config.nav_spg.pl_ena {
        vals.push(CfgVal::NavSpgPlEna(pl_ena));
    }

    // Dynamic platform model
    if let Some(dynamic_model) = config.nav_spg.dynamic_model {
        // Convert our DynamicModel enum to ublox NavDynamicModel
        let ublox_model = match dynamic_model {
            crate::config::DynamicModel::Portable => ublox::NavDynamicModel::Portable,
            crate::config::DynamicModel::Stationary => ublox::NavDynamicModel::Stationary,
            crate::config::DynamicModel::Pedestrian => ublox::NavDynamicModel::Pedestrian,
            crate::config::DynamicModel::Automotive => ublox::NavDynamicModel::Automotive,
            crate::config::DynamicModel::Sea => ublox::NavDynamicModel::Sea,
            crate::config::DynamicModel::AirborneLight => {
                ublox::NavDynamicModel::AirborneWithLess1gAcceleration
            }
            crate::config::DynamicModel::AirborneMedium => {
                ublox::NavDynamicModel::AirborneWithLess2gAcceleration
            }
            crate::config::DynamicModel::AirborneHigh => {
                ublox::NavDynamicModel::AirborneWithLess4gAcceleration
            }
            // Models not directly supported in ublox crate - use raw value via Portable + warning
            // The F9P firmware supports these but ublox-rs may not have them yet
            crate::config::DynamicModel::Wrist => {
                warn!("Wrist dynamic model may require newer ublox crate; using raw value 9");
                // Fall back to a supported model that's closest, or we could use unchecked
                ublox::NavDynamicModel::Portable
            }
            crate::config::DynamicModel::Bike => {
                warn!("Bike dynamic model may require newer ublox crate; using raw value 10");
                ublox::NavDynamicModel::Portable
            }
            crate::config::DynamicModel::Mower => {
                warn!(
                    "Mower dynamic model (11) not in ublox crate; defaulting to Automotive"
                );
                ublox::NavDynamicModel::Automotive
            }
            crate::config::DynamicModel::Escooter => {
                warn!(
                    "E-scooter dynamic model (12) not in ublox crate; defaulting to Automotive"
                );
                ublox::NavDynamicModel::Automotive
            }
            crate::config::DynamicModel::Robot => {
                warn!(
                    "Robot dynamic model (13) not in ublox crate; defaulting to Automotive"
                );
                ublox::NavDynamicModel::Automotive
            }
        };
        vals.push(CfgVal::NavSpgDynModel(ublox_model));
    }

    // Elevation mask (minimum satellite elevation in degrees)
    if let Some(elev) = config.nav_spg.elevation_mask {
        vals.push(CfgVal::NavSpgInfilMinElev(elev));
    }

    // PDOP mask (position dilution of precision threshold)
    if let Some(pdop) = config.nav_spg.pdop_mask {
        // PDOP is stored as u16 scaled by 10 (e.g., 6.0 -> 60)
        let pdop_scaled = (pdop * 10.0).round() as u16;
        vals.push(CfgVal::NavSpgOutfilPdop(pdop_scaled));
    }

    // Timepulse (PPS) configuration (CFG-TP-*)
    if let Some(ref tp) = config.timepulse {
        build_timepulse_cfg_vals(tp, &mut vals);
    }

    // Message output rates
    for (msg, rate) in &config.messages.usb {
        if let Some(val) = parse_message_rate("usb", msg, *rate) {
            vals.push(val);
        }
    }
    for (msg, rate) in &config.messages.uart1 {
        if let Some(val) = parse_message_rate("uart1", msg, *rate) {
            vals.push(val);
        }
    }
    for (msg, rate) in &config.messages.uart2 {
        if let Some(val) = parse_message_rate("uart2", msg, *rate) {
            vals.push(val);
        }
    }

    vals
}

/// Build CfgVal entries for signal/constellation configuration.
fn build_signal_cfg_vals(signals: &crate::config::SignalConfig, vals: &mut Vec<CfgVal>) {
    // GPS
    if let Some(enabled) = signals.gps.enabled {
        vals.push(CfgVal::SignalGpsEna(enabled));
    }
    if let Some(l1) = signals.gps.l1 {
        vals.push(CfgVal::SignalGpsL1caEna(l1));
    }
    if let Some(l2) = signals.gps.l2 {
        vals.push(CfgVal::SignalGpsL2cEna(l2));
    }

    // GLONASS
    if let Some(enabled) = signals.glonass.enabled {
        vals.push(CfgVal::SignalGloEna(enabled));
    }
    if let Some(l1) = signals.glonass.l1 {
        vals.push(CfgVal::SignalGloL1Ena(l1));
    }
    if let Some(l2) = signals.glonass.l2 {
        vals.push(CfgVal::SignalGLoL2Ena(l2));
    }

    // Galileo
    if let Some(enabled) = signals.galileo.enabled {
        vals.push(CfgVal::SignalGalEna(enabled));
    }
    if let Some(l1) = signals.galileo.l1 {
        vals.push(CfgVal::SignalGalE1Ena(l1));
    }
    if let Some(l2) = signals.galileo.l2 {
        vals.push(CfgVal::SignalGalE5bEna(l2));
    }

    // BeiDou
    if let Some(enabled) = signals.beidou.enabled {
        vals.push(CfgVal::SignalBdsEna(enabled));
    }
    if let Some(b1) = signals.beidou.b1 {
        vals.push(CfgVal::SignalBdsB1Ena(b1));
    }
    if let Some(b2) = signals.beidou.b2 {
        vals.push(CfgVal::SignalBdsB2Ena(b2));
    }

    // SBAS - not all variants available in ublox crate
    // SignalSbasEna and SignalSbasL1Ena not available

    // QZSS
    if let Some(enabled) = signals.qzss.enabled {
        vals.push(CfgVal::SignalQzssEna(enabled));
    }
    if let Some(l1ca) = signals.qzss.l1ca {
        vals.push(CfgVal::SignalQzssL1caEna(l1ca));
    }
    // SignalQzssL1sEna not available in ublox crate
    if let Some(l2c) = signals.qzss.l2c {
        vals.push(CfgVal::SignalQzssL2cEna(l2c));
    }
}

/// Build CfgVal entries for timepulse (PPS) configuration.
fn build_timepulse_cfg_vals(tp: &crate::config::TimepulseConfig, vals: &mut Vec<CfgVal>) {
    // Enable/disable timepulse output
    if let Some(enabled) = tp.enabled {
        vals.push(CfgVal::TpTp1Ena(enabled));
    }

    // Frequency when locked (primary setting)
    if let Some(freq) = tp.frequency_hz {
        vals.push(CfgVal::TpFreqLockTp1(freq));
        // Also set unlocked frequency if not specified separately
        if tp.frequency_unlocked_hz.is_none() {
            vals.push(CfgVal::TpFreqTp1(freq));
        }
    }

    // Frequency when unlocked (before GNSS lock)
    if let Some(freq) = tp.frequency_unlocked_hz {
        vals.push(CfgVal::TpFreqTp1(freq));
    }

    // Pulse length when locked (in microseconds)
    if let Some(len_us) = tp.pulse_length_us {
        vals.push(CfgVal::TpLenLockTp1(len_us));
        // Also set unlocked length if not specified separately
        if tp.pulse_length_unlocked_us.is_none() {
            vals.push(CfgVal::TpLenTp1(len_us));
        }
    }

    // Pulse length when unlocked
    if let Some(len_us) = tp.pulse_length_unlocked_us {
        vals.push(CfgVal::TpLenTp1(len_us));
    }

    // Polarity (true = rising edge at time mark)
    if let Some(polarity) = tp.polarity {
        let pol_val = match polarity {
            crate::config::TimepulsePolarity::Rising => true,
            crate::config::TimepulsePolarity::Falling => false,
        };
        vals.push(CfgVal::TpPolTp1(pol_val));
    }

    // Time grid alignment
    if let Some(time_grid) = tp.time_grid {
        let grid = match time_grid {
            crate::config::TimeGrid::Utc => ublox::AlignmentToReferenceTime::Utc,
            crate::config::TimeGrid::Gps => ublox::AlignmentToReferenceTime::Gps,
            crate::config::TimeGrid::Glonass => ublox::AlignmentToReferenceTime::Glo,
            crate::config::TimeGrid::Beidou => ublox::AlignmentToReferenceTime::Bds,
            crate::config::TimeGrid::Galileo => ublox::AlignmentToReferenceTime::Gal,
        };
        vals.push(CfgVal::TpTimegridTp1(grid));
    }

    // Align to top of second
    if let Some(align) = tp.align_to_tow {
        vals.push(CfgVal::TpAlignToTowTp1(align));
    }

    // Use locked parameters when GNSS is available
    if let Some(use_locked) = tp.use_locked_params {
        vals.push(CfgVal::TpUseLockedTp1(use_locked));
    }

    // Sync to GNSS time
    if let Some(sync) = tp.sync_to_gnss {
        vals.push(CfgVal::TpSyncGnssTp1(sync));
    }

    // Antenna cable delay compensation
    if let Some(delay_ns) = tp.cable_delay_ns {
        vals.push(CfgVal::TpAntCableDelay(delay_ns));
    }

    // Set pulse definition to frequency mode (not period)
    // This is needed when using frequency_hz settings
    if tp.frequency_hz.is_some() || tp.frequency_unlocked_hz.is_some() {
        vals.push(CfgVal::TpPulseDef(ublox::TpPulse::Freq));
    }

    // Set pulse length definition to absolute length (not ratio)
    // This is needed when using pulse_length_us settings
    if tp.pulse_length_us.is_some() || tp.pulse_length_unlocked_us.is_some() {
        vals.push(CfgVal::TpPulseLengthDef(ublox::TpPulseLength::Length));
    }
}

/// Build CfgVal entries for a port's protocol configuration.
fn build_port_protocol_cfg_vals(
    port: &str,
    protocols: &crate::config::PortProtocols,
    vals: &mut Vec<CfgVal>,
) {
    // Input protocols
    if let Some(v) = protocols.in_ubx {
        if let Some(cfg) = protocol_in_cfg_val(port, "ubx", v) {
            vals.push(cfg);
        }
    }
    if let Some(v) = protocols.in_nmea {
        if let Some(cfg) = protocol_in_cfg_val(port, "nmea", v) {
            vals.push(cfg);
        }
    }
    if let Some(v) = protocols.in_rtcm3x {
        if let Some(cfg) = protocol_in_cfg_val(port, "rtcm3x", v) {
            vals.push(cfg);
        }
    }
    if let Some(v) = protocols.in_spartn {
        if let Some(cfg) = protocol_in_cfg_val(port, "spartn", v) {
            vals.push(cfg);
        }
    }

    // Output protocols
    if let Some(v) = protocols.out_ubx {
        if let Some(cfg) = protocol_out_cfg_val(port, "ubx", v) {
            vals.push(cfg);
        }
    }
    if let Some(v) = protocols.out_nmea {
        if let Some(cfg) = protocol_out_cfg_val(port, "nmea", v) {
            vals.push(cfg);
        }
    }
    if let Some(v) = protocols.out_rtcm3x {
        if let Some(cfg) = protocol_out_cfg_val(port, "rtcm3x", v) {
            vals.push(cfg);
        }
    }
}

/// Get input protocol CfgVal for a port.
fn protocol_in_cfg_val(port: &str, protocol: &str, enabled: bool) -> Option<CfgVal> {
    match (port, protocol) {
        // USB
        ("usb", "ubx") => Some(CfgVal::UsbInProtUbx(enabled)),
        ("usb", "nmea") => Some(CfgVal::UsbInProtNmea(enabled)),
        ("usb", "rtcm3x") => Some(CfgVal::UsbInProtRtcm3x(enabled)),
        // UART1
        ("uart1", "ubx") => Some(CfgVal::Uart1InProtUbx(enabled)),
        ("uart1", "nmea") => Some(CfgVal::Uart1InProtNmea(enabled)),
        ("uart1", "rtcm3x") => Some(CfgVal::Uart1InProtRtcm3x(enabled)),
        // UART2
        ("uart2", "ubx") => Some(CfgVal::Uart2InProtUbx(enabled)),
        ("uart2", "nmea") => Some(CfgVal::Uart2InProtNmea(enabled)),
        ("uart2", "rtcm3x") => Some(CfgVal::Uart2InProtRtcm3x(enabled)),
        // I2C (DDC)
        ("i2c", "ubx") => Some(CfgVal::I2cInProtUbx(enabled)),
        ("i2c", "nmea") => Some(CfgVal::I2cInProtNmea(enabled)),
        ("i2c", "rtcm3x") => Some(CfgVal::I2cInProtRtcm3x(enabled)),
        ("i2c", "spartn") => Some(CfgVal::I2cInProtSpartn(enabled)),
        // SPI
        ("spi", "ubx") => Some(CfgVal::SpiInProtUbx(enabled)),
        ("spi", "nmea") => Some(CfgVal::SpiInProtNmea(enabled)),
        ("spi", "rtcm3x") => Some(CfgVal::SpiInProtRtcm3x(enabled)),
        ("spi", "spartn") => Some(CfgVal::SpiInProtSpartn(enabled)),
        _ => {
            warn!("Unknown input protocol '{}' for port '{}'", protocol, port);
            None
        }
    }
}

/// Get output protocol CfgVal for a port.
fn protocol_out_cfg_val(port: &str, protocol: &str, enabled: bool) -> Option<CfgVal> {
    match (port, protocol) {
        // USB
        ("usb", "ubx") => Some(CfgVal::UsbOutProtUbx(enabled)),
        ("usb", "nmea") => Some(CfgVal::UsbOutProtNmea(enabled)),
        ("usb", "rtcm3x") => Some(CfgVal::UsbOutProtRtcm3x(enabled)),
        // UART1
        ("uart1", "ubx") => Some(CfgVal::Uart1OutProtUbx(enabled)),
        ("uart1", "nmea") => Some(CfgVal::Uart1OutProtNmea(enabled)),
        ("uart1", "rtcm3x") => Some(CfgVal::Uart1OutProtRtcm3x(enabled)),
        // UART2
        ("uart2", "ubx") => Some(CfgVal::Uart2OutProtUbx(enabled)),
        ("uart2", "nmea") => Some(CfgVal::Uart2OutProtNmea(enabled)),
        ("uart2", "rtcm3x") => Some(CfgVal::Uart2OutProtRtcm3x(enabled)),
        // I2C (DDC)
        ("i2c", "ubx") => Some(CfgVal::I2cOutProtUbx(enabled)),
        ("i2c", "nmea") => Some(CfgVal::I2cOutProtNmea(enabled)),
        ("i2c", "rtcm3x") => Some(CfgVal::I2cOutProtRtcm3x(enabled)),
        // SPI
        ("spi", "ubx") => Some(CfgVal::SpiOutProtUbx(enabled)),
        ("spi", "nmea") => Some(CfgVal::SpiOutProtNmea(enabled)),
        ("spi", "rtcm3x") => Some(CfgVal::SpiOutProtRtcm3x(enabled)),
        _ => {
            warn!("Unknown output protocol '{}' for port '{}'", protocol, port);
            None
        }
    }
}

/// Parse message output rate setting.
fn parse_message_rate(port: &str, msg: &str, rate: u8) -> Option<CfgVal> {
    match (port, msg) {
        // ====== USB messages ======
        ("usb", "NAV_PVT") => Some(CfgVal::MsgOutUbxNavPvtUsb(rate)),
        ("usb", "NAV_HPPOSLLH") => Some(CfgVal::MsgOutUbxNavHpPosLlhUsb(rate)),
        ("usb", "NAV_HPPOSECEF") => Some(CfgVal::MsgOutUbxNavHpPosEcefUsb(rate)),
        ("usb", "NAV_SAT") => Some(CfgVal::MsgOutUbxNavSatUsb(rate)),
        ("usb", "NAV_SIG") => Some(CfgVal::MsgOutUbxNavSigUsb(rate)),
        ("usb", "NAV_STATUS") => Some(CfgVal::MsgOutUbxNavStatusUsb(rate)),
        ("usb", "NAV_DOP") => Some(CfgVal::MsgOutUbxNavDopUsb(rate)),
        ("usb", "NAV_CLOCK") => Some(CfgVal::MsgOutUbxNavClockUsb(rate)),
        ("usb", "NAV_EOE") => Some(CfgVal::MsgOutUbxNavEoeUsb(rate)),
        ("usb", "NAV_POSLLH") => Some(CfgVal::MsgOutUbxNavPosLlhUsb(rate)),
        ("usb", "NAV_POSECEF") => Some(CfgVal::MsgOutUbxNavPosEcefUsb(rate)),
        ("usb", "NAV_ODO") => Some(CfgVal::MsgOutUbxNavOdoUsb(rate)),
        ("usb", "NAV_COV") => Some(CfgVal::MsgOutUbxNavCovUsb(rate)),
        ("usb", "NAV_RELPOSNED") => Some(CfgVal::MsgOutUbxNavRelposNedUsb(rate)),
        ("usb", "NAV_PL") => Some(CfgVal::MsgOutUbxNavPlUsb(rate)),
        ("usb", "MON_RF") => Some(CfgVal::MsgOutUbxMonRfUsb(rate)),
        ("usb", "MON_COMMS") => Some(CfgVal::MsgOutUbxMoncommsUsb(rate)),
        ("usb", "MON_HW") => Some(CfgVal::MsgOutUbxMonHwUsb(rate)),
        ("usb", "SEC_SIG") => Some(CfgVal::MsgOutUbxSecSigUsb(rate)),
        ("usb", "SEC_SIGLOG") => Some(CfgVal::MsgOutUbxSecSiglogUsb(rate)),
        ("usb", "RXM_COR") => Some(CfgVal::MsgOutUbxRxmCorUsb(rate)),

        // ====== UART1 messages ======
        ("uart1", "NAV_PVT") => Some(CfgVal::MsgOutUbxNavPvtUart1(rate)),
        ("uart1", "NAV_HPPOSLLH") => Some(CfgVal::MsgOutUbxNavHpPosLlhUart1(rate)),
        ("uart1", "NAV_HPPOSECEF") => Some(CfgVal::MsgOutUbxNavHpPosEcefUart1(rate)),
        ("uart1", "NAV_SAT") => Some(CfgVal::MsgOutUbxNavSatUart1(rate)),
        ("uart1", "NAV_SIG") => Some(CfgVal::MsgOutUbxNavSigUart1(rate)),
        ("uart1", "NAV_STATUS") => Some(CfgVal::MsgOutUbxNavStatusUart1(rate)),
        ("uart1", "NAV_DOP") => Some(CfgVal::MsgOutUbxNavDopUart1(rate)),
        ("uart1", "NAV_POSLLH") => Some(CfgVal::MsgOutUbxNavPosLlhUart1(rate)),
        ("uart1", "NAV_POSECEF") => Some(CfgVal::MsgOutUbxNavPosEcefUart1(rate)),
        ("uart1", "NAV_COV") => Some(CfgVal::MsgOutUbxNavCovUart1(rate)),
        ("uart1", "NAV_RELPOSNED") => Some(CfgVal::MsgOutUbxNavRelposNedUart1(rate)),
        ("uart1", "NAV_PL") => Some(CfgVal::MsgOutUbxNavPlUart1(rate)),
        ("uart1", "MON_RF") => Some(CfgVal::MsgOutUbxMonRfUart1(rate)),
        ("uart1", "MON_COMMS") => Some(CfgVal::MsgOutUbxMoncommsUart1(rate)),
        ("uart1", "MON_HW") => Some(CfgVal::MsgOutUbxMonHwUart1(rate)),
        ("uart1", "SEC_SIG") => Some(CfgVal::MsgOutUbxSecSigUart1(rate)),
        ("uart1", "SEC_SIGLOG") => Some(CfgVal::MsgOutUbxSecSiglogUart1(rate)),
        ("uart1", "RXM_COR") => Some(CfgVal::MsgOutUbxRxmCorUart1(rate)),

        // ====== UART2 messages ======
        ("uart2", "NAV_PVT") => Some(CfgVal::MsgOutUbxNavPvtUart2(rate)),
        ("uart2", "NAV_HPPOSLLH") => Some(CfgVal::MsgOutUbxNavHpPosLlhUart2(rate)),
        ("uart2", "NAV_HPPOSECEF") => Some(CfgVal::MsgOutUbxNavHpPosEcefUart2(rate)),
        ("uart2", "NAV_SAT") => Some(CfgVal::MsgOutUbxNavSatUart2(rate)),
        ("uart2", "NAV_SIG") => Some(CfgVal::MsgOutUbxNavSigUart2(rate)),
        ("uart2", "NAV_STATUS") => Some(CfgVal::MsgOutUbxNavStatusUart2(rate)),
        ("uart2", "NAV_DOP") => Some(CfgVal::MsgOutUbxNavDopUart2(rate)),
        ("uart2", "NAV_POSLLH") => Some(CfgVal::MsgOutUbxNavPosLlhUart2(rate)),
        ("uart2", "NAV_POSECEF") => Some(CfgVal::MsgOutUbxNavPosEcefUart2(rate)),
        ("uart2", "NAV_COV") => Some(CfgVal::MsgOutUbxNavCovUart2(rate)),
        ("uart2", "NAV_RELPOSNED") => Some(CfgVal::MsgOutUbxNavRelposNedUart2(rate)),
        ("uart2", "NAV_PL") => Some(CfgVal::MsgOutUbxNavPlUart2(rate)),
        ("uart2", "NAV_SVIN") => Some(CfgVal::MsgOutUbxNavSvinUart2(rate)),
        ("uart2", "MON_RF") => Some(CfgVal::MsgOutUbxMonRfUart2(rate)),
        ("uart2", "MON_COMMS") => Some(CfgVal::MsgOutUbxMoncommsUart2(rate)),
        ("uart2", "MON_HW") => Some(CfgVal::MsgOutUbxMonHwUart2(rate)),
        ("uart2", "SEC_SIG") => Some(CfgVal::MsgOutUbxSecSigUart2(rate)),
        ("uart2", "SEC_SIGLOG") => Some(CfgVal::MsgOutUbxSecSiglogUart2(rate)),
        ("uart2", "RXM_COR") => Some(CfgVal::MsgOutUbxRxmCorUart2(rate)),

        // ====== RTCM3 output messages (UART2 - for moving base) ======
        ("uart2", "RTCM_3X_TYPE4072_0") => Some(CfgVal::MsgOutRtcm3Xtype40720Uart2(rate)),
        ("uart2", "RTCM_3X_TYPE1005") => Some(CfgVal::MsgOutRtcm3Xtype1005Uart2(rate)),
        ("uart2", "RTCM_3X_TYPE1074") => Some(CfgVal::MsgOutRtcm3Xtype1074Uart2(rate)),
        ("uart2", "RTCM_3X_TYPE1084") => Some(CfgVal::MsgOutRtcm3Xtype1084Uart2(rate)),
        ("uart2", "RTCM_3X_TYPE1094") => Some(CfgVal::MsgOutRtcm3Xtype1094Uart2(rate)),
        ("uart2", "RTCM_3X_TYPE1124") => Some(CfgVal::MsgOutRtcm3Xtype1124Uart2(rate)),
        ("uart2", "RTCM_3X_TYPE1230") => Some(CfgVal::MsgOutRtcm3Xtype1230Uart2(rate)),

        _ => {
            warn!("Unknown message '{}' for port '{}'", msg, port);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cfg_vals_from_config() {
        let config = UbloxConfig::default();
        let vals = build_cfg_vals_from_config(&config);
        // Should have rate settings at minimum
        assert!(vals.len() >= 2);
    }

    #[test]
    fn test_parse_message_rate() {
        let result = parse_message_rate("usb", "NAV_PVT", 1);
        assert!(matches!(result, Some(CfgVal::MsgOutUbxNavPvtUsb(1))));
    }

    #[test]
    fn test_protocol_in_cfg_val() {
        let result = protocol_in_cfg_val("usb", "ubx", true);
        assert!(matches!(result, Some(CfgVal::UsbInProtUbx(true))));

        let result_disabled = protocol_in_cfg_val("usb", "nmea", false);
        assert!(matches!(
            result_disabled,
            Some(CfgVal::UsbInProtNmea(false))
        ));
    }

    #[test]
    fn test_protocol_out_cfg_val() {
        let result = protocol_out_cfg_val("uart1", "rtcm3x", true);
        assert!(matches!(result, Some(CfgVal::Uart1OutProtRtcm3x(true))));
    }
}
