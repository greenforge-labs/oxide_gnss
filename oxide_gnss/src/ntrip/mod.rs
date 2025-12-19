//! NTRIP client module for receiving RTK corrections.
//!
//! This module provides an NTRIP v1/v2 client that connects to a caster,
//! streams RTCM corrections, and optionally sends GGA position reports.
//!
//! Features:
//! - Plain TCP and TLS-encrypted (HTTPS) connections
//! - NTRIP v1 (ICY) and v2 (HTTP/1.1) protocol support
//! - Sourcetable retrieval and mountpoint discovery

mod client;
mod gga;
mod sourcetable;
mod stream;
mod task;

pub use client::NtripClient;
pub use gga::GgaSentence;
pub use sourcetable::{CasterEntry, NetworkEntry, Sourcetable, StreamEntry};
pub use task::{spawn_ntrip_task, NtripMessage, NtripTask, NtripTaskChannels, NtripTaskHandle};
