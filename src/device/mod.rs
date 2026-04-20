//! Device communication module for GNSS receivers.
//!
//! This module provides the serial port layer and protocol handling for
//! communicating with GNSS devices like the u-blox ZED-F9P.

mod cfg_key_mapping;
mod config;
mod serial;
mod task;
#[cfg(test)]
pub mod test_fixtures;
pub mod ubx;

pub use cfg_key_mapping::build_cfg_vals_from_config;
pub use config::{
    assemble_usb_serial_string, chunk_usb_serial_to_cfg_vals, query_cfg_valget, ConfigStep,
    ConfiguratorOptions, DeviceConfigurator, USB_SERIAL_KEY_IDS,
};
pub use serial::{SerialPort, SerialPortBuilder};
pub use task::{
    spawn_device_task, DeviceMessage, DeviceTask, DeviceTaskChannels, DeviceTaskHandle,
    DeviceTaskState, GgaData,
};
pub use ubx::{
    build_cfg_msg, build_cfg_rate, build_cfg_valset, build_cfg_valset_all_layers,
    build_rover_config, build_safety_messages_config, cfg_keys, msg_ids, AckResult,
    AntennaPowerData, AntennaStatusData, CarrierSolution, CfgValGetResponse, CovData,
    JammingStateData, MonCommsData, MonCommsPortData, MonHwData, MonRfData, NavPlData, NavPlFrame,
    NavPlInvalidityReason, PosEcefData, ProcessResult, PvtData, RxmCorData, SecSigData,
    SecSiglogData, SecSiglogEventData, SignalQuality, SpoofingStateData, UbxHandler, UbxStats,
};
// Re-export CfgVal for configuration building
pub use ublox::cfg_val::CfgVal;
