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
    for proto in &config.protocols.usb_in {
        if let Some(val) = parse_protocol_in("usb", proto) {
            vals.push(val);
        }
    }
    for proto in &config.protocols.usb_out {
        if let Some(val) = parse_protocol_out("usb", proto) {
            vals.push(val);
        }
    }

    // Protocol settings - UART1
    for proto in &config.protocols.uart1_in {
        if let Some(val) = parse_protocol_in("uart1", proto) {
            vals.push(val);
        }
    }
    for proto in &config.protocols.uart1_out {
        if let Some(val) = parse_protocol_out("uart1", proto) {
            vals.push(val);
        }
    }

    // Protocol settings - UART2
    for proto in &config.protocols.uart2_in {
        if let Some(val) = parse_protocol_in("uart2", proto) {
            vals.push(val);
        }
    }
    for proto in &config.protocols.uart2_out {
        if let Some(val) = parse_protocol_out("uart2", proto) {
            vals.push(val);
        }
    }

    // Protocol settings - I2C
    for proto in &config.protocols.i2c_in {
        if let Some(val) = parse_protocol_in("i2c", proto) {
            vals.push(val);
        }
    }
    for proto in &config.protocols.i2c_out {
        if let Some(val) = parse_protocol_out("i2c", proto) {
            vals.push(val);
        }
    }

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

    // Signal/constellation settings
    build_signal_cfg_vals(&config.signals, &mut vals);

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

/// Parse input protocol setting.
fn parse_protocol_in(port: &str, protocol: &str) -> Option<CfgVal> {
    match (port, protocol.to_lowercase().as_str()) {
        // USB
        ("usb", "ubx") => Some(CfgVal::UsbInProtUbx(true)),
        ("usb", "nmea") => Some(CfgVal::UsbInProtNmea(true)),
        ("usb", "rtcm3x") => Some(CfgVal::UsbInProtRtcm3x(true)),
        // UART1
        ("uart1", "ubx") => Some(CfgVal::Uart1InProtUbx(true)),
        ("uart1", "nmea") => Some(CfgVal::Uart1InProtNmea(true)),
        ("uart1", "rtcm3x") => Some(CfgVal::Uart1InProtRtcm3x(true)),
        // UART2
        ("uart2", "ubx") => Some(CfgVal::Uart2InProtUbx(true)),
        ("uart2", "nmea") => Some(CfgVal::Uart2InProtNmea(true)),
        ("uart2", "rtcm3x") => Some(CfgVal::Uart2InProtRtcm3x(true)),
        // I2C - protocol variants not available in ublox crate
        ("i2c", _) => {
            warn!("I2C protocol configuration not supported by ublox crate");
            None
        }
        _ => {
            warn!("Unknown protocol '{}' for port '{}'", protocol, port);
            None
        }
    }
}

/// Parse output protocol setting.
fn parse_protocol_out(port: &str, protocol: &str) -> Option<CfgVal> {
    match (port, protocol.to_lowercase().as_str()) {
        // USB
        ("usb", "ubx") => Some(CfgVal::UsbOutProtUbx(true)),
        ("usb", "nmea") => Some(CfgVal::UsbOutProtNmea(true)),
        ("usb", "rtcm3x") => Some(CfgVal::UsbOutProtRtcm3x(true)),
        // UART1
        ("uart1", "ubx") => Some(CfgVal::Uart1OutProtUbx(true)),
        ("uart1", "nmea") => Some(CfgVal::Uart1OutProtNmea(true)),
        ("uart1", "rtcm3x") => Some(CfgVal::Uart1OutProtRtcm3x(true)),
        // UART2
        ("uart2", "ubx") => Some(CfgVal::Uart2OutProtUbx(true)),
        ("uart2", "nmea") => Some(CfgVal::Uart2OutProtNmea(true)),
        ("uart2", "rtcm3x") => Some(CfgVal::Uart2OutProtRtcm3x(true)),
        // I2C - protocol variants not available in ublox crate
        ("i2c", _) => {
            warn!("I2C protocol configuration not supported by ublox crate");
            None
        }
        _ => {
            warn!("Unknown protocol '{}' for port '{}'", protocol, port);
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
        ("uart2", "MON_RF") => Some(CfgVal::MsgOutUbxMonRfUart2(rate)),
        ("uart2", "MON_COMMS") => Some(CfgVal::MsgOutUbxMoncommsUart2(rate)),
        ("uart2", "MON_HW") => Some(CfgVal::MsgOutUbxMonHwUart2(rate)),
        ("uart2", "SEC_SIG") => Some(CfgVal::MsgOutUbxSecSigUart2(rate)),
        ("uart2", "SEC_SIGLOG") => Some(CfgVal::MsgOutUbxSecSiglogUart2(rate)),
        ("uart2", "RXM_COR") => Some(CfgVal::MsgOutUbxRxmCorUart2(rate)),

        _ => {
            warn!("Unknown message '{}' for port '{}'", msg, port);
            None
        }
    }
}

// Legacy support for old CFG_* format (deprecated)
/// Parse a CFG_* key name and value into a CfgVal.
/// This is for backward compatibility with the old config format.
#[deprecated(note = "Use structured config format instead")]
pub fn parse_cfg_key(name: &str, value: &serde_yaml::Value) -> Option<CfgVal> {
    match name {
        // Skip non-CFG keys (like "family")
        "family" => None,

        // ========================================
        // CFG-RATE: Navigation/Measurement Rates
        // ========================================
        "CFG_RATE_MEAS" => value.as_u64().map(|v| CfgVal::RateMeas(v as u16)),
        "CFG_RATE_NAV" => value.as_u64().map(|v| CfgVal::RateNav(v as u16)),

        // ========================================
        // CFG-MSGOUT: UBX Message Output Rates
        // ========================================
        // NAV messages - USB
        "CFG_MSGOUT_UBX_NAV_PVT_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxNavPvtUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_HPPOSLLH_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavHpPosLlhUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_HPPOSECEF_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavHpPosEcefUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_SAT_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxNavSatUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_SIG_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxNavSigUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_STATUS_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavStatusUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_DOP_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxNavDopUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_CLOCK_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavClockUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_EOE_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxNavEoeUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_POSLLH_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavPosLlhUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_POSECEF_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavPosEcefUsb(v as u8)),
        "CFG_MSGOUT_UBX_NAV_ODO_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxNavOdoUsb(v as u8)),

        // NAV messages - UART1
        "CFG_MSGOUT_UBX_NAV_PVT_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavPvtUart1(v as u8)),
        "CFG_MSGOUT_UBX_NAV_HPPOSLLH_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavHpPosLlhUart1(v as u8)),
        "CFG_MSGOUT_UBX_NAV_SAT_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavSatUart1(v as u8)),
        "CFG_MSGOUT_UBX_NAV_STATUS_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavStatusUart1(v as u8)),

        // ========================================
        // CFG-USBINPROT: USB Input Protocols
        // ========================================
        "CFG_USBINPROT_UBX" => value.as_bool().map(CfgVal::UsbInProtUbx),
        "CFG_USBINPROT_NMEA" => value.as_bool().map(CfgVal::UsbInProtNmea),
        "CFG_USBINPROT_RTCM3X" => value.as_bool().map(CfgVal::UsbInProtRtcm3x),

        // ========================================
        // CFG-USBOUTPROT: USB Output Protocols
        // ========================================
        "CFG_USBOUTPROT_UBX" => value.as_bool().map(CfgVal::UsbOutProtUbx),
        "CFG_USBOUTPROT_NMEA" => value.as_bool().map(CfgVal::UsbOutProtNmea),

        // ========================================
        // CFG-UART1INPROT: UART1 Input Protocols
        // ========================================
        "CFG_UART1INPROT_UBX" => value.as_bool().map(CfgVal::Uart1InProtUbx),
        "CFG_UART1INPROT_NMEA" => value.as_bool().map(CfgVal::Uart1InProtNmea),
        "CFG_UART1INPROT_RTCM3X" => value.as_bool().map(CfgVal::Uart1InProtRtcm3x),

        // ========================================
        // CFG-UART1OUTPROT: UART1 Output Protocols
        // ========================================
        "CFG_UART1OUTPROT_UBX" => value.as_bool().map(CfgVal::Uart1OutProtUbx),
        "CFG_UART1OUTPROT_NMEA" => value.as_bool().map(CfgVal::Uart1OutProtNmea),
        "CFG_UART1OUTPROT_RTCM3X" => value.as_bool().map(CfgVal::Uart1OutProtRtcm3x),

        // ========================================
        // CFG-UART1: UART1 Settings
        // ========================================
        "CFG_UART1_BAUDRATE" => value.as_u64().map(|v| CfgVal::Uart1Baudrate(v as u32)),
        "CFG_UART1_ENABLED" => value.as_bool().map(CfgVal::Uart1Enabled),

        // ========================================
        // Safety/Integrity Messages - USB
        // ========================================
        // Note: NAV-COV is NOT supported on ZED-F9P (only F9R/F9H)
        "CFG_MSGOUT_UBX_NAV_COV_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxNavCovUsb(v as u8)),
        // MON-HW is deprecated, prefer MON-RF
        "CFG_MSGOUT_UBX_MON_HW_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxMonHwUsb(v as u8)),
        "CFG_MSGOUT_UBX_MON_RF_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxMonRfUsb(v as u8)),
        "CFG_MSGOUT_UBX_MON_COMMS_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxMoncommsUsb(v as u8)),
        "CFG_MSGOUT_UBX_SEC_SIG_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxSecSigUsb(v as u8)),
        "CFG_MSGOUT_UBX_SEC_SIGLOG_USB" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxSecSiglogUsb(v as u8)),
        "CFG_MSGOUT_UBX_RXM_COR_USB" => value.as_u64().map(|v| CfgVal::MsgOutUbxRxmCorUsb(v as u8)),

        // Safety/Integrity Messages - UART1
        "CFG_MSGOUT_UBX_NAV_COV_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxNavCovUart1(v as u8)),
        "CFG_MSGOUT_UBX_MON_HW_UART1" => {
            value.as_u64().map(|v| CfgVal::MsgOutUbxMonHwUart1(v as u8))
        }
        "CFG_MSGOUT_UBX_MON_RF_UART1" => {
            value.as_u64().map(|v| CfgVal::MsgOutUbxMonRfUart1(v as u8))
        }
        "CFG_MSGOUT_UBX_MON_COMMS_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxMoncommsUart1(v as u8)),
        "CFG_MSGOUT_UBX_SEC_SIG_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxSecSigUart1(v as u8)),
        "CFG_MSGOUT_UBX_SEC_SIGLOG_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxSecSiglogUart1(v as u8)),
        "CFG_MSGOUT_UBX_RXM_COR_UART1" => value
            .as_u64()
            .map(|v| CfgVal::MsgOutUbxRxmCorUart1(v as u8)),

        // ========================================
        // Unknown key - log warning
        // ========================================
        _ => {
            warn!("Unknown CFG key '{}', ignoring", name);
            None
        }
    }
}

/// Build a list of CfgVal from UbloxConfig.
///
/// # Arguments
/// * `cfg_keys` - HashMap of CFG_* key names to values
///
/// # Returns
/// Vec of successfully parsed CfgVal entries
#[allow(deprecated)]
pub fn build_cfg_vals(
    cfg_keys: &std::collections::HashMap<String, serde_yaml::Value>,
) -> Vec<CfgVal> {
    cfg_keys
        .iter()
        .filter_map(|(name, value)| parse_cfg_key(name, value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_rate_meas() {
        let value = serde_yaml::Value::Number(100.into());
        let result = parse_cfg_key("CFG_RATE_MEAS", &value);
        assert!(matches!(result, Some(CfgVal::RateMeas(100))));
    }

    #[test]
    fn test_parse_msg_out() {
        let value = serde_yaml::Value::Number(1.into());
        let result = parse_cfg_key("CFG_MSGOUT_UBX_NAV_PVT_USB", &value);
        assert!(matches!(result, Some(CfgVal::MsgOutUbxNavPvtUsb(1))));
    }

    #[test]
    fn test_parse_protocol_bool() {
        let value = serde_yaml::Value::Bool(true);
        let result = parse_cfg_key("CFG_USBINPROT_UBX", &value);
        assert!(matches!(result, Some(CfgVal::UsbInProtUbx(true))));
    }

    #[test]
    fn test_unknown_key() {
        let value = serde_yaml::Value::Number(1.into());
        let result = parse_cfg_key("CFG_UNKNOWN_KEY", &value);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_cfg_vals() {
        let mut cfg_keys = HashMap::new();
        cfg_keys.insert(
            "CFG_RATE_MEAS".to_string(),
            serde_yaml::Value::Number(100.into()),
        );
        cfg_keys.insert(
            "CFG_MSGOUT_UBX_NAV_PVT_USB".to_string(),
            serde_yaml::Value::Number(1.into()),
        );
        cfg_keys.insert(
            "CFG_USBINPROT_UBX".to_string(),
            serde_yaml::Value::Bool(true),
        );

        let vals = build_cfg_vals(&cfg_keys);
        assert_eq!(vals.len(), 3);
    }

    #[test]
    fn test_family_is_skipped() {
        let value = serde_yaml::Value::String("F9P".to_string());
        let result = parse_cfg_key("family", &value);
        assert!(result.is_none());
    }
}
