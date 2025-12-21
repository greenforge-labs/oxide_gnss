# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **ROS2 Jazzy driver** for u-blox ZED-F9P GNSS receivers via ros2-rust
- **Integrated NTRIP client** for RTK corrections with TLS support
- **Safety integrity monitoring** with configurable thresholds and go/no-go signal
- **Mode-based configuration** (standalone, rover_ntrip, moving_base, etc.)
- **Jamming and spoofing detection** via SEC-SIG message
- **Comprehensive diagnostics** including antenna status and satellite info
- ROS2 topics: ~/fix, ~/velocity, ~/time_reference, ~/integrity, ~/operational, ~/satellites, ~/baseline_pose
- **Automatic port/protocol optimization** - Modes automatically disable unused ports (UART1, SPI) and protocols (NMEA) to reduce CPU load
- **Configurable namespace** in launch file for running multiple nodes (e.g., gnss_base, gnss_rover)
- Config files for all modes: rover_ntrip, rover_radio, standalone, moving_base, moving_base_rover, static_base
