//! Mapping from u-blox CFG_* key names to ublox crate CfgVal enum variants.
//!
//! This module provides translation between the u-blox documentation naming convention
//! (e.g., `CFG_MSGOUT_UBX_NAV_PVT_USB`) and the `ublox` crate's `CfgVal` enum variants
//! (e.g., `CfgVal::MsgOutUbxNavPvtUsb`).

use tracing::warn;
use ublox::cfg_val::CfgVal;

/// Parse a CFG_* key name and value into a CfgVal.
///
/// # Arguments
/// * `name` - CFG_* key name from u-blox documentation (e.g., "CFG_MSGOUT_UBX_NAV_PVT_USB")
/// * `value` - The configuration value (integer or boolean)
///
/// # Returns
/// * `Some(CfgVal)` if the key is recognized
/// * `None` if the key is unknown or the value type is incorrect
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
