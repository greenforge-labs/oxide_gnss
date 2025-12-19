//! Main ROS2 node for oxide_gnss.

use rclrs::{Context, Node, RclrsError};

use crate::config::{DeviceConfig, NtripConfig};
use crate::state::Supervisor;

use super::publishers::GnssPublishers;

/// Configuration for the GNSS ROS2 node.
#[derive(Debug, Clone)]
pub struct GnssNodeConfig {
    /// Node name
    pub node_name: String,
    /// Node namespace (e.g., "gnss")
    pub namespace: String,
    /// Device configuration
    pub device: DeviceConfig,
    /// NTRIP configuration (optional)
    pub ntrip: Option<NtripConfig>,
    /// Diagnostics publish rate in Hz
    pub diagnostics_rate_hz: f64,
    /// Enabled ROS topics (from Config::enabled_topics())
    pub enabled_topics: Vec<String>,
    /// Use high-precision data in ~/fix topic
    pub use_hp_for_fix: bool,
}

impl Default for GnssNodeConfig {
    fn default() -> Self {
        Self {
            node_name: "oxide_gnss".to_string(),
            namespace: "".to_string(),
            device: DeviceConfig {
                port: "/dev/ttyACM0".to_string(),
                baud_rate: 460800,
                frame: crate::config::CoordinateFrame::ENU,
                navigation: Default::default(),
                reconnect: Default::default(),
                ublox: None,
            },
            ntrip: None,
            diagnostics_rate_hz: 1.0,
            enabled_topics: vec![
                "~/fix".to_string(),
                "~/velocity".to_string(),
                "~/time_reference".to_string(),
            ],
            use_hp_for_fix: false,
        }
    }
}

/// The main GNSS ROS2 node.
///
/// This struct holds the ROS2 node, publishers, and supervisor.
/// Message handling is delegated to `RosTask` which runs asynchronously.
pub struct GnssNode {
    /// ROS2 node handle
    node: Node,
    /// Publishers
    publishers: GnssPublishers,
    /// Supervisor for device and NTRIP tasks
    supervisor: Supervisor,
    /// Configuration
    config: GnssNodeConfig,
}

impl GnssNode {
    /// Create a new GNSS driver instance using an existing ROS2 node.
    pub fn new(node: Node, config: GnssNodeConfig) -> Result<Self, RclrsError> {
        let publishers = GnssPublishers::new(
            &node,
            config.device.frame,
            config.use_hp_for_fix,
            &config.enabled_topics,
        )?;

        Ok(Self {
            node,
            publishers,
            supervisor: Supervisor::new(),
            config,
        })
    }

    /// Get the ROS2 node handle.
    pub fn node(&self) -> &Node {
        &self.node
    }

    /// Get the supervisor handle.
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    /// Get a mutable reference to the supervisor.
    pub fn supervisor_mut(&mut self) -> &mut Supervisor {
        &mut self.supervisor
    }

    /// Get the node configuration.
    pub fn config(&self) -> &GnssNodeConfig {
        &self.config
    }

    /// Get a reference to the publishers.
    pub fn publishers(&self) -> &GnssPublishers {
        &self.publishers
    }
}

/// Create a ROS2 context from environment.
pub fn create_context() -> Result<Context, RclrsError> {
    Context::default_from_env()
}
