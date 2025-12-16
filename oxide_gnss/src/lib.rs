//! # oxide_gnss
//!
//! A Rust-based ROS2 GNSS driver with integrated NTRIP client.
//!
//! ## Overview
//!
//! This crate provides a reliable, safety-focused GNSS driver for ROS2 applications.
//! It connects to GNSS receivers (initially u-blox ZED-F9P) and publishes position,
//! velocity, and diagnostic information to ROS2 topics.
//!
//! ## Features
//!
//! - Integrated NTRIP client for RTK corrections
//! - Multi-device support with configurable namespaces
//! - Comprehensive diagnostics for safety-critical applications
//! - Support for NED and ENU coordinate frames

pub mod config;
pub mod device;
pub mod error;
pub mod logging;
pub mod ntrip;
pub mod state;
pub mod transform;
pub mod util;

// ROS2 module - only compiled with ros2 feature
#[cfg(feature = "ros2")]
pub mod ros;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
