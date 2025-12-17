# oxide_gnss

A Rust-based ROS2 GNSS driver for u-blox ZED-F9P receivers with integrated NTRIP client.

## Features

- **Rust implementation** — Memory-safe, reliable operation
- **Integrated NTRIP client** — RTK corrections for centimeter-level accuracy
- **Safety integrity monitoring** — Aggregated quality metrics with go/no-go signal
- **Comprehensive diagnostics** — Jamming/spoofing detection, antenna status
- **ROS2 Jazzy** — Modern ROS2 support via ros2-rust

## Status

🚧 **Under Development** — Not yet production-ready.

## Supported Hardware

| Device | Status |
|--------|--------|
| u-blox ZED-F9P | ✅ Primary target |
| u-blox ZED-F9R | 🔄 Planned |

## Quick Start

### Prerequisites

- ROS2 Jazzy (Ubuntu 24.04)
- Rust stable (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- libclang-dev (`sudo apt install libclang-dev`)

See [Development Guide](docs/DEVELOPMENT.md) for complete setup instructions.

### Install Udev Rules

```bash
sudo cp udev/99-oxide-gnss.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG dialout $USER  # Then log out/in
```

### Build

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash
colcon build --packages-up-to oxide_gnss
```

### Launch

```bash
source ~/ros2_ws/install/setup.bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/src/oxide_gnss/config/default.yaml
```

## Configuration

Configuration is via YAML files. Copy and customize `config/default.yaml`:

```yaml
device:
  port: "/dev/gnss_f9p_rover"
  baud_rate: 460800
  frame: ENU
  ublox:
    messages:
      usb:
        NAV_PVT: 1
        NAV_HPPOSLLH: 1
        MON_RF: 1
        SEC_SIG: 1

ntrip:
  host: "ntrip.data.gnss.ga.gov.au"
  port: 2101
  mountpoint: "SWTC00AUS0"
  username: "${NTRIP_USERNAME}"
  password: "${NTRIP_PASSWORD}"
```

See [Configuration Reference](docs/CONFIGURATION.md) for all options.

## ROS2 Topics

| Topic | Type | Description |
|-------|------|-------------|
| `~/fix` | `sensor_msgs/NavSatFix` | Position with covariance |
| `~/velocity` | `geometry_msgs/TwistWithCovarianceStamped` | 3D velocity |
| `~/time_reference` | `sensor_msgs/TimeReference` | GPS time |
| `~/hp_pos` | `sensor_msgs/NavSatFix` | High-precision position |
| `~/baseline_pose` | `geometry_msgs/PoseWithCovarianceStamped` | Moving base/rover baseline |
| `~/integrity` | `oxide_gnss_msgs/OxideIntegrity` | Safety integrity status |
| `~/operational` | `std_msgs/Bool` | Go/no-go signal |
| `/diagnostics` | `diagnostic_msgs/DiagnosticArray` | System diagnostics |

## Safety Integrity

The driver aggregates quality metrics from multiple UBX messages:

| Level | Value | Meaning |
|-------|-------|---------|
| **OK** | 0 | All checks pass, full operation |
| **DEGRADED** | 1 | Some checks failed, reduced capability |
| **CRITICAL** | 2 | Critical failure, stop operation |
| **FAILED** | 3 | System unavailable |

Data sources: NAV-PVT, NAV-COV, SEC-SIG, MON-RF, MON-COMMS, RXM-COR

## Documentation

- [Configuration Reference](docs/CONFIGURATION.md) — All config options, message requirements, topics
- [Development Guide](docs/DEVELOPMENT.md) — Building, architecture, contributing

## License

MIT License — see [LICENSE](LICENSE)

## References

- [u-blox F9 Interface Description](https://www.u-blox.com/en/docs/UBX-18010854)
- [ros2-rust](https://github.com/ros2-rust/ros2_rust)
