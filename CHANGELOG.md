# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

Initial public release.

### Added

#### Core Features
- **ROS2 Jazzy driver** for u-blox ZED-F9P GNSS receivers via ros2-rust
- **Integrated NTRIP client** for RTK corrections with TLS/HTTPS support
- **Safety integrity monitoring** with configurable thresholds and go/no-go signal
- **Jamming and spoofing detection** via SEC-SIG message
- **Comprehensive diagnostics** including antenna status and satellite info

#### Configuration System
- **Mode-based configuration** with 6 operating modes:
  - `standalone` - Basic GPS without corrections
  - `rover_ntrip` - RTK rover with NTRIP corrections
  - `rover_radio` - RTK rover with radio corrections
  - `moving_base` - Moving base station
  - `moving_base_rover` - Rover in moving base + rover pair
  - `static_base` - Static base station
- **Feature flags** for optional functionality: `high_precision`, `integrity`, `satellites`, `heading`
- **Automatic port/protocol optimization** - Modes automatically disable unused ports (UART1, SPI) and protocols (NMEA) to reduce CPU load
- **Configurable namespace** in launch file for running multiple nodes (e.g., gnss_base, gnss_rover)
- Config files for all modes with example YAML configurations

#### ROS2 Topics
- `~/fix` (sensor_msgs/NavSatFix) - Position with covariance
- `~/velocity` (geometry_msgs/TwistWithCovarianceStamped) - Velocity
- `~/time_reference` (sensor_msgs/TimeReference) - GPS timestamp
- `/diagnostics` (diagnostic_msgs/DiagnosticArray) - Health status
- `~/integrity` (oxide_gnss_msgs/OxideIntegrity) - Safety integrity status (optional)
- `~/operational` (std_msgs/Bool) - Go/no-go decision (optional)
- `~/satellites` (oxide_gnss_msgs/OxideSatellites) - Per-satellite info (optional)
- `~/baseline_pose` (geometry_msgs/PoseWithCovarianceStamped) - Moving base/rover (optional)

#### UBX Protocol Support
- NAV-PVT, NAV-HPPOSLLH, NAV-SAT, NAV-COV, NAV-POSECEF, NAV-RELPOSNED
- NAV-PL (protection levels for integrity monitoring)
- SEC-SIG, SEC-SIGLOG (jamming/spoofing detection)
- RXM-COR (correction status)
- MON-HW, MON-RF, MON-COMMS (hardware monitoring)

#### Configuration Options
- **Configurable channel buffer sizes** (`channels.message_capacity`, `channels.rtcm_capacity`) for tuning memory usage and backpressure behavior

#### Error Handling
- **Structured error context** with timeout duration and config key details for easier debugging
- **Warn-level logging** for stale data detection with configurable grace period

#### Documentation
- Comprehensive user and developer documentation
- Architecture documentation with async task model
- Integrity monitoring guide (docs/INTEGRITY.md)
- Configuration reference (docs/CONFIGURATION.md)
- ROS2 topic reference (docs/TOPICS.md)
- CONTRIBUTING.md with development workflow
- ROADMAP.md with planned features

#### Testing
- 167 unit tests covering all modules
- 22 integration tests for async task coordination
- Multi-distro CI (Humble, Jazzy, Kilted)
