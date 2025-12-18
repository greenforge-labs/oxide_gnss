//! oxide_gnss ROS2 node entry point.
//!
//! This binary starts the GNSS driver node which:
//! - Connects to a GNSS receiver via serial port
//! - Optionally connects to an NTRIP caster for RTK corrections
//! - Publishes position, velocity, and diagnostics to ROS2 topics

use std::path::PathBuf;
use std::time::Duration;

use rclrs::{CreateBasicExecutor, SpinOptions};
use tokio::time::sleep;
use tracing::{error, info, warn};

use oxide_gnss::config::Config;
use oxide_gnss::device::{spawn_device_task, DeviceTaskChannels};
use oxide_gnss::ntrip::{spawn_ntrip_task, NtripTaskChannels};
use oxide_gnss::ros::{create_context, GnssNode, GnssNodeConfig};
use oxide_gnss::ros::{spawn_ros_task, RosTaskChannels, RosTaskConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (respects RUST_LOG and FERROUS_GNSS_LOG_STYLE env vars)
    oxide_gnss::logging::init();

    info!("oxide_gnss v{}", oxide_gnss::VERSION);
    info!("Starting GNSS driver node...");

    // Initialize ROS2 context from environment (handles arguments like --ros-args)
    let context = match create_context() {
        Ok(ctx) => ctx,
        Err(e) => {
            error!("Failed to create ROS2 context: {}", e);
            return Err(e.into());
        }
    };

    // Create a basic executor from the context
    let mut executor = context.create_basic_executor();

    // Create the ROS2 node immediately to access parameters
    let node = executor.create_node("oxide_gnss")?;

    // Declare and get the config_file parameter
    // We treat this as a required parameter
    let config_param_name = "config_file";

    let param = node
        .declare_parameter::<std::sync::Arc<str>>(config_param_name)
        .default(std::sync::Arc::from(""))
        .mandatory()
        .map_err(|e| {
            error!("Failed to declare parameter '{}': {}", config_param_name, e);
            e
        })?;

    let config_path_str = param.get();

    if config_path_str.is_empty() {
        let msg = "Parameter 'config_file' is required. Launch with 'ros2 run oxide_gnss oxide_gnss_node --ros-args -p config_file:=/path/to/config.yaml'";
        error!("{}", msg);
        return Err(msg.into());
    }

    let config_path = PathBuf::from(config_path_str.as_ref());
    info!("Loading configuration from {}", config_path.display());

    // Load configuration (blocking I/O is acceptable at startup before tasks are spawned)
    let config = match Config::from_file(&config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            return Err(e.into());
        }
    };

    // Log effective configuration (mode, features, topics, messages)
    config.log_effective_config();

    // Resolve the u-blox configuration from mode + features (or legacy config)
    let ublox_config = config.resolve_ublox_config();

    // Get enabled topics for mode-aware validation
    let enabled_topics: Vec<String> = config
        .enabled_topics()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Determine if HP data should be used for ~/fix
    let use_hp_for_fix = config.features.high_precision
        || config
            .device
            .ublox
            .as_ref()
            .is_some_and(|u| u.is_message_enabled("NAV_HPPOSLLH"));

    // Create node configuration
    let node_config = GnssNodeConfig {
        node_name: "oxide_gnss".to_string(),
        namespace: node.namespace(),
        device: config.device.clone(),
        ntrip: config.ntrip.clone(),
        diagnostics_rate_hz: config.ros.rates.diagnostics_hz,
        enabled_topics: enabled_topics.clone(),
        use_hp_for_fix,
    };

    // Create the driver wrapper around the node
    let mut gnss_driver = match GnssNode::new(node, node_config) {
        Ok(driver) => driver,
        Err(e) => {
            error!("Failed to initialize GNSS driver: {}", e);
            return Err(e.into());
        }
    };

    // Get values we need from node before taking mutable borrow of supervisor
    let diagnostics_rate_hz = gnss_driver.config().diagnostics_rate_hz;
    let integrity_rate_hz = config.ros.rates.integrity_hz;
    let publishers = gnss_driver.publishers().clone();

    // Create supervisor (takes mutable borrow of driver)
    let supervisor = gnss_driver.supervisor_mut();
    let supervisor_handle = supervisor.handle();

    // Set up shutdown signal handler
    let shutdown_handle = supervisor_handle.clone();
    tokio::spawn(async move {
        // Set up Ctrl+C handler
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Received Ctrl+C, initiating shutdown");
                shutdown_handle.shutdown();
            }
            Err(err) => {
                error!("Unable to listen for shutdown signal: {}", err);
            }
        }
    });

    // Create a channel for device messages
    let (device_msg_tx, mut device_msg_rx) =
        tokio::sync::mpsc::channel::<oxide_gnss::device::DeviceMessage>(64);

    // Create a channel for NTRIP messages
    let (ntrip_msg_tx, mut ntrip_msg_rx) = tokio::sync::mpsc::channel(64);

    // Create a channel for forwarding messages to the supervisor
    let supervisor_msg_tx = supervisor.msg_tx().clone();

    // Spawn task to forward device messages to the supervisor
    let supervisor_msg_tx_device = supervisor_msg_tx.clone();
    tokio::spawn(async move {
        while let Some(msg) = device_msg_rx.recv().await {
            // Convert and forward messages that need to reach ROS/supervisor
            // Internal messages (Covariance, PosEcef, etc.) return None and are skipped
            if let Some(gnss_msg) = msg.into_gnss_message() {
                let _ = supervisor_msg_tx_device.send(gnss_msg).await;
            }
        }
    });

    let supervisor_msg_tx_ntrip = supervisor_msg_tx.clone();
    tokio::spawn(async move {
        while let Some(msg) = ntrip_msg_rx.recv().await {
            // Forward state changes for diagnostics
            if let oxide_gnss::ntrip::NtripMessage::StateChanged(state) = msg {
                let _ = supervisor_msg_tx_ntrip
                    .send(oxide_gnss::state::GnssMessage::NtripStateChanged(state))
                    .await;
            }
        }
    });

    // Create channels for device task
    let device_channels = DeviceTaskChannels {
        rtcm_rx: supervisor.take_rtcm_rx(),
        gga_tx: supervisor.gga_tx(),
        msg_tx: device_msg_tx,
        shutdown_rx: supervisor.shutdown_rx(),
    };

    // Spawn device task with resolved u-blox config and enabled topics
    let (_, _device_handle) =
        spawn_device_task(config.device, ublox_config, enabled_topics, device_channels);

    // Spawn NTRIP task if configured
    let _ntrip_handle = if let Some(ntrip_config) = config.ntrip {
        let ntrip_channels = NtripTaskChannels {
            rtcm_tx: supervisor.rtcm_tx(),
            gga_rx: supervisor.gga_rx(),
            msg_tx: ntrip_msg_tx,
            shutdown_rx: supervisor.shutdown_rx(),
        };

        let (_, handle) = spawn_ntrip_task(ntrip_config, ntrip_channels);
        Some(handle)
    } else {
        None
    };

    // Create ROS task
    let ros_channels = RosTaskChannels {
        msg_rx: supervisor.take_msg_rx(),
        shutdown_rx: supervisor.shutdown_rx(),
    };

    let ros_config = RosTaskConfig {
        diagnostics_rate_hz,
        integrity_rate_hz,
    };

    let _ros_handle = spawn_ros_task(publishers, ros_channels, ros_config);

    // Spin the node
    info!("Node is ready, spinning...");

    // Main loop
    loop {
        // Check if shutdown was requested
        if *supervisor.shutdown_rx().borrow() {
            info!("Shutdown requested, stopping node");
            break;
        }

        // Process ROS callbacks (spin once with timeout)
        let errors = executor.spin(SpinOptions::spin_once().timeout(Duration::from_millis(100)));
        for e in &errors {
            // Ignore timeout errors - they're expected when there's nothing to process
            if !e.to_string().contains("Timeout") {
                warn!("Error spinning ROS2 executor: {}", e);
            }
        }

        // Small sleep to avoid busy loop
        sleep(Duration::from_millis(10)).await;
    }

    // Wait for tasks to finish
    info!("Waiting for tasks to finish...");
    sleep(Duration::from_millis(500)).await;

    info!("Node shutdown complete");
    Ok(())
}
