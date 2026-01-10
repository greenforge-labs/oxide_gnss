# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

oxide_gnss is a Rust-based ROS2 GNSS driver for u-blox ZED-F9P receivers with integrated NTRIP client. It uses the ros2-rust bindings and async Tokio runtime.

## Build Commands

**Initial workspace setup** (run once from `~/ros2_ws`):
```bash
source /opt/ros/jazzy/setup.bash
colcon build --packages-up-to oxide_gnss_msgs oxide_gnss \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs sensor_msgs diagnostic_msgs action_msgs
source install/setup.bash
```

**Rebuild driver only** (fast iteration):
```bash
source /opt/ros/jazzy/setup.bash
source ~/ros2_ws/install/setup.bash
cd ~/ros2_ws/src/oxide_gnss
cargo build --features ros2
```

**Or use justfile** (from package directory):
```bash
just build      # cargo build --features ros2
just ci         # fmt-check + clippy + test
just test-one NAME  # run single test with output
```

## Testing and CI

All cargo commands require `--features ros2`:
```bash
cargo test --features ros2
cargo clippy --features ros2 -- -D warnings
cargo fmt --all -- --check
```

Local CI against multiple ROS2 distros:
```bash
./scripts/local_ci_test.sh jazzy        # test specific distro
./scripts/local_ci_test.sh jazzy check  # quick cargo check only
```

## Architecture

### Async Task Model

The driver runs three concurrent Tokio tasks coordinated by a Supervisor:

```
DeviceTask (serial) ──→ Supervisor (routing) ──→ RosTask (publish)
                              ↑
                        NtripTask (corrections)
```

- **DeviceTask** (`device/task.rs`): Reads UBX packets from serial, parses them, emits `GnssMessage` variants
- **NtripTask** (`ntrip/task.rs`): Connects to NTRIP casters, receives RTCM data, sends to device
- **RosTask** (`ros/task.rs`): Receives messages, converts to ROS types, publishes to topics
- **Supervisor** (`state/supervisor.rs`): Manages channels, shared state, shutdown coordination

### Message Flow

1. DeviceTask reads raw bytes → parses UBX protocol → creates `DeviceMessage`
2. Messages forwarded as `GnssMessage` variants via `msg_tx` channel
3. RosTask receives, converts UBX data → ROS messages (NavSatFix, TwistWithCovariance, etc.)
4. NTRIP sends GGA position updates via `gga_tx` watch channel for VRS positioning

### Module Structure

- `config/`: YAML parsing, mode resolution, feature flags. `modes.rs` defines operating modes (rover_ntrip, moving_base, etc.)
- `device/`: Hardware interface. `ubx.rs` has UBX protocol parsing, `cfg_key_mapping.rs` maps config to u-blox CFG-VAL keys
- `ros/`: ROS2 interface. Publishers are mode-aware (created based on enabled features)
- `state/`: State management. `supervisor.rs` coordinates tasks, `device_state.rs` has state machine

### Key Patterns

- All ROS2 code is behind `#[cfg(feature = "ros2")]`
- Configuration uses mode presets with optional feature flags (high_precision, integrity, satellites)
- UBX message requirements are declared in `config/ublox.rs` `MESSAGE_REQUIREMENTS`
- No `unwrap()` in production paths; use `tracing` for logging

## Workspace Layout

The `oxide_gnss_msgs` package is nested but requires a symlink at workspace level:
```
~/ros2_ws/src/
├── oxide_gnss/           # this repo
│   └── oxide_gnss_msgs/  # message definitions (source)
├── oxide_gnss_msgs -> oxide_gnss/oxide_gnss_msgs  # symlink for colcon
└── ros2_rust/            # ros2-rust bindings
```

If you get "ignoring unknown package 'oxide_gnss_msgs'", the symlink is missing.

## Adding New Functionality

**New UBX message**: Parse in `device/ubx.rs` → add `DeviceMessage` variant → add `GnssMessage` variant → handle in `ros/task.rs`

**New ROS topic**: Add to `GnssPublishers` → check `enabled_topics` before creation → update `Config::enabled_topics()`

**New feature flag**: Add to `Feature` enum in `config/modes.rs` → implement `required_messages()` and `enabled_topics()`
