# Oxide GNSS ROS 2 Driver — User Manual

## Document Purpose

This manual is for **end users** who want to **configure and operate** the `oxide_gnss` ROS 2 node.

It is *not* a developer guide. It focuses on:

- Installing and launching the node
- Common working configuration examples
- Advanced configuration options
- A definitive reference for **all configuration parameters**

This manual assumes you are running on Linux with ROS 2 Humble or newer.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [What Oxide GNSS Provides](#2-what-oxide-gnss-provides)
3. [Quick Start](#3-quick-start)
4. [Operating the Node](#4-operating-the-node)
5. [Common Use Cases (Examples)](#5-common-use-cases-examples)
6. [Advanced Configuration](#6-advanced-configuration)
7. [Integrity Monitoring](#7-integrity-monitoring)
8. [Troubleshooting](#8-troubleshooting)
9. [Definitive Configuration Reference](#9-definitive-configuration-reference)

---

# 1. Introduction

`oxide_gnss` is a Rust-based ROS 2 GNSS driver for u-blox receivers, with a primary focus on **u-blox ZED-F9P**.

It is designed for robotics and autonomy use cases where you need:

- A robust GNSS -> ROS interface
- Optional **RTK corrections via NTRIP**
- Optional **integrity monitoring** (jamming/spoofing/antenna/correction health)

**Supported ROS 2:** Humble and newer.

**Supported hardware:** ZED-F9P is the main target; other u-blox devices may work depending on firmware support for required messages.

## 1.1 Safety and operational disclaimer

Oxide’s integrity monitoring is intended as **quality monitoring and gating** for robotics/autonomy.
It is **not** a certified safety monitor.

## 1.2 Key concepts used throughout this manual

- **Configuration file**: a YAML file passed via the required ROS parameter `config_file`.
- **Mode-based config**: setting `mode: ...` and optional `features: ...` (recommended).
- **Legacy config**: omitting `mode:` and explicitly configuring `device.ublox.*` (advanced).

---

# 2. What Oxide GNSS Provides

## 2.1 Core ROS topics

These topics are always published:

- `~/fix` (`sensor_msgs/NavSatFix`)
- `~/velocity` (`geometry_msgs/TwistWithCovarianceStamped`)
- `~/time_reference` (`sensor_msgs/TimeReference`)
- `/diagnostics` (`diagnostic_msgs/DiagnosticArray`)

## 2.2 Optional ROS topics

Optional topics are only created when enabled via `mode` + `features` (recommended), or in legacy mode by explicitly enabling certain UBX messages.

- `~/integrity` (`oxide_gnss_msgs/OxideIntegrity`)
- `~/operational` (`std_msgs/Bool`)
- `~/satellites` (`oxide_gnss_msgs/OxideSatellites`)
- `~/baseline_pose` (`geometry_msgs/PoseWithCovarianceStamped`) (moving base rover)

### Topic namespaces (`~/...`)

Oxide uses the ROS convention where `~` means "relative to the node".

If you use the provided launch file defaults:

- node namespace: `oxide_gnss`
- node name: `gnss_node`

Then `~/fix` appears as `/oxide_gnss/fix`.

## 2.3 Configuration philosophy

Oxide supports two configuration styles:

- **Mode-based configuration (recommended)**
  - You set `mode: ...` and optional `features: ...`
  - Oxide generates a suitable internal u-blox configuration automatically
  - You can still override/extend it in `device.ublox.*`

- **Legacy configuration (advanced)**
  - You omit `mode:`
  - You explicitly set `device.ublox.messages.*` and other u-blox fields

For almost all deployments, **use mode-based configuration**.

## 2.4 Modes and features at a glance

### Modes

- **`standalone`**
  - Basic GNSS positioning.
- **`rover_ntrip`**
  - RTK rover using corrections from the built-in NTRIP client.
- **`rover_radio`**
  - RTK rover using corrections arriving on receiver UART2 (e.g., radio).
- **`moving_base`**
  - Moving base outputting RTCM on receiver UART2.
- **`moving_base_rover`**
  - Rover receiving RTCM on UART2 and publishing baseline information.
- **`static_base`**
  - Static base station outputting RTCM on USB and/or UART2.

### Features

- **`high_precision`**
  - Uses u-blox high precision position when available to enhance `~/fix`.
- **`integrity`**
  - Publishes `~/integrity` and `~/operational`.
  - Also enables protection level calculation on the receiver when supported.
- **`satellites`**
  - Publishes `~/satellites` (can be higher bandwidth).
- **`heading`**
  - Enables `~/baseline_pose` in moving-base-rover scenarios.

---

# 3. Quick Start

## 3.1 Build and source your workspace

After building with `colcon`, source your workspace:

```bash
source install/setup.bash
```

## 3.2 (Recommended) Install udev rules

Install the provided udev rules for stable device names:

```bash
sudo cp src/oxide_gnss/udev/99-oxide-gnss.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

This creates symlinks like:

- `/dev/gnss_f9p_<serial>`

You can also add your own alias line to create `/dev/gnss_rover` or `/dev/gnss_base` for known serial numbers.

Also ensure your user can open serial devices:

```bash
sudo usermod -aG dialout $USER
```

Log out/in (or reboot) after changing group membership.

## 3.3 Launch the node

Use the provided launch file and pass the config:

```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
  config_file:=/absolute/path/to/your/config.yaml
```

### Launch file arguments

The launch file exposes these launch arguments:

- `config_file` (default: `config/rover_ntrip.yaml` inside the package)
- `namespace` (default: `oxide_gnss`)
- `log_level` (default: `info`, passed to `RUST_LOG`)
- `ntrip_username` (default: `$NTRIP_USERNAME`)
- `ntrip_password` (default: `$NTRIP_PASSWORD`)

If you set `ntrip_username` / `ntrip_password` on the launch command line, those values become environment variables for the process.

If you are using NTRIP, credentials are typically provided via environment variables (recommended):

```bash
export NTRIP_USERNAME="your_username"
export NTRIP_PASSWORD="your_password"

ros2 launch oxide_gnss oxide_gnss.launch.py \
  config_file:=/absolute/path/to/your/rover_ntrip.yaml
```

## 3.4 Verify it is working

In a new terminal:

```bash
ros2 topic echo /oxide_gnss/fix
```

Also check diagnostics:

```bash
ros2 topic echo /diagnostics
```

If you changed the namespace, replace `/oxide_gnss` with your namespace.

---

# 4. Operating the Node

## 4.1 Node name and namespace

The default launch file starts:

- package: `oxide_gnss`
- executable: `oxide_gnss_node`
- node name: `gnss_node`
- default namespace: `oxide_gnss`

So topics appear under `/oxide_gnss/...` unless you change `namespace:=...`.

Example:

```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
  namespace:=gnss \
  config_file:=/absolute/path/to/config.yaml
```

### Inspecting the running node

These commands are often useful when validating a deployment:

```bash
# See node(s)
ros2 node list

# See topics and types
ros2 topic list
ros2 topic info /oxide_gnss/fix

# See node interfaces
ros2 node info /oxide_gnss/gnss_node
```

## 4.2 Required ROS parameter

Oxide requires exactly one ROS parameter:

- `config_file` (string)

It must point to a YAML file.

## 4.3 Running without the launch file (ros2 run)

You can also run the node directly:

```bash
ros2 run oxide_gnss oxide_gnss_node --ros-args -p config_file:=/absolute/path/to/config.yaml
```

In this case the node name is `oxide_gnss` (unless you override it with `__node:=...`).

## 4.4 Logging

Logging is controlled via `RUST_LOG` (the launch file exposes this as `log_level`).

Examples:

```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
  log_level:=debug \
  config_file:=/absolute/path/to/config.yaml
```

You can also set more granular logging using `RUST_LOG`, for example:

```bash
export RUST_LOG="oxide_gnss=debug,info"
```

### Useful logging examples

Device comms only:

```bash
export RUST_LOG="oxide_gnss::device=debug,info"
```

NTRIP only:

```bash
export RUST_LOG="oxide_gnss::ntrip=debug,info"
```

---

# 5. Common Use Cases (Examples)

All examples below assume you launch using:

```bash
ros2 launch oxide_gnss oxide_gnss.launch.py config_file:=/absolute/path/to/config.yaml
```

## 5.1 Standalone GNSS (no RTK)

Use when you do not have corrections and you just want GNSS position.

Minimal config:

```yaml
mode: standalone

features:
  satellites: true

device:
  port: "/dev/ttyACM0"
```

Use the provided example config as a starting point:

- `config/standalone.yaml`

## 5.2 RTK Rover with NTRIP corrections (most common)

Use the provided example config:

- `config/rover_ntrip.yaml`

Typical highlights:

- `mode: rover_ntrip`
- `features.high_precision: true` (improves `~/fix` when HP messages are available)
- `features.integrity: true` (publishes `~/integrity` + `~/operational`)
- `ntrip.*` configured

## 5.3 RTK Rover with radio/serial corrections (UART2)

Use when RTCM corrections arrive over a radio or other serial link connected to the receiver’s UART2.

Use the provided example config:

- `config/rover_radio.yaml`

Make sure the rover’s u-blox UART2 baudrate matches your radio:

```yaml
device:
  ublox:
    ports:
      uart2:
        baudrate: 115200
```

## 5.4 Moving base + rover pair

This uses two receivers:

- **Moving base** publishes RTCM out of UART2
- **Moving base rover** receives RTCM on UART2 and publishes `~/baseline_pose`

Example launches (two terminals):

```bash
# Terminal 1
ros2 launch oxide_gnss oxide_gnss.launch.py \
  namespace:=gnss_base \
  config_file:=/absolute/path/to/moving_base.yaml

# Terminal 2
ros2 launch oxide_gnss oxide_gnss.launch.py \
  namespace:=gnss_rover \
  config_file:=/absolute/path/to/moving_base_rover.yaml
```

Use the provided example configs:

- `config/moving_base.yaml`
- `config/moving_base_rover.yaml`

### Interpreting `~/baseline_pose`

When enabled, Oxide publishes `~/baseline_pose` as a `PoseWithCovarianceStamped` describing the rover’s baseline to the base.

- Position is in **ENU meters**.
- `header.frame_id` is set to `gnss_base`.

## 5.5 Static base station (survey-in or fixed)

Use the provided example config:

- `config/static_base.yaml`

This config enables base station timing mode via u-blox configuration (`device.ublox.base_position`).

If you want a base station *without* configuring survey-in/fixed position from Oxide, you can omit `device.ublox.base_position` and configure the receiver separately (e.g. u-center). Oxide will still be able to output RTCM messages if the mode config enables them.

**Monitoring survey-in progress:** while survey-in is running, Oxide publishes a `{node}: SurveyIn` substatus on `/diagnostics` with `active`, `valid`, `mean_acc_mm`, `duration_s` and `observations` fields. Watch it with `ros2 topic echo /diagnostics`; the substatus transitions from `WARN` (active) to `OK` (valid) when convergence is reached, and is omitted for modes that don't run survey-in. See [TOPICS.md](TOPICS.md) for the full field list.

---

# 6. Advanced Configuration

## 6.1 Environment variable substitution in YAML

Oxide replaces `${VARNAME}` patterns in the YAML with environment variables at startup.

Example:

```yaml
ntrip:
  username: "${NTRIP_USERNAME}"
  password: "${NTRIP_PASSWORD}"
```

If an environment variable is missing, the `${...}` text is kept as-is.

## 6.2 Mode + feature interaction

The following are important:

- `mode` controls ports/protocols/messages defaults
- `features` enable additional messages and topics
- `device.ublox.messages.*` can override/add message rates

As an example, enabling `features.integrity: true` automatically requires and enables:

- `SEC_SIG`, `MON_RF`, `MON_COMMS`, `NAV_SAT`, `NAV_PL`

It also enables the receiver’s protection level calculation (`CFG-NAVSPG-PL_ENA`) when integrity is enabled.

### Feature compatibility

Not all features are permitted in all modes.

Examples:

- `standalone` does not allow `integrity`.
- `moving_base_rover` is the primary mode that allows `heading`.

If you use an incompatible combination, Oxide will fail fast at startup with a clear config validation error.

## 6.3 High precision fix behavior

`~/fix` is always published. Oxide can optionally use high precision u-blox position data when available.

Enable with:

```yaml
features:
  high_precision: true
```

This causes `~/fix` to use NAV-HPPOSLLH lat/lon/height and accuracy when available.

## 6.4 Velocity frame selection

Velocity is always published on `~/velocity`. You can choose the output frame:

```yaml
device:
  frame: ENU
```

Valid values:

- `ENU` (default; ROS convention)
- `NED`

## 6.5 u-blox device configuration (structured)

Advanced u-blox configuration lives under:

- `device.ublox.*`

This is sent via UBX `CFG-VALSET` at startup.

See [CONFIGURATION.md](CONFIGURATION.md) for advanced options.

Common advanced knobs:

- `device.ublox.nav_spg.*`
- `device.ublox.timepulse.*`
- `device.ublox.base_position.*` (base station)
- `device.ublox.signals.*` (constellations/signals)
- `device.ublox.messages.*` (output rates)

## 6.6 Overriding UBX message rates

You can override or add message output rates under `device.ublox.messages`.

Example: reduce satellite message bandwidth:

```yaml
device:
  ublox:
    messages:
      usb:
        NAV_SAT: 5
```

### Supported message keys

Oxide only recognizes a defined set of message keys. Unknown message names are ignored with a warning.

### Message rate meaning

- `0` disables the message
- `1` publishes every navigation solution
- `N` publishes every Nth solution

Currently recognized keys include (non-exhaustive but practical set):

- `NAV_PVT`
- `NAV_HPPOSLLH`
- `NAV_HPPOSECEF`
- `NAV_SAT`
- `NAV_SIG`
- `NAV_STATUS`
- `NAV_DOP`
- `NAV_CLOCK`
- `NAV_EOE`
- `NAV_POSLLH`
- `NAV_POSECEF`
- `NAV_ODO`
- `NAV_COV`
- `NAV_RELPOSNED`
- `NAV_PL`
- `NAV_SVIN` (UART2 only)
- `MON_RF`
- `MON_COMMS`
- `MON_HW`
- `SEC_SIG`
- `SEC_SIGLOG`
- `RXM_COR`

RTCM output keys on UART2 for base modes:

- `RTCM_3X_TYPE4072_0`
- `RTCM_3X_TYPE1005`
- `RTCM_3X_TYPE1074`
- `RTCM_3X_TYPE1084`
- `RTCM_3X_TYPE1094`
- `RTCM_3X_TYPE1124`
- `RTCM_3X_TYPE1230`

---

# 7. Integrity Monitoring

If you enable integrity, Oxide publishes:

- `~/integrity` (detailed integrity metrics with individual check results)
- `~/operational` (simple boolean go/no-go)

Enable it with:

```yaml
features:
  integrity: true
```

Integrity thresholds are configured in:

```yaml
integrity:
  thresholds:
    ...
```

## 7.0 Diagnostic Visibility

The `~/integrity` message includes **15 `check_*` boolean fields** showing pass/fail status for each individual check. This enables visualization tools like Foxglove to display a grid of pass/fail indicators, making it easy to identify which specific checks cause state changes.

Example check fields:
- `check_fix_type_ok` — Fix type is acceptable
- `check_satellites_ok` — Satellite count meets threshold
- `check_jamming_ok` — No critical jamming detected
- `check_pl_horizontal_ok` — Horizontal protection level within alert limit

For the full list of check fields and their meanings, see `docs/INTEGRITY.md`.

## 7.1 The `~/operational` output

`~/operational` is intended for systems that want a single go/no-go signal.

- `true` typically means integrity is OK or DEGRADED (configurable)
- `false` means CRITICAL or FAILED

Configure what is considered operational with:

- `integrity.thresholds.operational_threshold`

## 7.2 Required messages

When you are using mode-based configuration and enable `features.integrity: true`, Oxide ensures integrity-related UBX messages are enabled.

In legacy configuration, you must enable the required messages yourself via `device.ublox.messages.*`.

If you enable integrity on a receiver/firmware that does not support some integrity messages (notably `NAV_PL` on older firmware), Oxide will still run but may report reduced observability. Check startup logs for warnings.

---

# 8. Troubleshooting

## 8.1 "Permission denied" opening `/dev/ttyACM0` or `/dev/gnss_f9p_*`

- Ensure you are in the `dialout` group.
- Re-log after changing group membership.

## 8.2 Config file not found

- Pass an **absolute path** to `config_file`.

## 8.3 Topics appear under a different namespace than expected

If you launched with `namespace:=...`, your topics will be under that namespace.

Example: `namespace:=gnss` means `~/fix` becomes `/gnss/fix`.

## 8.4 NTRIP connects but RTK never fixes

Within Oxide’s scope, verify:

- `mode: rover_ntrip` is set
- `ntrip.host`, `ntrip.port`, `ntrip.mountpoint` are correct
- `ntrip.send_gga: true` if your network requires GGA

Also watch `/diagnostics` and `~/integrity` (if enabled) for correction status and age.

## 8.5 Integrity topic exists but seems empty or stuck

Integrity can report `FAILED` when GNSS data is stale. Check:

- Device connectivity
- UBX message configuration warnings at startup
- `/diagnostics`

## 8.6 Device config is rejected (NAK during configuration)

If the receiver NAKs a UBX `CFG-VALSET` command, Oxide treats it as a hard failure (no retry). Common causes:

- Unsupported configuration key for your device/firmware
- A configuration option incompatible with the selected mode

Try reducing your `device.ublox.*` overrides to isolate which setting is rejected.

## 8.7 Recovering a factory-reset F9P (blank USB serial → no `/dev/gnss_f9p_<serial>`)

A factory-reset (or freshly-flashed) ZED-F9P ships with a **blank USB serial string**. The kernel still enumerates the device as `/dev/ttyACM*`, but udev has nothing to key its `ID_SERIAL_SHORT`-based rules on, so the `/dev/gnss_f9p_<serial>` symlinks never appear.

Restore the serial with the bundled `oxide_gnss_assign_serial` CLI. It probes the current value via `CFG-VALGET`, refuses to overwrite a non-blank serial or collide with another device on the host (unless `--force` is set), and writes the new string to RAM + BBR + FLASH so it survives power cycles.

```bash
# Dry-run: print what would be written, don't touch the device.
oxide_gnss_assign_serial --port /dev/ttyACM0 --serial F9P-ROVER-01 --dry-run

# For real:
oxide_gnss_assign_serial --port /dev/ttyACM0 --serial F9P-ROVER-01
```

After the tool exits successfully, unplug and replug the F9P so USB re-enumerates; the `/dev/gnss_f9p_F9P-ROVER-01` symlink should then appear (assuming the udev rules from §3.2 are installed).

Options:

| Option | Purpose |
|--------|---------|
| `--port` | Serial port (e.g. `/dev/ttyACM0`). Required. |
| `--serial` | ASCII serial string, 1..=32 bytes. Required. |
| `--baud` | Baud rate (default `460800`). |
| `--force` | Overwrite a non-blank serial, or accept a collision with another device on the host. |
| `--dry-run` | Probe and report only; no writes. |

---


# 9. Configuration Reference

For an exhaustive reference of every supported YAML key, see [CONFIGURATION.md](CONFIGURATION.md). For integrity thresholds specifically see [INTEGRITY.md](INTEGRITY.md), and for ROS topics see [TOPICS.md](TOPICS.md). The sections above cover the common workflows; [CONFIGURATION.md](CONFIGURATION.md) is the place to look when hunting a specific parameter name or default.
