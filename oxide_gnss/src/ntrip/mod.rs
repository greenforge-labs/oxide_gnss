//! NTRIP client module for receiving RTK corrections.
//!
//! This module provides an NTRIP v1 client that connects to a caster,
//! streams RTCM corrections, and optionally sends GGA position reports.
//!
//! Supports both plain TCP and TLS-encrypted (HTTPS) connections.

mod client;
mod gga;
mod stream;
mod task;

pub use client::NtripClient;
pub use gga::GgaSentence;
pub use task::{spawn_ntrip_task, NtripMessage, NtripTask, NtripTaskChannels, NtripTaskHandle};
