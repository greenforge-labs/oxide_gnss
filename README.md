# oxide_gnss

A Rust-based ROS 2 GNSS driver for u-blox receivers (ZED-F9P focus) with an integrated NTRIP client and optional integrity monitoring.

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

## Status

Under active development; configuration and interfaces may change.

## Supported ROS 2

Humble and newer.

## Supported hardware

| Manufacturer | Model | Notes |
|--------------|-------|-------|
| u-blox | ZED-F9P | Primary target |
| u-blox | Other u-blox devices | May work depending on firmware message support |

## Quick Start

For full build/setup instructions, see [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

Once built, source your workspace:
```bash
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

The driver includes built-in safety integrity monitoring that aggregates quality metrics from multiple UBX messages.

For the integrity model, required messages, and threshold meanings, see [`docs/INTEGRITY.md`](docs/INTEGRITY.md).

**Integrity Levels:**
- **OK (0)** — All checks pass, full operation permitted
- **DEGRADED (1)** — Some checks failed, reduced capability recommended
- **CRITICAL (2)** — Critical checks failed, operation should stop
- **FAILED (3)** — System unavailable or data stale

**Diagnostic Visibility:**

The `~/integrity` message includes 15 `check_*` boolean fields showing pass/fail for each individual check. This enables visualization tools like Foxglove to display pass/fail grids, making it easy to identify which specific checks cause state changes.

**Data Sources:**
- NAV-PVT: Fix type, satellites, accuracy, PDOP
- NAV-PL: Protection levels with TMIR (ISO 26262 integrity bounds)
- NAV-COV: Covariance validity (matrices in NavSatFix/Twist)
- SEC-SIG: Jamming/spoofing detection
- SEC-SIGLOG: Security event log
- RXM-COR: Differential correction status
- MON-COMMS: Communication port status
- MON-HW: Antenna status, jamming indicator

### Diagnostics

The driver publishes comprehensive diagnostics including:

- Fix type (No fix / 2D / 3D / RTK Float / RTK Fixed)
- Number of satellites used
- HDOP / PDOP values
- Age of differential corrections
- NTRIP connection status

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

## References

- [u-blox ZED-F9P Interface Description](https://www.u-blox.com/en/docs/UBX-18010854)
- [RTCM Standard 10403.3](https://rtcm.myshopify.com/)
- [ros2-rust](https://github.com/ros2-rust/ros2_rust)
