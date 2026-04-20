//! ROS2 integration module for oxide_gnss.
//!
//! This module is only compiled when the `ros2` feature is enabled.
//! It provides:
//! - ROS2 node creation and lifecycle management
//! - Topic publishers for GNSS data
//! - Diagnostic publishing
//!
//! # Building with ROS2 support
//!
//! ```bash
//! # In a ROS2 environment (e.g., WSL2 with ROS2 Jazzy)
//! source /opt/ros/jazzy/setup.bash
//! cargo build --features ros2
//! ```

mod conversions;
mod node;
mod publishers;
mod task;

pub use conversions::{pvt_to_twist, ToRosMessage};
pub use node::{create_context, GnssNode, GnssNodeConfig};
pub use publishers::GnssPublishers;
pub use task::{
    spawn_ros_task, RosTask, RosTaskChannels, RosTaskConfig, RosTaskHandle, RosTaskState,
};
