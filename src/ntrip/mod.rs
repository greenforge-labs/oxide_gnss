//! NTRIP client module for receiving RTK corrections.
//!
//! This module wraps the [`ntrip_core`] crate and provides the async task
//! infrastructure for managing NTRIP connections within oxide_gnss.
//!
//! Features:
//! - Plain TCP and TLS-encrypted (HTTPS) connections
//! - NTRIP v1 (ICY) and v2 (HTTP/1.1) protocol support
//! - Sourcetable retrieval and mountpoint discovery
//! - Automatic reconnection with exponential backoff
//! - GGA position reporting
//!
//! The low-level NTRIP protocol implementation is provided by `ntrip_core`.
//! This module adds ROS2-integrated task management, state machine, and
//! channel-based communication.

mod task;

// Re-export ntrip_core types for convenience
pub use ntrip_core::{
    CasterEntry, GgaSentence, NetworkEntry, NtripClient, Sourcetable, StreamEntry,
};

// Export oxide_gnss task infrastructure
pub use task::{spawn_ntrip_task, NtripMessage, NtripTask, NtripTaskChannels, NtripTaskHandle};
