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
- NAV-SVIN (survey-in status; surfaced as a `{node}: SurveyIn` substatus on `/diagnostics` with `active`, `valid`, `mean_acc_mm`, `duration_s`, `observations` — only when the receiver is running survey-in)
- NAV-PL (protection levels for integrity monitoring)
- SEC-SIG, SEC-SIGLOG (jamming/spoofing detection)
- RXM-COR (correction status)
- MON-HW, MON-RF, MON-COMMS (hardware monitoring)

#### Configuration Options
- **Configurable channel buffer sizes** (`channels.message_capacity`, `channels.rtcm_capacity`) for tuning memory usage and backpressure behavior

#### Tooling
- **`oxide_gnss_assign_serial` CLI** — one-shot admin binary that probes a ZED-F9P's USB serial via `CFG-VALGET` and, if it's blank (or `--force`), writes the chosen value to RAM + BBR + FLASH so udev symlinks line up after a factory reset. Refuses to collide with serials already in use by other tty devices on the host. See [USER_MANUAL.md §8.7](docs/USER_MANUAL.md#87-recovering-a-factory-reset-f9p-blank-usb-serial--no-devgnss_f9p_serial) for usage.

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

### Fixed

- `device.navigation.rate_hz` now derives `CFG-RATE-MEAS` when `device.ublox.rate.measurement_ms` is unset (was previously validation-only; the Hz knob didn't reach the receiver)
- Non-base modes now force `CFG-TMODE-MODE = Disabled` unless the user sets `device.ublox.base_position`, preventing a prior `static_base` session from leaving the receiver in SurveyIn/Fixed mode (observed on-bench as `fixType = 5` "Time Only" with no RTCM output)
- `heading` feature is now authoritative for `moving_base_rover`: it gates both the `~/baseline_pose` topic and the `NAV_RELPOSNED` UBX message (previously `NAV_RELPOSNED` was always on and `~/baseline_pose` followed the mode rather than the feature flag)
- USB-CDC disconnects on newer kernels where `read()` returns `Ok(0)` (EOF) instead of a hard error are now detected and trigger reconnect
- Added a packet-level watchdog (`device.packet_watchdog_secs`, default 3.0 s) that fires when reads keep returning zero bytes while the device is `Active` — catches stuck firmware, silenced MSGOUT output, and USB-CDC EOF cases that the existing `device.watchdog_timeout_secs` (read-never-returns) does not cover
