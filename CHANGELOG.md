# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Default file log filter is now `info` globally. Set `OXIDE_GNSS_FILE_LOG=oxide_gnss=debug` to re-enable debug-level events in the file appender; `RUST_LOG` continues to control stderr independently.

No public API or topic-content changes.

## [0.2.0] - 2026-05-13

### Changed

- Lower steady-state CPU usage (~35–45% across all supported modes), release binary size (~22%), and process memory footprint (~2–3.5%).
- ROS control-plane callbacks (parameter and service handlers, timers) are now serviced immediately rather than within a ~110 ms polling cycle. Data-plane publish latency is unchanged.

No public API or topic-content changes.

## [0.1.0] - 2026-04-19

Initial public release.

- ROS 2 driver for u-blox ZED-F9P GNSS receivers via ros2-rust; CI against Humble, Jazzy, and Kilted on amd64 and arm64.
- Integrated NTRIP client with TLS/HTTPS and GGA uplink for VRS mountpoints.
- Mode-based configuration: `standalone`, `rover_ntrip`, `rover_radio`, `moving_base`, `moving_base_rover`, `static_base`.
- Optional features: `high_precision`, `integrity`, `satellites`, `heading`.
- Safety integrity monitoring with configurable thresholds, jamming/spoofing detection, and a `~/operational` go/no-go topic.
- UBX messages consumed: NAV-PVT, NAV-HPPOSLLH, NAV-SAT, NAV-COV, NAV-POSECEF, NAV-RELPOSNED, NAV-SVIN, NAV-PL, SEC-SIG, SEC-SIGLOG, RXM-COR, MON-HW, MON-RF, MON-COMMS.
- Published topics: `~/fix`, `~/velocity`, `~/time_reference`, `~/integrity`, `~/operational`, `~/satellites`, `~/baseline_pose`, `/diagnostics`.
- `oxide_gnss_assign_serial` CLI for restoring a blank USB serial after a factory reset so udev symlinks line up again.
- Configurable channel buffer sizes (`channels.message_capacity`, `channels.rtcm_capacity`).
- Namespace-configurable launch file for running multiple nodes concurrently (e.g. `gnss_base`, `gnss_rover`).
