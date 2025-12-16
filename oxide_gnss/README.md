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

Configuration is via YAML files. See `config/` directory for examples.

### Device Configuration

```yaml
device:
  port: "/dev/ttyACM0"
  baud_rate: 460800
  frame: "ENU"  # or "NED"
  reconnect:
    enabled: true
    initial_delay_secs: 1
    max_delay_secs: 30
    max_attempts: 0  # 0 = unlimited retries with exponential backoff
    # Recommended settings for safety-critical use:
    # - enabled: true
    # - initial_delay_secs: 1
    # - max_delay_secs: 30
    # - max_attempts: 0 (unlimited retries)
    # This configuration allows the driver to continuously attempt to reconnect to the device
    # with an exponential backoff strategy, ensuring that the system can recover from temporary
    # device disconnections or communication errors.
```

### NTRIP Configuration

```yaml
ntrip:
  enabled: true
  host: "auscors.ga.gov.au"
  port: 2101
  mountpoint: "ALIC00AUS0"
  username: "your_username"
  password: "your_password"
  gga_interval_sec: 10
```

## ROS2 Interface

### Published Topics

| Topic | Type | Description |
|-------|------|-------------|
| `~/fix` | `sensor_msgs/NavSatFix` | Position with covariance |
| `~/velocity` | `geometry_msgs/TwistWithCovarianceStamped` | 3D velocity |
| `~/time_reference` | `sensor_msgs/TimeReference` | GPS time |
| `~/diagnostics` | `diagnostic_msgs/DiagnosticArray` | Device status |
| `~/hp_pos` | `sensor_msgs/NavSatFix` | High-precision position (UBX-NAV-HPPOSLLH) |
| `~/satellites` | `std_msgs/String` | Satellite visibility info (UBX-NAV-SAT) |
| `~/integrity` | `oxide_gnss_msgs/GnssIntegrity` | Safety integrity status (Typed Message) |
| `~/operational` | `std_msgs/Bool` | Go/no-go signal for safety-critical operation |

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

- **ROS2 Jazzy** (Ubuntu 24.04 / WSL2 recommended)
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

# Clone this package
git clone https://github.com/gsokoll/oxide_gnss.git

# Ensure the oxide_gnss_msgs ROS2 interface package is present in this workspace (~/ros2_ws/src)

# Clone ros2-rust
git clone https://github.com/ros2-rust/ros2_rust.git

# Import ros2-rust dependencies (message packages with Rust bindings)
vcs import . < ros2_rust/ros2_rust_jazzy.repos
```

#### 3. Initial build (generates cargo patches)

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash

# Build ros2-rust packages first, allowing overrides
colcon build --packages-up-to oxide_gnss_msgs oxide_gnss \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs \
    sensor_msgs diagnostic_msgs action_msgs
```

This creates `.cargo/config.toml` in the workspace root with patches that redirect ROS2 crates to the locally-built versions.

> **Important**: The specific order of operations is critical:
> 1. First source ROS2: `source /opt/ros/jazzy/setup.bash`
> 2. Then build ros2-rust dependencies with colcon
> 3. Source the workspace: `source ~/ros2_ws/install/setup.bash`
> 4. Finally build oxide_gnss with cargo: `cargo build --features ros2`


### Building

#### Option A: Build with colcon (recommended for deployment)

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash

# First time build (builds dependencies)
colcon build --packages-up-to oxide_gnss

# Subsequent builds (driver only)
colcon build --packages-select oxide_gnss
```

#### Option B: Build with cargo (fast iteration for development)

After the initial colcon build, you can iterate faster with cargo:

```bash
source /opt/ros/jazzy/setup.bash
source ~/ros2_ws/install/setup.bash
cd ~/ros2_ws/src/oxide_gnss

# Build binary
cargo build
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

## Quick Start

### 1. Install Udev Rules (Required for Device Stability)

To ensure consistent device naming and support for ZED-X20 devices, install the provided udev rules:

```bash
# Copy rules to system directory
sudo cp src/oxide_gnss/udev/99-oxide-gnss.rules /etc/udev/rules.d/

# Reload rules and trigger
sudo udevadm control --reload-rules && sudo udevadm trigger
```

This creates symbolic links:
- `/dev/gnss_f9p_YOURSERIAL` for ZED-F9P
- `/dev/gnss_x20_YOURSERIAL` for ZED-X20 (and handles `cdc_acm` loading)

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

#### Single Device (Rover)


**Option 1: From Workspace Root (Recommended)**
```bash
cd ~/ros2_ws
ros2 launch oxide_gnss oxide_gnss.launch.py config_file:=src/oxide_gnss/config/default.yaml
```

**Option 2: Using Absolute Path (Safest)**
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py config_file:=/home/gtec/ros2_ws/src/oxide_gnss/config/default.yaml
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

_TBD_

## References

- [u-blox ZED-F9P Interface Description](https://www.u-blox.com/en/docs/UBX-18010854)
- [RTCM Standard 10403.3](https://rtcm.myshopify.com/)
- [ros2-rust](https://github.com/ros2-rust/ros2_rust)
