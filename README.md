# oxide_gnss

A Rust-based ROS2 GNSS driver for u-blox ZED-F9P devices with integrated NTRIP client.

## Overview

oxide_gnss provides a reliable, safety-focused GNSS driver for ROS2 applications. It connects to u-blox ZED-F9P GNSS receivers and publishes position, velocity, and diagnostic information to ROS2 topics.

### Key Features

- **Rust implementation** — Memory-safe, reliable operation
- **Integrated NTRIP client** — Receive RTK corrections from casters like AusCORS
- **Comprehensive diagnostics** — Safety-critical status monitoring
- **ROS2 Jazzy+** — Modern ROS2 support via ros2-rust

## Status

🚧 **Under Development** — Not yet ready for production use.

## Supported Hardware

| Manufacturer | Model | Status |
|--------------|-------|--------|
| u-blox | ZED-F9P | 🎯 Exclusive target |

## Quick Start

See the [Building](#building) section for installation.

Once built, source the workspace:
```bash
source install/setup.bash
```

Then follow the [Launching](#launching) instructions below.

## Configuration

Configuration is via YAML files. See the `config/` directory for examples.

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

See [CONFIGURATION.md](docs/CONFIGURATION.md) for full details.

## ROS2 Interface

### Published Topics

Topics are created based on your mode and feature configuration:

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
| `~/satellites` | `std_msgs/String` | `satellites: true` |
| `~/baseline_pose` | `geometry_msgs/PoseWithCovarianceStamped` | `mode: moving_base_rover` |

### Safety Integrity Monitoring

The driver includes built-in safety integrity monitoring that aggregates quality metrics from multiple UBX messages.

**Integrity Levels:**
- **OK (0)** — All checks pass, full operation permitted
- **DEGRADED (1)** — Some checks failed, reduced capability recommended
- **CRITICAL (2)** — Critical checks failed, operation should stop
- **FAILED (3)** — System unavailable or data stale

**Data Sources:**
- NAV-PVT: Fix type, satellites, accuracy, PDOP
- NAV-COV: Position/velocity covariance matrices
- SEC-SIG: Jamming/spoofing detection
- SEC-SIGLOG: Security event log
- RXM-COR: Differential correction status
- MON-COMMS: Communication port status
- MON-HW: Antenna status, jamming indicator

**Example integrity JSON:**
```json
{
  "level": 0,
  "level_name": "OK",
  "status_message": "All integrity checks passed",
  "fix_type": 5,
  "num_satellites": 12,
  "h_accuracy_m": 0.02,
  "jamming_state": "Ok",
  "spoofing_state": "Ok",
  "operational": true
}
```

### Diagnostics

The driver publishes comprehensive diagnostics including:

- Fix type (No fix / 2D / 3D / RTK Float / RTK Fixed)
- Number of satellites used
- HDOP / PDOP values
- Age of differential corrections
- NTRIP connection status

## Building

### Prerequisites

- **ROS2** — Humble, Jazzy, Kilted, or Rolling (Jazzy on Ubuntu 24.04 recommended)
- **Rust toolchain** — stable, install via [rustup](https://rustup.rs/)
  ```bash
  # Install Rust using rustup (recommended over apt)
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source ~/.cargo/env
  
  # Set the default toolchain
  rustup default stable
  ```
- **libclang-dev** — required for bindgen: `sudo apt install libclang-dev`
- **vcstool** — for importing repos: `sudo apt install python3-vcstool`
- **minicom** — for testing serial communication: `sudo apt install minicom`
- **python3-pip** — for installing colcon plugins: `sudo apt install python3-pip`

### Development Environment Setup

The ros2-rust ecosystem requires some setup before building. This only needs to be done once per workspace.

#### 1. Install colcon-cargo plugins

Ubuntu 24.04 uses PEP 668 which restricts pip installs. For ROS2 build tools, use `--break-system-packages`:

```bash
# Install colcon-cargo plugins with --break-system-packages flag (required for Ubuntu 24.04)
pip install --break-system-packages \
    git+https://github.com/colcon/colcon-cargo.git \
    git+https://github.com/colcon/colcon-ros-cargo.git

# Install cargo-ament-build (required for colcon to build Rust packages)
cargo install cargo-ament-build
```

#### 2. Set up workspace with ros2-rust

```bash
# Create workspace
mkdir -p ~/ros2_ws/src
cd ~/ros2_ws/src

# Clone this package (includes oxide_gnss driver and oxide_gnss_msgs)
git clone https://github.com/gsokoll/oxide_gnss.git

# Clone ros2-rust
git clone https://github.com/ros2-rust/ros2_rust.git

# Import ros2-rust dependencies (message packages with Rust bindings)
vcs import . < ros2_rust/ros2_rust_jazzy.repos
```

#### 3. Initial build

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash

# Build message definitions first (must complete before driver)
colcon build --packages-select oxide_gnss_msgs \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs \
    sensor_msgs diagnostic_msgs action_msgs

# Source to make messages available
source install/setup.bash

# Build the driver
colcon build --packages-select oxide_gnss \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs \
    sensor_msgs diagnostic_msgs action_msgs

# Source to make driver available
source install/setup.bash
```

This creates `.cargo/config.toml` in the workspace root with patches that redirect ROS2 crates to the locally-built versions.

### Rebuilding

After the initial setup, you can rebuild with:

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash

# Rebuild just the driver (fast - messages rarely change)
colcon build --packages-select oxide_gnss \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs \
    sensor_msgs diagnostic_msgs action_msgs

source install/setup.bash
```

If you've modified `oxide_gnss_msgs`, rebuild both:

```bash
colcon build --packages-select oxide_gnss_msgs oxide_gnss \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs \
    sensor_msgs diagnostic_msgs action_msgs

source install/setup.bash
```

### Pre-commit Checks

Before pushing, run these checks to match CI:

```bash
# Format check
cargo fmt --all -- --check

# Clippy lints (treats warnings as errors)
cargo clippy --features ros2 -- -D warnings

# Run tests
cargo test --features ros2
```

### Troubleshooting

| Problem | Solution |
|---------|----------|
| `No task extension to 'build' a 'ros.ament_cargo' package` | Install colcon-cargo plugins (step 1) |
| `failed to resolve patches` | Delete `.cargo/` and `install/` dirs, rebuild |
| `externally-managed-environment` error | Use `--break-system-packages` with pip |
| ROS2 crates not found by cargo | Run colcon build first to generate patches |
| `command 'cargo' not found` | Run `source ~/.cargo/env` after installing rustup |
| `error: no matching package named 'builtin_interfaces' found` | Run `colcon build --packages-up-to oxide_gnss` |
| `Config file not found` / `os error 2` | Use absolute path for `config_file` param |
| `Permission denied (os error 13)` opening `/dev/gnss_*` | Ensure your user is in the `dialout` group (`sudo usermod -aG dialout $USER` then log out/in), or set `MODE="0666"` in the udev rule to relax device permissions |
| `CMake Error: The source directory ... does not exist` | Stale build cache. Run `rm -rf build/ install/` and rebuild |

## Quick Start

### 1. Install Udev Rules (Required for Device Stability)

To ensure consistent device naming, install the provided udev rules:

```bash
# Copy rules to system directory
sudo cp src/oxide_gnss/udev/99-oxide-gnss.rules /etc/udev/rules.d/

# Reload rules and trigger
sudo udevadm control --reload-rules && sudo udevadm trigger
```

This creates symbolic links like `/dev/gnss_f9p_YOURSERIAL` for your ZED-F9P device.

By default, these serial devices are typically owned by `root:dialout` with
mode `0660`. The **recommended** approach is to ensure your user is in the
`dialout` group so that ROS2 nodes can open the ports without root:

```bash
sudo usermod -aG dialout $USER
# Log out and back in (or reboot) for the new group membership to take effect
```

On single-user development machines where strict permissions are less
important, you may **as a last resort** set `MODE="0666"` in the udev rules to
allow all users to access the device.

### 2. Launching

**Option 1: From Workspace Root (Recommended)**
```bash
cd ~/ros2_ws
ros2 launch oxide_gnss oxide_gnss.launch.py config_file:=src/oxide_gnss/config/rover_ntrip.yaml
```

**Option 2: Using Absolute Path (Safest)**
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py config_file:=/home/user/ros2_ws/src/oxide_gnss/config/rover_ntrip.yaml
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## References

- [u-blox ZED-F9P Interface Description](https://www.u-blox.com/en/docs/UBX-18010854)
- [RTCM Standard 10403.3](https://rtcm.myshopify.com/)
- [ros2-rust](https://github.com/ros2-rust/ros2_rust)
