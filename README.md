# oxide_gnss

[![CI](https://github.com/greenforge-labs/oxide_gnss/actions/workflows/ci.yml/badge.svg)](https://github.com/greenforge-labs/oxide_gnss/actions/workflows/ci.yml)

A Rust-based ROS 2 GNSS driver for u-blox receivers (ZED-F9P focus) with an integrated NTRIP client and optional integrity monitoring.  

It has been designed with a focus on providing simple configuration of the ZED-F9P in a range of possible operating modes without needing to delve into the complexity of UBX messages and config flags.

This repository contains:

- `oxide_gnss`: the ROS 2 node
- `oxide_gnss_msgs`: custom message definitions (`OxideIntegrity`, `OxideSatellites`, `OxideSatellite`)

## Documentation

- **User manual (end users):** [`docs/USER_MANUAL.md`](docs/USER_MANUAL.md)
- **ROS 2 topics reference:** [`docs/TOPICS.md`](docs/TOPICS.md)
- **Integrity monitoring:** [`docs/INTEGRITY.md`](docs/INTEGRITY.md)
- **Development guide (build/dev/CI):** [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)

## Overview

`oxide_gnss` connects to a u-blox GNSS receiver and publishes position, velocity, time, and diagnostics to ROS 2 topics. In RTK rover modes it can fetch RTCM corrections via the built-in NTRIP client.

### Key features

- **Rust implementation** — memory-safe, reliable operation
- **Mode-based configuration** — presets for common rover/base scenarios
- **Integrated NTRIP client** — receive RTK corrections from NTRIP casters
- **Integrity monitoring (optional)** — publishes `~/integrity` and `~/operational`
- **ROS diagnostics** — publishes `/diagnostics` for status visibility
- **`oxide_gnss_assign_serial` CLI** — one-shot tool that allows users to write a USB serial string to a F9P without external tools (see [USER_MANUAL.md](docs/USER_MANUAL.md))

## Status

Version 0.1.0. Released from in-house use; only occasional feature development is planned. Bug-fix PRs and forks are welcome, but responses may be slow.

## Supported ROS 2

CI builds and tests against Humble, Jazzy, and Kilted.

### Platform support

| Platform | Architecture | Status |
|---|---|---|
| Ubuntu 24.04 (Noble) | amd64 | CI-tested (Jazzy, Kilted) |
| Ubuntu 24.04 (Noble) | arm64 | CI-tested (Jazzy, Kilted) |
| Ubuntu 22.04 (Jammy) | amd64 | CI-tested (Humble) |

## Supported hardware

| Manufacturer | Model | Notes |
|--------------|-------|-------|
| u-blox | ZED-F9P | Primary target |
| u-blox | Other u-blox devices | May work depending on firmware message support |

## Quick Start

For full build/setup instructions, see [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

Build and source your workspace:
```bash
colcon build --packages-up-to oxide_gnss_msgs oxide_gnss
source install/setup.bash
```

### 1. Install udev rules (recommended)

```bash
sudo cp src/oxide_gnss/udev/99-oxide-gnss.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG dialout $USER
```

Log out/in (or reboot) after changing group membership.

### 2. Launch

```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
  config_file:=/absolute/path/to/your/config.yaml
```

Example configs are in `config/`.

## Configuration

Configuration is via YAML files. See the `config/` directory for examples.

For the full end-user workflow (modes, features, NTRIP, integrity, and a complete parameter reference), see:

- [`docs/USER_MANUAL.md`](docs/USER_MANUAL.md)

### Mode-Based Configuration (Recommended)

The simplest way to configure oxide_gnss is using **modes** and **features**:

```yaml
# RTK rover with NTRIP corrections
mode: rover_ntrip

features:
  high_precision: true    # Use HP data in ~/fix topic
  integrity: true         # Enable ~/integrity monitoring
  satellites: false       # Disable to reduce bandwidth

device:
  port: "/dev/gnss_f9p"
  baud_rate: 460800
  frame: ENU

ntrip:
  host: "ntrip.data.gnss.ga.gov.au"
  port: 2101
  mountpoint: "ALIC00AUS0"
  username: "${NTRIP_USERNAME}"
  password: "${NTRIP_PASSWORD}"
```

### Available Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `standalone` | Basic GPS without RTK | Testing, low-accuracy applications |
| `rover_ntrip` | RTK rover with NTRIP corrections | Most common RTK setup |
| `rover_radio` | RTK rover with radio/serial corrections | Remote areas without internet |
| `moving_base` | Moving base in MB+R pair | Heading from dual receivers |
| `moving_base_rover` | Rover in MB+R pair | Heading from dual receivers |
| `static_base` | Static base station | Providing corrections |

### Feature Flags

| Feature | Description | Topics Enabled |
|---------|-------------|----------------|
| `high_precision` | Use HP position in ~/fix | Enhanced ~/fix accuracy |
| `integrity` | Jamming/spoofing detection | `~/integrity`, `~/operational` |
| `satellites` | Per-satellite info | `~/satellites` |

See also:

- [`docs/TOPICS.md`](docs/TOPICS.md) (topic list, types, and UBX requirements)

## ROS 2 Interface

### Published Topics

Topics are created based on your mode and feature configuration:

For a complete topic reference (including required UBX messages), see [`docs/TOPICS.md`](docs/TOPICS.md).

**Core Topics (always enabled):**

| Topic | Type | Description |
|-------|------|-------------|
| `~/fix` | `sensor_msgs/NavSatFix` | Position with covariance (HP-enhanced if `high_precision: true`) |
| `~/velocity` | `geometry_msgs/TwistWithCovarianceStamped` | 3D velocity |
| `~/time_reference` | `sensor_msgs/TimeReference` | GPS time |
| `/diagnostics` | `diagnostic_msgs/DiagnosticArray` | Device status |

**Optional Topics (based on mode/features):**

| Topic | Type | Requires |
|-------|------|----------|
| `~/integrity` | `oxide_gnss_msgs/OxideIntegrity` | `integrity: true` |
| `~/operational` | `std_msgs/Bool` | `integrity: true` |
| `~/satellites` | `oxide_gnss_msgs/OxideSatellites` | `satellites: true` |
| `~/baseline_pose` | `geometry_msgs/PoseWithCovarianceStamped` | `mode: moving_base_rover` |

### Safety Integrity Monitoring

When `integrity: true`, the driver aggregates jamming/spoofing detection, protection levels, covariance validity, correction age, and antenna status into a four-level state (OK / DEGRADED / CRITICAL / FAILED) published on `~/integrity` and `~/operational`. See [`docs/INTEGRITY.md`](docs/INTEGRITY.md) for the check model, required UBX messages, and configurable thresholds.

### Diagnostics

A standard `/diagnostics` array is always published (fix type, satellites, DOP, correction age, NTRIP status). See [`docs/TOPICS.md`](docs/TOPICS.md) for the full topic list.

## Development

See [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) for:

- Workspace setup (ros2-rust + colcon-cargo)
- Building with `colcon` or `cargo`
- CI parity checks and troubleshooting

Common commands:

```bash
cargo fmt --all -- --check
cargo clippy --features ros2 -- -D warnings
cargo test --features ros2
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Acknowledgments

`oxide_gnss` is built on top of several open-source Rust crates:

- **[ublox](https://github.com/ublox-rs/ublox)** — UBX protocol parser and serializer for u-blox receivers
- **[rtcm-rs](https://github.com/martinhakansson/rtcm-rs)** — RTCM 3.x message parsing, by Martin Håkansson
- **[ntrip-core](https://github.com/greenforge-labs/ntrip-core)** — NTRIP client (also maintained by GreenForge Labs)

Thanks to the maintainers and contributors of these projects.

## References

- [u-blox ZED-F9P Interface Description](https://www.u-blox.com/en/docs/UBX-18010854)
- [RTCM Standard 10403.3](https://rtcm.myshopify.com/)
- [ros2-rust](https://github.com/ros2-rust/ros2_rust)
