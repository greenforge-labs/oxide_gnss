# Development Guide

This document covers building, developing, and contributing to oxide_gnss.

## Prerequisites

### Required

| Dependency | Purpose | Install |
|------------|---------|----------|
| ROS2 | ROS2 framework | Humble, Jazzy, or Rolling ([install](https://docs.ros.org/en/jazzy/Installation.html)) |
| Rust (stable) | Compiler | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| libclang-dev | Bindgen (FFI) | `sudo apt install libclang-dev` |
| vcstool | Repo import | `sudo apt install python3-vcstool` |

### Optional

| Dependency | Purpose | Install |
|------------|---------|----------|
| Docker | Local CI testing | [Install Docker](https://docs.docker.com/engine/install/ubuntu/) |
| minicom | Serial debugging | `sudo apt install minicom` |
| just | Task runner | `cargo install just` |

---

## Initial Setup

### 1. Install colcon-cargo Plugins

Ubuntu 24.04 uses PEP 668. Use `--break-system-packages` for ROS2 build tools:

```bash
pip install --break-system-packages \
    git+https://github.com/colcon/colcon-cargo.git \
    git+https://github.com/colcon/colcon-ros-cargo.git

cargo install cargo-ament-build
```

### 2. Create Workspace

```bash
mkdir -p ~/ros2_ws/src
cd ~/ros2_ws/src

# Clone oxide_gnss
git clone https://github.com/gsokoll/oxide_gnss.git

# Clone ros2-rust
git clone --branch v0.7.0 https://github.com/ros2-rust/ros2_rust.git

# Import ros2-rust dependencies
vcs import . < ros2_rust/ros2_rust_jazzy.repos

# Create symlink for message package (required for colcon to discover it)
ln -s oxide_gnss/oxide_gnss_msgs oxide_gnss_msgs
```

> **Note:** The `oxide_gnss_msgs` package is nested inside `oxide_gnss` for source control,
> but colcon requires it at the `src/` level. The symlink makes it discoverable while
> keeping the source in the main repository.

### 3. Initial Build

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash

# Build ros2-rust packages (creates cargo patches)
colcon build --packages-up-to oxide_gnss_msgs oxide_gnss \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs \
    sensor_msgs diagnostic_msgs action_msgs
```

This creates `.cargo/config.toml` with patches redirecting ROS2 crates to locally-built versions.

---

## Building

### Option A: colcon (Deployment)

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash

# Full build
colcon build --packages-up-to oxide_gnss

# Driver only (faster)
colcon build --packages-select oxide_gnss
```

### Option B: cargo (Development)

After initial colcon build, iterate faster with cargo:

```bash
source /opt/ros/jazzy/setup.bash
source ~/ros2_ws/install/setup.bash
cd ~/ros2_ws/src/oxide_gnss

cargo build --features ros2
```

### Using just (Recommended)

The project includes a `justfile` for common tasks:

```bash
just build      # cargo build
just check      # cargo check
just test       # cargo test
just clippy     # lint check
just fmt        # format code
just ci         # all CI checks (fmt, clippy, test)
```

---

## Development Workflow

### Source Order (Critical)

Always source in this order:

```bash
source /opt/ros/jazzy/setup.bash      # 1. ROS2 base
source ~/ros2_ws/install/setup.bash   # 2. Workspace overlay
```

### Fast Iteration Cycle

```bash
# Terminal 1: Build on change
cd ~/ros2_ws/src/oxide_gnss
cargo watch -x 'build --features ros2'

# Terminal 2: Run tests
cargo watch -x 'test --features ros2'
```

### Running the Node

```bash
# From workspace root (recommended)
cd ~/ros2_ws
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=src/oxide_gnss/config/rover_ntrip.yaml

# With absolute path
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=/home/user/ros2_ws/src/oxide_gnss/config/rover_ntrip.yaml
```

---

## Code Quality

### Pre-commit Checks

Run before pushing to match CI:

```bash
# Format
cargo fmt --all -- --check

# Lints (warnings as errors)
cargo clippy --features ros2 -- -D warnings

# Tests
cargo test --features ros2
```

Or use just:
```bash
just ci
```

### Code Style

- Follow Rust standard style (`rustfmt`)
- No `unwrap()` in production code paths
- Use `tracing` for logging (not `println!`)
- Document public APIs with rustdoc

---

## Architecture Overview

```
oxide_gnss/                 # Repository root
├── oxide_gnss_msgs/       # ROS2 message definitions
│   ├── CMakeLists.txt
│   └── msg/
│       ├── OxideSatellite.msg   # Per-satellite data
│       ├── OxideSatellites.msg  # Satellite constellation status
│       └── OxideIntegrity.msg   # Integrity monitoring
├── src/
│   ├── main.rs           # Entry point (ROS2 node)
│   ├── lib.rs            # Library root
│   ├── config/           # Configuration parsing
│   │   ├── mod.rs        # Config loading, mode resolution
│   │   ├── modes.rs      # Operating modes and features
│   │   ├── ublox.rs      # UbloxConfig, message requirements
│   │   ├── device.rs     # DeviceConfig
│   │   └── ntrip.rs      # NtripConfig
│   ├── device/           # Hardware interface
│   │   ├── mod.rs
│   │   ├── task.rs       # Device communication task
│   │   ├── ubx.rs        # UBX protocol parser
│   │   ├── serial.rs     # Serial port handling
│   │   ├── config.rs     # Device configurator
│   │   └── cfg_key_mapping.rs  # UbloxConfig -> CfgVal
│   ├── ntrip/            # NTRIP client
│   │   ├── mod.rs
│   │   └── task.rs       # NTRIP communication task
│   ├── ros/              # ROS2 interface
│   │   ├── mod.rs
│   │   ├── node.rs       # GnssNode, GnssNodeConfig
│   │   ├── task.rs       # ROS publisher task
│   │   ├── publishers.rs # Topic publishers (mode-aware)
│   │   └── conversions.rs # UBX -> ROS message conversion
│   └── state/            # State management
│       ├── mod.rs
│       ├── supervisor.rs # Task coordination
│       └── integrity.rs  # Safety integrity aggregation
├── config/
│   ├── rover_ntrip.yaml  # RTK rover with NTRIP
│   ├── standalone.yaml   # Basic GPS
│   ├── moving_base.yaml  # Moving base station
│   └── static_base.yaml  # Static base station
├── launch/
│   └── oxide_gnss.launch.py
└── docs/
    ├── USER_MANUAL.md
    ├── TOPICS.md
    ├── INTEGRITY.md
    └── DEVELOPMENT.md
```

### Task Model

The driver uses async tasks coordinated by a supervisor:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ Device Task │────▶│ Supervisor  │────▶│  ROS Task   │
│  (serial)   │     │  (routing)  │     │ (publish)   │
└─────────────┘     └─────────────┘     └─────────────┘
                           ▲
                           │
                    ┌─────────────┐
                    │ NTRIP Task  │
                    │ (corrections)│
                    └─────────────┘
```

### Message Flow

1. **Device Task** reads UBX packets from serial port
2. Parsed data becomes `DeviceMessage` variants
3. **Supervisor** routes messages to appropriate handlers
4. **ROS Task** converts to ROS messages and publishes
5. **NTRIP Task** sends RTCM corrections back to device

---

## Testing

### Unit Tests

```bash
cargo test --features ros2
```

### Local CI Testing (Docker)

Test against multiple ROS2 distros locally before pushing:

```bash
# Test all supported distros (humble, jazzy, kilted, rolling)
./scripts/local_ci_test.sh

# Test specific distro
./scripts/local_ci_test.sh jazzy

# Quick check only (faster)
./scripts/local_ci_test.sh jazzy check

# Build only (no tests)
./scripts/local_ci_test.sh humble build
```

This script:
- Uses `rostooling/setup-ros-docker` images
- Installs Rust and ros2-rust from scratch
- Builds and tests oxide_gnss
- Mirrors the GitHub Actions CI environment

### Manual Testing

```bash
# Monitor all topics
ros2 topic list
ros2 topic echo /oxide_gnss_node/fix

# Check diagnostics
ros2 topic echo /diagnostics

# Verify integrity
ros2 topic echo /oxide_gnss_node/integrity
```

### Serial Debugging

```bash
# Check device is connected
ls -la /dev/ttyACM* /dev/gnss_*

# Monitor raw serial (read-only)
minicom -D /dev/gnss_f9p_rover -b 460800
```

---

## Troubleshooting

| Problem | Cause | Solution |
|---------|-------|----------|
| `ignoring unknown package 'oxide_gnss_msgs'` | Missing symlink | Create symlink: `ln -s oxide_gnss/oxide_gnss_msgs oxide_gnss_msgs` in `src/` |
| `No task extension to 'build' a 'ros.ament_cargo' package` | Missing colcon plugins | Install colcon-cargo plugins (see Initial Setup) |
| `failed to resolve patches` | Stale cargo state | Delete `.cargo/` and `install/` dirs, rebuild |
| `externally-managed-environment` | PEP 668 restriction | Use `--break-system-packages` with pip |
| ROS2 crates not found | Missing patches | Run `colcon build` first to generate patches |
| `command 'cargo' not found` | Rust not in PATH | Run `source ~/.cargo/env` |
| `Config file not found` | Relative path issue | Use absolute path for `config_file` param |
| `Permission denied` on `/dev/gnss_*` | User not in dialout | `sudo usermod -aG dialout $USER`, then re-login |
| No data from device | Wrong baud rate | Verify device baud matches config |
| NTRIP not connecting | Network/auth issue | Check credentials, try `curl` to caster |

### Clean Rebuild

When things go wrong:

```bash
cd ~/ros2_ws

# Remove build artifacts
rm -rf build/ install/ log/ .cargo/

# Rebuild from scratch
source /opt/ros/jazzy/setup.bash
colcon build --packages-up-to oxide_gnss
```

---

## Adding New Features

### Adding a New UBX Message

1. **Parse in `device/ubx.rs`**:
   - Add data struct (e.g., `NewMsgData`)
   - Add to `ProcessResult`
   - Add parsing in `process()` method
   - Add parse function

2. **Route through message system**:
   - Add `DeviceMessage::NewMsg` variant in `device/task.rs`
   - Add `GnssMessage::NewMsg` variant in `state/supervisor.rs`
   - Forward in device task

3. **Publish to ROS**:
   - Add publisher in `ros/publishers.rs` (optional if mode-aware)
   - Add conversion in `ros/conversions.rs`
   - Handle in `ros/task.rs`

4. **Document**:
   - Add to `MESSAGE_REQUIREMENTS` in `config/ublox.rs`
   - Update `docs/USER_MANUAL.md` and/or `docs/TOPICS.md`

### Adding a New ROS Topic

1. Add optional publisher in `GnssPublishers` struct
2. Check `enabled_topics` in `GnssPublishers::new()` before creating
3. Add publish method with `Option` check
4. Call from `RosTask::handle_message()`
5. Update `Config::enabled_topics()` in `config/mod.rs`
6. Document in `USER_MANUAL.md` and `TOPICS.md`

### Adding a New Feature Flag

1. Add variant to `Feature` enum in `config/modes.rs`
2. Implement `required_messages()` for the feature
3. Implement `enabled_topics()` for the feature
4. Update `FeaturesConfig` with new field
5. Update `enabled_features()` to include it
6. Add to mode presets' `allowed_features` if appropriate
7. Document in `USER_MANUAL.md` and `TOPICS.md` (and `INTEGRITY.md` if relevant)

### Adding a New Operating Mode

1. Add variant to `OperatingMode` enum in `config/modes.rs`
2. Add preset function in `presets` module
3. Update `ModePreset::for_mode()` match
4. Update `Config::enabled_topics()` if mode has special topics
5. Create example config file in `config/`
6. Document in `USER_MANUAL.md` and `TOPICS.md`

---

## References

- [u-blox F9 Interface Description](https://www.u-blox.com/en/docs/UBX-18010854)
- [ros2-rust](https://github.com/ros2-rust/ros2_rust)
- [RTCM Standard 10403.3](https://rtcm.myshopify.com/)
- [Rust Async Book](https://rust-lang.github.io/async-book/)
