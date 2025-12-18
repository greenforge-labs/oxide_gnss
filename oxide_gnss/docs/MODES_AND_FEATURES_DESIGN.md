# Modes and Features Configuration Design

This document proposes a redesign of the oxide_gnss configuration system to make it more user-friendly by abstracting away low-level u-blox UBX message configuration.

## Problem Statement

The current configuration requires users to:
1. Understand which UBX messages are needed for their use case
2. Know which ports (USB, UART1, UART2) to configure
3. Understand protocol routing (UBX, RTCM3X, NMEA)
4. Map UBX messages to ROS topics

This creates a high barrier to entry and increases the risk of misconfiguration.

## Proposed Solution

Replace low-level UBX configuration with:
1. **Operating Modes** - Mutually exclusive device roles
2. **Feature Flags** - Optional capabilities that can be enabled per mode
3. **Expert Overrides** - Escape hatch for advanced users

---

## Operating Modes

Modes are mutually exclusive and define the primary role of the GNSS device.

### Mode Definitions

| Mode | Description | Corrections Source | Key Outputs |
|------|-------------|-------------------|-------------|
| `standalone` | Basic GPS, no RTK corrections | None | `~/fix`, `~/velocity`, `~/time_reference` |
| `rover_ntrip` | RTK rover with NTRIP corrections via network | USB (NTRIP client) | `~/fix` (HP), `~/velocity`, `~/integrity` |
| `rover_radio` | RTK rover with corrections via radio/serial | UART2 input | `~/fix` (HP), `~/velocity` |
| `moving_base` | Moving base in MB+R pair | USB (NTRIP optional) | `~/fix` (HP), RTCM out on UART2 |
| `moving_base_rover` | Rover in MB+R pair | UART2 (from moving base) | `~/fix` (HP), `~/baseline_pose`, `~/velocity` |
| `static_base` | Static base station (survey-in or fixed coords) | None | RTCM out (USB or UART) |

### Mode: `standalone`

Basic GNSS operation without RTK corrections. Suitable for applications that don't require centimeter-level accuracy.

**Automatic configuration:**
- USB: UBX in/out only
- UART1/UART2: Disabled
- Messages: NAV_PVT

**Available features:** `satellites`

---

### Mode: `rover_ntrip`

RTK rover receiving corrections from an NTRIP caster over the network. This is the most common RTK configuration.

**Automatic configuration:**
- USB: UBX in/out, RTCM3X in (for corrections from NTRIP)
- UART1/UART2: Disabled
- Messages: NAV_PVT, NAV_HPPOSLLH

**Available features:** `high_precision`, `integrity`, `satellites`

**Requires:** `ntrip` section in config

---

### Mode: `rover_radio`

RTK rover receiving corrections from a radio link or direct serial connection (e.g., from a static base station).

**Automatic configuration:**
- USB: UBX in/out only
- UART2: RTCM3X in (corrections from radio/base)
- UART2 baudrate: Configurable (default 460800)
- Messages: NAV_PVT, NAV_HPPOSLLH

**Available features:** `high_precision`, `integrity`, `satellites`

---

### Mode: `moving_base`

Moving base station in a moving base + rover pair. Outputs RTCM corrections to the rover via UART2.

**Automatic configuration:**
- USB: UBX in/out, RTCM3X in (optional NTRIP for absolute position)
- UART2: RTCM3X out (to rover)
- RTCM messages on UART2:
  - RTCM_3X_TYPE4072_0 (u-blox proprietary)
  - RTCM_3X_TYPE1074 (GPS MSM4)
  - RTCM_3X_TYPE1084 (GLONASS MSM4)
  - RTCM_3X_TYPE1094 (Galileo MSM4)
  - RTCM_3X_TYPE1124 (BeiDou MSM4)
  - RTCM_3X_TYPE1230 (GLONASS code-phase biases)
- Messages: NAV_PVT, NAV_HPPOSLLH, NAV_COV

**Available features:** `high_precision`, `integrity`, `satellites`

**Note:** The moving base outputs its own position fix. The RTCM stream to the rover enables the rover to compute its position relative to the base.

---

### Mode: `moving_base_rover`

Rover in a moving base + rover pair. Receives RTCM corrections from the moving base via UART2.

**Automatic configuration:**
- USB: UBX in/out only
- UART2: RTCM3X in (from moving base)
- Messages: NAV_PVT, NAV_HPPOSLLH, NAV_COV, NAV_RELPOSNED

**Output topics:**
- `~/fix` - Absolute position (from RTK solution)
- `~/baseline_pose` - Position relative to moving base (from NAV_RELPOSNED)
- `~/velocity` - Velocity

**Available features:** `high_precision`, `integrity`, `satellites`, `heading`

**Use cases:**
- Dual-antenna heading determination
- Precise baseline monitoring between two points on a vehicle

---

### Mode: `static_base`

Static base station for providing RTK corrections. Can use survey-in or fixed coordinates.

**Automatic configuration:**
- USB or UART: RTCM3X out
- Survey-in or fixed position configuration
- RTCM messages: Full MSM7 set for maximum accuracy

**Available features:** `satellites`

**Note:** This mode is primarily for generating corrections, not for navigation.

---

## Feature Flags

Features are optional capabilities that can be enabled in addition to the base mode configuration.

| Feature | Description | Required UBX Messages | Output Topics |
|---------|-------------|----------------------|---------------|
| `high_precision` | Use HP position for `~/fix` | NAV_HPPOSLLH | `~/fix` (enhanced), `~/hp_pos` |
| `integrity` | Integrity monitoring | SEC_SIG, MON_RF, MON_COMMS | `~/integrity`, `~/operational`, `~/sec_sig_details` |
| `satellites` | Per-satellite signal info | NAV_SAT | `~/satellites` |
| `heading` | Heading from baseline (MB+R) | NAV_RELPOSNED | `~/baseline_pose` |
| `dead_reckoning` | Sensor fusion for F9R | ESF_* messages | `~/velocity` (enhanced) |

### Feature: `high_precision`

Enables high-precision position output. When enabled:
- NAV_HPPOSLLH message is requested from the receiver
- `~/fix` topic uses HP coordinates (~0.1mm precision vs ~1cm)
- `~/hp_pos` topic publishes separately

**Applicable modes:** All except `standalone`

---

### Feature: `integrity`

Enables integrity monitoring for safety-critical applications. When enabled:
- Jamming/spoofing detection (SEC_SIG)
- Antenna status monitoring (MON_RF)
- Communication port health (MON_COMMS)

**Output topics:**
- `~/integrity` - Detailed integrity status
- `~/operational` - Simple go/no-go boolean
- `~/sec_sig_details` - Per-frequency jamming status

**Applicable modes:** All RTK modes

---

### Feature: `satellites`

Enables per-satellite signal information. 

**Note:** This can generate significant message traffic at high nav rates. Consider using a reduced output rate (e.g., every 5th solution).

**Applicable modes:** All

---

### Feature: `heading`

Enables heading calculation from the baseline vector in moving base + rover configurations.

**Requires:** `moving_base_rover` mode

**Output:** Heading is included in `~/baseline_pose` orientation

---

## Proposed YAML Configuration

### Minimal Configuration (rover with NTRIP)

```yaml
mode: rover_ntrip

device:
  port: "/dev/gnss_f9p"

ntrip:
  host: "ntrip.example.com"
  port: 2101
  mountpoint: "MOUNT1"
  username: "${NTRIP_USER}"
  password: "${NTRIP_PASS}"
```

### Full Configuration Example

```yaml
# Operating mode - defines the primary role
mode: rover_ntrip

# Optional features to enable
features:
  high_precision: true    # Use HP position in ~/fix
  integrity: true         # Enable integrity monitoring
  satellites: false       # Disable to reduce bandwidth

# Device configuration
device:
  port: "/dev/gnss_f9p"
  baud_rate: 460800
  frame: ENU              # Coordinate frame for velocity
  
  # Navigation rate
  rate_hz: 10
  
  # Optional: GNSS constellation selection
  signals:
    gps: true
    glonass: true
    galileo: true
    beidou: true

# NTRIP configuration (required for rover_ntrip mode)
ntrip:
  host: "ntrip.example.com"
  port: 2101
  mountpoint: "MOUNT1"
  username: "${NTRIP_USER}"
  password: "${NTRIP_PASS}"
  send_gga: true
  gga_interval_secs: 10
```

### Moving Base + Rover Configuration

**Base device (moving_base.yaml):**
```yaml
mode: moving_base

features:
  high_precision: true
  integrity: true

device:
  port: "/dev/gnss_base"
  baud_rate: 460800
  
  # UART2 settings for RTCM output to rover
  uart2:
    baud_rate: 460800

# Optional NTRIP for absolute position
ntrip:
  host: "ntrip.example.com"
  # ...
```

**Rover device (moving_base_rover.yaml):**
```yaml
mode: moving_base_rover

features:
  high_precision: true
  heading: true           # Get heading from baseline

device:
  port: "/dev/gnss_rover"
  baud_rate: 460800
  
  # UART2 settings for RTCM input from base
  uart2:
    baud_rate: 460800
```

### Expert Override (Advanced Users)

For users who need specific UBX messages not covered by features:

```yaml
mode: rover_ntrip

features:
  high_precision: true

device:
  port: "/dev/gnss_f9p"

# Expert: Add specific UBX messages beyond mode defaults
# WARNING: Messages not supported by the driver will be ignored
ublox_overrides:
  messages:
    usb:
      NAV_TIMEUTC: 1      # Add UTC time message
      NAV_DOP: 5          # DOP values every 5th solution
```

**Behavior:**
- Override messages are ADDED to mode defaults
- Unknown messages generate a warning: "NAV_FOOBAR enabled but not parsed by this driver"
- Protocol/port settings from mode are not overridable (to prevent misconfiguration)

---

## Implementation Plan

### Phase 1: Define Mode Presets

Create Rust structures defining the UBX configuration for each mode:

```rust
pub struct ModePreset {
    pub name: &'static str,
    pub description: &'static str,
    pub protocols: ProtocolConfig,
    pub messages: Vec<(&'static str, u8)>,  // (message_name, rate)
    pub rtcm_messages: Vec<(&'static str, Port)>,
    pub allowed_features: &'static [&'static str],
}
```

### Phase 2: Feature to Message Mapping

```rust
const FEATURE_MESSAGES: &[(&str, &[&str])] = &[
    ("high_precision", &["NAV_HPPOSLLH"]),
    ("integrity", &["SEC_SIG", "MON_RF", "MON_COMMS", "SEC_SIGLOG"]),
    ("satellites", &["NAV_SAT"]),
    ("heading", &["NAV_RELPOSNED"]),
];
```

### Phase 3: Config Merging Logic

```
final_config = mode_defaults + feature_messages + ublox_overrides
```

### Phase 4: Validation

- Validate feature compatibility with mode
- Warn on unsupported override messages
- Log final effective configuration at startup

### Phase 5: Startup Logging

```
INFO Mode: rover_ntrip
INFO Features: [high_precision, integrity]
INFO UBX Messages: [NAV_PVT, NAV_HPPOSLLH, SEC_SIG, MON_RF, MON_COMMS]
INFO Topics: [~/fix, ~/velocity, ~/time_reference, ~/hp_pos, ~/integrity, ~/operational]
```

---

## Migration Path

### Backward Compatibility

The existing `ublox:` section will continue to work. When present without a `mode:` setting, the driver operates in "legacy mode" using the explicit UBX configuration.

### Deprecation Timeline

1. **v0.2**: Introduce modes/features alongside existing config
2. **v0.3**: Warn when using legacy `ublox.messages` without `mode`
3. **v1.0**: Legacy config deprecated (still functional with warning)

---

## Open Questions

1. **Static base survey-in parameters** - How to configure survey-in duration/accuracy or fixed coordinates?

2. **Multi-device coordination** - Should we provide a way to configure base+rover pairs together, or keep them as separate config files?

3. **Device family differences** - F9P vs F9R vs F9H have different capabilities. Should mode validation consider device family?

4. **Runtime mode switching** - Is there a use case for changing modes without restart?

---

## References

- [u-blox F9P Interface Description](https://content.u-blox.com/sites/default/files/documents/u-blox-F9-HPG-1.32_InterfaceDescription_UBX-22008968.pdf)
- [aussierobots/ublox_dgnss](https://github.com/aussierobots/ublox_dgnss) - Moving base/rover launch examples
- [oxide_gnss MESSAGE_REQUIREMENTS](../src/config/ublox.rs) - Current message dependency definitions
