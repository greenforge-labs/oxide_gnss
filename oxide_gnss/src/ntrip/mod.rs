//! NTRIP client module for receiving RTK corrections.
//!
//! This module provides an NTRIP v1 client that connects to a caster,
//! streams RTCM corrections, and optionally sends GGA position reports.

mod client;
mod gga;
mod task;

pub use client::NtripClient;
pub use gga::GgaSentence;
pub use task::{spawn_ntrip_task, NtripMessage, NtripTask, NtripTaskChannels, NtripTaskHandle};
