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
- **`dead_reckoning`**
  - Present as a config flag; currently not implemented as an output feature.

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

- `config_file` (default: `config/default.yaml` inside the package)
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

Use the example:

- `config/advanced_rover.yaml`

Common advanced knobs:

- `device.ublox.rate.*`
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

- `~/integrity` (detailed integrity metrics)
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

For the conceptual model, see `docs/INTEGRITY.md`.

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

---

# 9. Definitive Configuration Reference

This section documents every configuration parameter supported by Oxide.

The root YAML structure is:

```yaml
mode: <optional>
features: <optional>
ros: <optional>
device: <required>
ntrip: <optional>
integrity: <optional>
```

## 9.1 Root parameters

### `mode`

- **Type:** string (snake_case)
- **Required:** no
- **Default:** omitted (legacy mode)
- **Values:**
  - `standalone`
  - `rover_ntrip`
  - `rover_radio`
  - `moving_base`
  - `moving_base_rover`
  - `static_base`

**Notes:**

- If `mode` is set, Oxide generates an internal u-blox configuration from `mode` + `features`.
- If `mode` is omitted, Oxide runs in "legacy" mode and expects explicit `device.ublox.*` config for non-trivial setups.

### `features`

- **Type:** object
- **Required:** no
- **Default:** all false

Supported feature flags:

- `high_precision` (bool, default `false`)
- `integrity` (bool, default `false`)
- `satellites` (bool, default `false`)
- `heading` (bool, default `false`)
- `dead_reckoning` (bool, default `false`)

**Notes:**

- Some features are only allowed with some modes; incompatible combinations cause a startup validation error.

### `ros`

- **Type:** object
- **Required:** no
- **Default:** `{}`

Sub-parameters:

- `ros.rates.diagnostics_hz` (f64, default `1.0`)
- `ros.rates.integrity_hz` (f64, default `1.0`)

### `device`

- **Type:** object
- **Required:** yes

See [9.2 Device parameters](#92-device-parameters).

### `ntrip`

- **Type:** object
- **Required:** no
- **Default:** omitted (NTRIP disabled)

See [9.3 NTRIP parameters](#93-ntrip-parameters).

### `integrity`

- **Type:** object
- **Required:** no
- **Default:** `{}`

See [9.4 Integrity parameters](#94-integrity-parameters).

---

## 9.2 Device parameters

### `device.port`

- **Type:** string
- **Required:** yes
- **Default:** none

Serial device path (Linux examples: `/dev/ttyACM0`, `/dev/gnss_f9p_<serial>`).

### `device.baud_rate`

- **Type:** u32
- **Required:** no
- **Default:** `460800`

### `device.frame`

- **Type:** enum string
- **Required:** no
- **Default:** `ENU`
- **Values:** `ENU`, `NED`

Controls the velocity output frame in `~/velocity`.

### `device.navigation`

- **Type:** object
- **Required:** no
- **Default:**
  - `rate_hz: 10`
  - `min_satellites: 4`
  - `max_hdop: 5.0`
  - `max_pdop: 10.0`

Fields:

- `device.navigation.rate_hz` (u8, default `10`)
  - Valid range enforced: `1..=25`
- `device.navigation.min_satellites` (u8, default `4`)
- `device.navigation.max_hdop` (f32, default `5.0`)
- `device.navigation.max_pdop` (f32, default `10.0`)

Value notes:

- `device.navigation.rate_hz` is validated as `1..=25`.
- `max_hdop` and `max_pdop` must be `> 0`.

**Notes:**

- These values are validated (e.g., `rate_hz` must be 1–25) and describe user intent.
- The receiver’s actual measurement rate is configured via `device.ublox.rate.*` (if present) or by the mode defaults.

### `device.reconnect`

- **Type:** object
- **Required:** no
- **Default:**
  - `enabled: true`
  - `initial_delay_secs: 1`
  - `max_delay_secs: 30`
  - `max_attempts: 0`
  - `backoff_reset_secs: 300`

Fields:

- `device.reconnect.enabled` (bool)
- `device.reconnect.initial_delay_secs` (u32)
- `device.reconnect.max_delay_secs` (u32)
- `device.reconnect.max_attempts` (u32; `0` means unlimited)
- `device.reconnect.backoff_reset_secs` (u32; `0` means never reset)

### `device.ublox`

- **Type:** object
- **Required:** no
- **Default:** omitted

Subsections:

- `family`
- `rate`
- `protocols`
- `ports`
- `signals`
- `nav_spg`
- `timepulse`
- `base_position`
- `time_mark`
- `messages`

#### `device.ublox.family`

- **Type:** string
- **Required:** no
- **Default:** omitted

Used primarily for validation warnings/documentation; typical values: `F9P`, `F9R`, `F9H`.

#### `device.ublox.rate`

- **Type:** object
- **Required:** no
- **Default:**
  - `measurement_ms: 100`
  - `nav_ratio: 1`

Fields:

- `device.ublox.rate.measurement_ms` (u16)
- `device.ublox.rate.nav_ratio` (u16)

#### `device.ublox.protocols`

- **Type:** object
- **Required:** no
- **Default:** all fields omitted

Each port has a `PortProtocols` object with optional booleans:

- Input: `in_ubx`, `in_nmea`, `in_rtcm3x`, `in_spartn`
- Output: `out_ubx`, `out_nmea`, `out_rtcm3x`

Ports:

- `device.ublox.protocols.usb.*`
- `device.ublox.protocols.uart1.*`
- `device.ublox.protocols.uart2.*`
- `device.ublox.protocols.i2c.*`
- `device.ublox.protocols.spi.*`

Each field is an optional bool:

- omitted: leave device default
- `true`: enable
- `false`: disable

#### `device.ublox.ports`

- **Type:** object
- **Required:** no
- **Default:** all fields omitted

Fields:

- `device.ublox.ports.uart1.enabled` (bool, optional)
- `device.ublox.ports.uart1.baudrate` (u32, optional)
- `device.ublox.ports.uart2.enabled` (bool, optional)
- `device.ublox.ports.uart2.baudrate` (u32, optional)
- `device.ublox.ports.i2c_enabled` (bool, optional)
- `device.ublox.ports.spi_enabled` (bool, optional)

#### `device.ublox.signals`

- **Type:** object
- **Required:** no
- **Default:** all fields omitted

Constellation sections:

- `gps: { enabled?, l1?, l2? }`
- `glonass: { enabled?, l1?, l2? }`
- `galileo: { enabled?, l1?, l2? }`
- `beidou: { enabled?, b1?, b2? }`
- `sbas: { enabled?, l1? }`
- `qzss: { enabled?, l1ca?, l1s?, l2c? }`

All fields are optional booleans; omitted values leave device defaults.

#### `device.ublox.nav_spg`

- **Type:** object
- **Required:** no
- **Default:** `{}`

Fields (all optional):

- `device.ublox.nav_spg.pl_ena` (bool)
- `device.ublox.nav_spg.dynamic_model` (enum string)
- `device.ublox.nav_spg.elevation_mask` (i8)
- `device.ublox.nav_spg.pdop_mask` (f32)

Value notes:

- `elevation_mask` is in degrees (typical range 0–90).
- `pdop_mask` is a PDOP threshold; internally it is sent to the receiver as PDOP*10 (e.g. 6.0 -> 60).

`dynamic_model` supported values (snake_case):

- `portable` (default)
- `stationary`
- `pedestrian`
- `automotive`
- `sea`
- `airborne_light`
- `airborne_medium`
- `airborne_high`
- `wrist`
- `bike`
- `mower`
- `escooter`
- `robot`

#### `device.ublox.timepulse`

- **Type:** object
- **Required:** no
- **Default:** omitted

Fields (all optional):

- `enabled` (bool)
- `frequency_hz` (u32)
- `frequency_unlocked_hz` (u32)
- `pulse_length_us` (u32)
- `pulse_length_unlocked_us` (u32)
- `polarity` (enum: `rising` | `falling`)
- `time_grid` (enum: `utc` | `gps` | `glonass` | `beidou` | `galileo`)
- `align_to_tow` (bool)
- `use_locked_params` (bool)
- `sync_to_gnss` (bool)
- `cable_delay_ns` (i16)

Operational notes:

- The timepulse signal is on the receiver’s TIMEPULSE pin. Refer to your carrier board documentation for the physical pin/connector.
- Oxide programs timepulse using UBX configuration at startup. If the receiver NAKs timepulse-related keys, remove the timepulse section and validate base functionality first.

#### `device.ublox.base_position`

- **Type:** object
- **Required:** no
- **Default:** omitted

Fields:

- `mode` (enum: `disabled` | `survey_in` | `fixed`)

If `mode: survey_in`:

- `survey_in.min_duration_s` (u32, optional)
- `survey_in.accuracy_limit_m` (f32, optional)

If `mode: fixed`:

- `fixed.latitude` (f64, optional)
- `fixed.longitude` (f64, optional)
- `fixed.height_m` (f64, optional)
- `fixed.accuracy_m` (f32, optional)

Operational notes:

- For `fixed` mode, you should provide at least `latitude`, `longitude`, and `height_m`.
- For `survey_in` mode, you should provide `survey_in.min_duration_s` and `survey_in.accuracy_limit_m`.

#### `device.ublox.time_mark`

- **Type:** object
- **Required:** no
- **Default:** omitted

Fields:

- `enabled` (bool, optional)

When enabled, Oxide configures TIM-TM2 message output on USB.

Operational notes:

- TIM-TM2 timestamps external events on the receiver’s EXTINT pin.
- Oxide enables the receiver-side message output; it does not currently publish a dedicated ROS topic for TIM-TM2.

#### `device.ublox.messages`

- **Type:** object
- **Required:** no
- **Default:** `{}`

Port maps:

- `device.ublox.messages.usb` (map string -> u8)
- `device.ublox.messages.uart1` (map string -> u8)
- `device.ublox.messages.uart2` (map string -> u8)

Rate meaning:

- `0` disables a message
- `1` means every solution
- `N` means every Nth solution

**Notes:**

- In mode-based config, these entries are merged on top of the generated defaults.
- UART2 can also be used for RTCM output message selection in base modes.

---

## 9.3 NTRIP parameters

The entire `ntrip:` block is optional. If omitted, NTRIP is disabled.

### `ntrip.host`

- **Type:** string
- **Required:** yes (if `ntrip` block exists)

### `ntrip.port`

- **Type:** u16
- **Required:** no
- **Default:** `2101`

### `ntrip.use_https`

- **Type:** bool
- **Required:** no
- **Default:** `false`

 Notes:

 - Set to `true` to use HTTPS/TLS (commonly on port `443`).
 - If `use_https: false`, TLS options have no effect.

### `ntrip.tls_skip_verify`

- **Type:** bool
- **Required:** no
- **Default:** `false`

 Notes:

 - Testing only: skips TLS certificate verification (use only with self-signed certs in controlled environments).
 - Only relevant when `ntrip.use_https: true`.

### `ntrip.ntrip_version`

- **Type:** string
- **Required:** no
- **Default:** `auto`
- **Values:** `1`, `2`, `auto`

### `ntrip.mountpoint`

- **Type:** string
- **Required:** yes (if `ntrip` block exists)

### `ntrip.username`

- **Type:** string
- **Required:** no
- **Default:** omitted

### `ntrip.password`

- **Type:** string
- **Required:** no
- **Default:** omitted

 Notes:

 - Authentication is used only when both `ntrip.username` and `ntrip.password` are provided.
 - Prefer using environment variable substitution: `${NTRIP_USERNAME}` and `${NTRIP_PASSWORD}`.

### `ntrip.send_gga`

- **Type:** bool
- **Required:** no
- **Default:** `true`

### `ntrip.gga_interval_secs`

- **Type:** u32
- **Required:** no
- **Default:** `10`

### `ntrip.connection`

- **Type:** object
- **Required:** no
- **Default:**
  - `timeout_secs: 10`
  - `read_timeout_secs: 30`
  - `reconnect: true`
  - `initial_delay_secs: 1`
  - `max_delay_secs: 60`
  - `backoff_reset_secs: 3600`

Fields:

- `ntrip.connection.timeout_secs` (u32)
- `ntrip.connection.read_timeout_secs` (u32)
- `ntrip.connection.reconnect` (bool)
- `ntrip.connection.initial_delay_secs` (u32)
- `ntrip.connection.max_delay_secs` (u32)
- `ntrip.connection.backoff_reset_secs` (u32)

Operational notes:

- Oxide manages reconnection behavior itself; it does not rely on the underlying NTRIP client’s built-in reconnect.
- If `ntrip.connection.reconnect: false`, Oxide will not attempt to re-establish the stream after a connection loss.

---

## 9.4 Integrity parameters

### `integrity.thresholds`

- **Type:** object
- **Required:** no
- **Default:** (shown below)

Fields:

- `min_satellites_critical` (u8, default `4`)
- `min_satellites_high` (u8, default `6`)
- `max_h_accuracy_m` (f32, default `0.10`)
- `max_v_accuracy_m` (f32, default `0.15`)
- `max_pdop` (f32, default `3.0`)
- `max_correction_age_s` (f32, default `10.0`)
- `min_cno_degraded` (u8, default `25`)
- `min_mean_cno_degraded` (f32, default `35.0`)
- `max_pvt_age_s` (f32, default `2.0`)
- `operational_threshold` (u8, default `1`)
- `max_horizontal_pl_m` (f32, default `0.50`)
- `max_vertical_pl_m` (f32, default `1.00`)
- `max_velocity_pl_ms` (f32, default `0.10`)
- `max_tmir_per_epoch` (f64, default `6.0`)
- `require_valid_pl` (bool, default `false`)
