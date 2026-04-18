//! Mapping from structured u-blox config to ublox crate CfgVal enum variants.
//!
//! This module provides translation between the structured YAML configuration
//! and the `ublox` crate's `CfgVal` enum variants.

use tracing::warn;
use ublox::cfg_nav5::NavDynamicModel;
use ublox::cfg_rate::AlignmentToReferenceTime;
use ublox::cfg_tmode2::CfgTModeModes;
use ublox::cfg_val::{CfgVal, TModePosType, TpPulse, TpPulseLength};

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
        let ublox_model = convert_dynamic_model(dynamic_model);
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

    // Base station position configuration (CFG-TMODE-*)
    if let Some(ref bp) = config.base_position {
        build_base_position_cfg_vals(bp, &mut vals);
    }

    // Time mark (TIM-TM2) configuration
    if let Some(ref tm) = config.time_mark {
        if tm.enabled == Some(true) {
            // Enable TIM-TM2 message output on USB (rate 1 = every measurement)
            vals.push(CfgVal::MsgOutUbxTimTm2Usb(1));
        }
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

    // Enumerate-and-zero: for every (port, msg) in our managed MSGOUT table not
    // explicitly requested above, push rate=0. Makes oxide self-cleaning —
    // stale config left by a previous driver (e.g. ferrous) gets turned off on
    // startup rather than silently loading the receiver and throttling the nav
    // rate.
    if config.clear_unmanaged {
        use std::collections::HashSet;
        let user_keys: HashSet<(&str, &str)> = config
            .messages
            .usb
            .keys()
            .map(|m| ("usb", m.as_str()))
            .chain(
                config
                    .messages
                    .uart1
                    .keys()
                    .map(|m| ("uart1", m.as_str())),
            )
            .chain(
                config
                    .messages
                    .uart2
                    .keys()
                    .map(|m| ("uart2", m.as_str())),
            )
            .collect();
        for (port, msg, make_cfg) in MSG_RATE_TABLE {
            if !user_keys.contains(&(*port, *msg)) {
                vals.push(make_cfg(0));
            }
        }
        // TIM-TM2 on USB isn't in MSG_RATE_TABLE; zero it unless time_mark is on.
        let tm_enabled = matches!(&config.time_mark, Some(tm) if tm.enabled == Some(true));
        if !tm_enabled {
            vals.push(CfgVal::MsgOutUbxTimTm2Usb(0));
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
            crate::config::TimeGrid::Utc => AlignmentToReferenceTime::Utc,
            crate::config::TimeGrid::Gps => AlignmentToReferenceTime::Gps,
            crate::config::TimeGrid::Glonass => AlignmentToReferenceTime::Glo,
            crate::config::TimeGrid::Beidou => AlignmentToReferenceTime::Bds,
            crate::config::TimeGrid::Galileo => AlignmentToReferenceTime::Gal,
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
        vals.push(CfgVal::TpPulseDef(TpPulse::Freq));
    }

    // Set pulse length definition to absolute length (not ratio)
    // This is needed when using pulse_length_us settings
    if tp.pulse_length_us.is_some() || tp.pulse_length_unlocked_us.is_some() {
        vals.push(CfgVal::TpPulseLengthDef(TpPulseLength::Length));
    }
}

/// Build CfgVal entries for base station position configuration.
fn build_base_position_cfg_vals(bp: &crate::config::BasePositionConfig, vals: &mut Vec<CfgVal>) {
    // Set the time mode based on configuration
    let tmode = match bp.mode {
        crate::config::BasePositionMode::Disabled => CfgTModeModes::Disabled,
        crate::config::BasePositionMode::SurveyIn => CfgTModeModes::SurveyIn,
        crate::config::BasePositionMode::Fixed => CfgTModeModes::Fixed,
    };
    vals.push(CfgVal::TModeModeDef(tmode));

    // Survey-in configuration
    if let Some(ref svin) = bp.survey_in {
        if let Some(min_dur) = svin.min_duration_s {
            vals.push(CfgVal::TModeSvInMinDur(min_dur));
        }
        if let Some(acc_limit) = svin.accuracy_limit_m {
            // Accuracy is stored in 0.1mm units (e.g., 2.0m = 20000)
            let acc_0_1mm = (acc_limit * 10000.0).round() as u32;
            vals.push(CfgVal::TModeSvInAccLimit(acc_0_1mm));
        }
    }

    // Fixed position configuration
    if let Some(ref fixed) = bp.fixed {
        // Use LLH (latitude/longitude/height) format
        vals.push(CfgVal::TModePosTypeDef(TModePosType::LLH));

        if let Some(lat) = fixed.latitude {
            // Latitude in 1e-7 degrees, with high-precision component in 1e-9 degrees
            let lat_scaled = lat * 1e7;
            let lat_main = lat_scaled.trunc() as i32;
            let lat_hp = ((lat_scaled.fract()) * 100.0).round() as i8;
            vals.push(CfgVal::TModeLat(lat_main));
            vals.push(CfgVal::TModeLatHp(lat_hp));
        }

        if let Some(lon) = fixed.longitude {
            // Longitude in 1e-7 degrees, with high-precision component in 1e-9 degrees
            let lon_scaled = lon * 1e7;
            let lon_main = lon_scaled.trunc() as i32;
            let lon_hp = ((lon_scaled.fract()) * 100.0).round() as i8;
            vals.push(CfgVal::TModeLon(lon_main));
            vals.push(CfgVal::TModeLonHp(lon_hp));
        }

        if let Some(height) = fixed.height_m {
            // Height in cm, with high-precision component in 0.1mm
            let height_cm = (height * 100.0).trunc() as i32;
            let height_hp = (((height * 100.0).fract()) * 100.0).round() as i8;
            vals.push(CfgVal::TModeHeight(height_cm));
            vals.push(CfgVal::TModeHeightHp(height_hp));
        }

        if let Some(acc) = fixed.accuracy_m {
            // Accuracy in 0.1mm units
            let acc_0_1mm = (acc * 10000.0).round() as u32;
            vals.push(CfgVal::TModeFixedPosAcc(acc_0_1mm));
        }
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

/// Convert our DynamicModel enum to ublox NavDynamicModel.
fn convert_dynamic_model(model: crate::config::DynamicModel) -> NavDynamicModel {
    use crate::config::DynamicModel;

    /// Helper for unsupported models - logs warning and returns fallback.
    fn unsupported(name: &str, raw: u8, fallback: NavDynamicModel) -> NavDynamicModel {
        warn!(
            "{} dynamic model ({}) not in ublox crate; using {:?} fallback",
            name, raw, fallback
        );
        fallback
    }

    match model {
        DynamicModel::Portable => NavDynamicModel::Portable,
        DynamicModel::Stationary => NavDynamicModel::Stationary,
        DynamicModel::Pedestrian => NavDynamicModel::Pedestrian,
        DynamicModel::Automotive => NavDynamicModel::Automotive,
        DynamicModel::Sea => NavDynamicModel::Sea,
        DynamicModel::AirborneLight => NavDynamicModel::AirborneWithLess1gAcceleration,
        DynamicModel::AirborneMedium => NavDynamicModel::AirborneWithLess2gAcceleration,
        DynamicModel::AirborneHigh => NavDynamicModel::AirborneWithLess4gAcceleration,
        // Models not directly supported in ublox crate
        DynamicModel::Wrist => unsupported("Wrist", 9, NavDynamicModel::Portable),
        DynamicModel::Bike => unsupported("Bike", 10, NavDynamicModel::Portable),
        DynamicModel::Mower => unsupported("Mower", 11, NavDynamicModel::Automotive),
        DynamicModel::Escooter => unsupported("E-scooter", 12, NavDynamicModel::Automotive),
        DynamicModel::Robot => unsupported("Robot", 13, NavDynamicModel::Automotive),
    }
}

/// Message rate entry: (port, message_name, constructor)
type MsgRateEntry = (&'static str, &'static str, fn(u8) -> CfgVal);

/// Message rate configuration lookup table.
/// Maps (port, message) to a constructor function for the appropriate CfgVal variant.
static MSG_RATE_TABLE: &[MsgRateEntry] = &[
    // USB messages
    ("usb", "NAV_PVT", CfgVal::MsgOutUbxNavPvtUsb),
    ("usb", "NAV_HPPOSLLH", CfgVal::MsgOutUbxNavHpPosLlhUsb),
    ("usb", "NAV_HPPOSECEF", CfgVal::MsgOutUbxNavHpPosEcefUsb),
    ("usb", "NAV_SAT", CfgVal::MsgOutUbxNavSatUsb),
    ("usb", "NAV_SIG", CfgVal::MsgOutUbxNavSigUsb),
    ("usb", "NAV_STATUS", CfgVal::MsgOutUbxNavStatusUsb),
    ("usb", "NAV_DOP", CfgVal::MsgOutUbxNavDopUsb),
    ("usb", "NAV_CLOCK", CfgVal::MsgOutUbxNavClockUsb),
    ("usb", "NAV_EOE", CfgVal::MsgOutUbxNavEoeUsb),
    ("usb", "NAV_POSLLH", CfgVal::MsgOutUbxNavPosLlhUsb),
    ("usb", "NAV_POSECEF", CfgVal::MsgOutUbxNavPosEcefUsb),
    ("usb", "NAV_ODO", CfgVal::MsgOutUbxNavOdoUsb),
    ("usb", "NAV_COV", CfgVal::MsgOutUbxNavCovUsb),
    ("usb", "NAV_RELPOSNED", CfgVal::MsgOutUbxNavRelposNedUsb),
    ("usb", "NAV_PL", CfgVal::MsgOutUbxNavPlUsb),
    ("usb", "NAV_SVIN", CfgVal::MsgOutUbxNavSvinUsb),
    ("usb", "MON_RF", CfgVal::MsgOutUbxMonRfUsb),
    ("usb", "MON_COMMS", CfgVal::MsgOutUbxMoncommsUsb),
    ("usb", "MON_HW", CfgVal::MsgOutUbxMonHwUsb),
    ("usb", "SEC_SIG", CfgVal::MsgOutUbxSecSigUsb),
    ("usb", "SEC_SIGLOG", CfgVal::MsgOutUbxSecSiglogUsb),
    ("usb", "RXM_COR", CfgVal::MsgOutUbxRxmCorUsb),
    // UART1 messages
    ("uart1", "NAV_PVT", CfgVal::MsgOutUbxNavPvtUart1),
    ("uart1", "NAV_HPPOSLLH", CfgVal::MsgOutUbxNavHpPosLlhUart1),
    ("uart1", "NAV_HPPOSECEF", CfgVal::MsgOutUbxNavHpPosEcefUart1),
    ("uart1", "NAV_SAT", CfgVal::MsgOutUbxNavSatUart1),
    ("uart1", "NAV_SIG", CfgVal::MsgOutUbxNavSigUart1),
    ("uart1", "NAV_STATUS", CfgVal::MsgOutUbxNavStatusUart1),
    ("uart1", "NAV_DOP", CfgVal::MsgOutUbxNavDopUart1),
    ("uart1", "NAV_POSLLH", CfgVal::MsgOutUbxNavPosLlhUart1),
    ("uart1", "NAV_POSECEF", CfgVal::MsgOutUbxNavPosEcefUart1),
    ("uart1", "NAV_COV", CfgVal::MsgOutUbxNavCovUart1),
    ("uart1", "NAV_RELPOSNED", CfgVal::MsgOutUbxNavRelposNedUart1),
    ("uart1", "NAV_PL", CfgVal::MsgOutUbxNavPlUart1),
    ("uart1", "NAV_SVIN", CfgVal::MsgOutUbxNavSvinUart1),
    ("uart1", "MON_RF", CfgVal::MsgOutUbxMonRfUart1),
    ("uart1", "MON_COMMS", CfgVal::MsgOutUbxMoncommsUart1),
    ("uart1", "MON_HW", CfgVal::MsgOutUbxMonHwUart1),
    ("uart1", "SEC_SIG", CfgVal::MsgOutUbxSecSigUart1),
    ("uart1", "SEC_SIGLOG", CfgVal::MsgOutUbxSecSiglogUart1),
    ("uart1", "RXM_COR", CfgVal::MsgOutUbxRxmCorUart1),
    // UART2 messages
    ("uart2", "NAV_PVT", CfgVal::MsgOutUbxNavPvtUart2),
    ("uart2", "NAV_HPPOSLLH", CfgVal::MsgOutUbxNavHpPosLlhUart2),
    ("uart2", "NAV_HPPOSECEF", CfgVal::MsgOutUbxNavHpPosEcefUart2),
    ("uart2", "NAV_SAT", CfgVal::MsgOutUbxNavSatUart2),
    ("uart2", "NAV_SIG", CfgVal::MsgOutUbxNavSigUart2),
    ("uart2", "NAV_STATUS", CfgVal::MsgOutUbxNavStatusUart2),
    ("uart2", "NAV_DOP", CfgVal::MsgOutUbxNavDopUart2),
    ("uart2", "NAV_POSLLH", CfgVal::MsgOutUbxNavPosLlhUart2),
    ("uart2", "NAV_POSECEF", CfgVal::MsgOutUbxNavPosEcefUart2),
    ("uart2", "NAV_COV", CfgVal::MsgOutUbxNavCovUart2),
    ("uart2", "NAV_RELPOSNED", CfgVal::MsgOutUbxNavRelposNedUart2),
    ("uart2", "NAV_PL", CfgVal::MsgOutUbxNavPlUart2),
    ("uart2", "NAV_SVIN", CfgVal::MsgOutUbxNavSvinUart2),
    ("uart2", "MON_RF", CfgVal::MsgOutUbxMonRfUart2),
    ("uart2", "MON_COMMS", CfgVal::MsgOutUbxMoncommsUart2),
    ("uart2", "MON_HW", CfgVal::MsgOutUbxMonHwUart2),
    ("uart2", "SEC_SIG", CfgVal::MsgOutUbxSecSigUart2),
    ("uart2", "SEC_SIGLOG", CfgVal::MsgOutUbxSecSiglogUart2),
    ("uart2", "RXM_COR", CfgVal::MsgOutUbxRxmCorUart2),
    // RTCM3 output messages (UART2 - for moving base)
    (
        "uart2",
        "RTCM_3X_TYPE4072_0",
        CfgVal::MsgOutRtcm3Xtype40720Uart2,
    ),
    (
        "uart2",
        "RTCM_3X_TYPE1005",
        CfgVal::MsgOutRtcm3Xtype1005Uart2,
    ),
    (
        "uart2",
        "RTCM_3X_TYPE1074",
        CfgVal::MsgOutRtcm3Xtype1074Uart2,
    ),
    (
        "uart2",
        "RTCM_3X_TYPE1084",
        CfgVal::MsgOutRtcm3Xtype1084Uart2,
    ),
    (
        "uart2",
        "RTCM_3X_TYPE1094",
        CfgVal::MsgOutRtcm3Xtype1094Uart2,
    ),
    (
        "uart2",
        "RTCM_3X_TYPE1124",
        CfgVal::MsgOutRtcm3Xtype1124Uart2,
    ),
    (
        "uart2",
        "RTCM_3X_TYPE1230",
        CfgVal::MsgOutRtcm3Xtype1230Uart2,
    ),
];

/// Parse message output rate setting using lookup table.
fn parse_message_rate(port: &str, msg: &str, rate: u8) -> Option<CfgVal> {
    MSG_RATE_TABLE
        .iter()
        .find(|(p, m, _)| *p == port && *m == msg)
        .map(|(_, _, make_cfg)| make_cfg(rate))
        .or_else(|| {
            warn!("Unknown message '{}' for port '{}'", msg, port);
            None
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cfg_vals_from_config() {
        let config = UbloxConfig::default();
        let vals = build_cfg_vals_from_config(&config);
        // Default config enables clear_unmanaged, so every MSG_RATE_TABLE entry
        // plus TIM-TM2 gets pushed as rate=0, in addition to rate/protocol/etc.
        assert!(vals.len() >= 2);
        // And should be way more than 2 thanks to enumerate-and-zero
        assert!(vals.len() > MSG_RATE_TABLE.len());
    }

    #[test]
    fn test_enumerate_and_zero_defaults_on() {
        // With clear_unmanaged (the default) and no user-requested messages,
        // every managed MSGOUT key should get pushed with rate=0.
        let config = UbloxConfig::default();
        assert!(config.clear_unmanaged, "clear_unmanaged defaults to true");
        let vals = build_cfg_vals_from_config(&config);

        // NAV_SAT on USB is in MSG_RATE_TABLE but not requested → must be zeroed
        let nav_sat_zeroed = vals
            .iter()
            .any(|v| matches!(v, CfgVal::MsgOutUbxNavSatUsb(0)));
        assert!(nav_sat_zeroed, "NAV-SAT USB should be zeroed when unmanaged");

        // TIM-TM2 USB isn't in MSG_RATE_TABLE; separate code path should zero it
        let tim_tm2_zeroed = vals
            .iter()
            .any(|v| matches!(v, CfgVal::MsgOutUbxTimTm2Usb(0)));
        assert!(tim_tm2_zeroed, "TIM-TM2 USB should be zeroed by default");
    }

    #[test]
    fn test_enumerate_and_zero_respects_user_requested() {
        // When the user requests NAV_PVT on USB, enumerate-and-zero must NOT
        // overwrite it with rate=0 — user's value wins.
        let mut config = UbloxConfig::default();
        config.messages.usb.insert("NAV_PVT".to_string(), 1);
        let vals = build_cfg_vals_from_config(&config);

        let nav_pvt_one = vals
            .iter()
            .any(|v| matches!(v, CfgVal::MsgOutUbxNavPvtUsb(1)));
        let nav_pvt_zero = vals
            .iter()
            .any(|v| matches!(v, CfgVal::MsgOutUbxNavPvtUsb(0)));
        assert!(nav_pvt_one, "NAV-PVT USB rate=1 must be present");
        assert!(
            !nav_pvt_zero,
            "NAV-PVT USB rate=0 must NOT be added when user requests rate=1"
        );
    }

    #[test]
    fn test_clear_unmanaged_opt_out() {
        // When clear_unmanaged=false, NO extra zero entries should be added.
        let config = UbloxConfig {
            clear_unmanaged: false,
            ..UbloxConfig::default()
        };
        let vals = build_cfg_vals_from_config(&config);

        let any_nav_sat_zero = vals
            .iter()
            .any(|v| matches!(v, CfgVal::MsgOutUbxNavSatUsb(0)));
        assert!(
            !any_nav_sat_zero,
            "NAV-SAT USB must not be forcibly zeroed when clear_unmanaged=false"
        );

        let any_tim_tm2_zero = vals
            .iter()
            .any(|v| matches!(v, CfgVal::MsgOutUbxTimTm2Usb(0)));
        assert!(
            !any_tim_tm2_zero,
            "TIM-TM2 USB must not be zeroed when clear_unmanaged=false"
        );
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
