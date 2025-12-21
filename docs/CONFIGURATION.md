# Configuration Reference

This document describes all configuration options for oxide_gnss.

## Configuration File

oxide_gnss uses YAML configuration files. Example configs are in `config/`:

| File | Description |
|------|-------------|
| `rover_ntrip.yaml` | RTK rover with NTRIP corrections (most common) |
| `rover_radio.yaml` | RTK rover with radio/serial corrections on UART2 |
| `standalone.yaml` | Basic GPS without RTK |
| `moving_base.yaml` | Moving base in MB+R pair |
| `moving_base_rover.yaml` | Rover in MB+R pair |
| `static_base.yaml` | Static base station providing RTCM corrections |
| `advanced_rover.yaml` | Advanced config with custom messages/signals |

```bash
ros2 launch oxide_gnss oxide_gnss.launch.py config_file:=/path/to/your/config.yaml
```

---

## Mode-Based Configuration

The recommended way to configure oxide_gnss is using **modes** and **features**. This provides sensible defaults without requiring detailed UBX message knowledge.

### Operating Modes

```yaml
mode: rover_ntrip
```

| Mode | Description | Protocols | Base Messages |
|------|-------------|-----------|---------------|
| `standalone` | Basic GPS without RTK | USB: UBX in/out | NAV_PVT |
| `rover_ntrip` | RTK rover with NTRIP | USB: UBX, RTCM3X in | NAV_PVT, NAV_HPPOSLLH |
| `rover_radio` | RTK rover with radio corrections | UART2: RTCM3X in | NAV_PVT, NAV_HPPOSLLH |
| `moving_base` | Moving base station | UART2: RTCM3X out | NAV_PVT, NAV_HPPOSLLH, NAV_COV, NAV_STATUS |
| `moving_base_rover` | Rover in MB+R pair | UART2: RTCM3X in | NAV_PVT, NAV_HPPOSLLH, NAV_RELPOSNED |
| `static_base` | Static base station | UART2: RTCM3X out | NAV_PVT, NAV_HPPOSLLH |

### Feature Flags

```yaml
features:
  high_precision: true    # Use HP position data in ~/fix
  integrity: true         # Enable ~/integrity, ~/operational topics
  satellites: false       # Enable ~/satellites topic
```

| Feature | Description | Messages Added | Topics Enabled |
|---------|-------------|----------------|----------------|
| `high_precision` | HP data enhances ~/fix accuracy | NAV_HPPOSLLH | (enhances ~/fix) |
| `integrity` | Jamming/spoofing detection | SEC_SIG, MON_RF, MON_COMMS | ~/integrity, ~/operational |
| `satellites` | Per-satellite visibility info | NAV_SAT | ~/satellites |

### How Mode + Features Work

1. **Mode** sets base protocols and messages for your operating scenario
2. **Features** add optional functionality on top
3. **device.ublox** overrides allow fine-tuning (see Advanced Configuration)

---

## ROS Configuration

```yaml
ros:
  rates:
    diagnostics_hz: 1.0    # /diagnostics publish rate
    integrity_hz: 1.0      # ~/integrity, ~/operational publish rate
```

Most topics publish when UBX data arrives (driven by device rate). These rates control timer-based aggregated topics.

---

## Device Configuration

### Basic Settings

```yaml
device:
  port: "/dev/gnss_f9p_rover"   # Serial port path
  baud_rate: 460800              # Baud rate (460800 recommended for high-rate)
  frame: ENU                     # Velocity frame: ENU or NED
```

| Parameter | Description | Default |
|-----------|-------------|---------|
| `port` | Serial port path. Use udev symlinks for stability. | Required |
| `baud_rate` | Serial baud rate. Must match device configuration. | `460800` |
| `frame` | Coordinate frame for velocity: `ENU` (ROS convention) or `NED` (aviation) | `ENU` |

### Navigation Settings

```yaml
device:
  navigation:
    rate_hz: 10           # Update rate (1-25 Hz for ZED-F9P)
    min_satellites: 4     # Minimum satellites for valid fix
    max_hdop: 5.0         # HDOP warning threshold
    max_pdop: 10.0        # PDOP warning threshold
```

### Reconnection Behavior

```yaml
device:
  reconnect:
    enabled: true
    initial_delay_secs: 1
    max_delay_secs: 30
    max_attempts: 0       # 0 = unlimited (recommended for safety-critical)
```

Reconnection uses exponential backoff. For safety-critical applications, use `max_attempts: 0` to continuously retry.

---

## u-blox Receiver Configuration

The `ublox` section configures the receiver directly via UBX-CFG-VALSET commands.

### Device Family

```yaml
device:
  ublox:
    family: "F9P"    # F9P, F9R, or F9H
```

Used for validation warnings (e.g., NAV_COV is only available on F9R/F9H).

### Measurement Rate

```yaml
device:
  ublox:
    rate:
      measurement_ms: 100   # Measurement period (100ms = 10Hz)
      nav_ratio: 1          # Nav solutions per measurement
```

### Port and Protocol Settings

**When using mode-based configuration**, ports and protocols are configured automatically based on your selected mode. The mode presets:

- Enable only the ports needed for that mode (e.g., UART2 for moving base)
- Disable unused ports (UART1, SPI) to reduce CPU load
- Enable only required protocols (UBX, RTCM3X where needed)
- Disable unused protocols (NMEA) to reduce processing overhead

**You typically don't need to configure ports/protocols manually.** The mode handles it.

### Port Settings (Optional Overrides)

If you need to override the mode defaults (e.g., set UART2 baudrate):

```yaml
device:
  ublox:
    ports:
      uart2:
        baudrate: 460800      # Set UART2 baud rate for RTCM
```

### Advanced: Explicit Protocol Control

For advanced users who need fine-grained control, you can override protocol settings:

```yaml
device:
  ublox:
    protocols:
      usb:
        in_ubx: true
        in_nmea: false
        in_rtcm3x: true
        out_ubx: true
        out_nmea: false
```

Available protocols per port:
- **Input**: `in_ubx`, `in_nmea`, `in_rtcm3x`, `in_spartn` (I2C/SPI only)
- **Output**: `out_ubx`, `out_nmea`, `out_rtcm3x`

**Note:** Explicit protocol settings override the mode defaults. Only use this if you have a specific need.

### GNSS Signal Selection (Optional)

```yaml
device:
  ublox:
    signals:
      gps:
        enabled: true
        l1: true    # L1C/A
        l2: true    # L2C
      glonass:
        enabled: true
        l1: true
        l2: true
      galileo:
        enabled: false
      beidou:
        enabled: true
        b1: true
        b2: true
      sbas:
        enabled: false
      qzss:
        enabled: false
```

---

## UBX Message Configuration

### Message Output Rates

```yaml
device:
  ublox:
    messages:
      usb:
        NAV_PVT: 1          # Every solution
        NAV_HPPOSLLH: 1
        NAV_SAT: 5          # Every 5th solution (reduces bandwidth)
```

Rate values:
- `0` = disabled
- `1` = every solution
- `N` = every Nth solution

### Message Requirements

The driver validates message configuration at startup. Messages are categorized by importance:

| Level | Behavior | Description |
|-------|----------|-------------|
| **Essential** | ERROR + fail | Driver won't work without this |
| **Recommended** | WARN | Full functionality requires this |
| **RequiredForFeature** | INFO | Specific topic won't publish without this |
| **Optional** | (silent) | Nice to have |

### Message Reference

| Message | Level | Purpose | ROS Topics |
|---------|-------|---------|------------|
| `NAV_PVT` | Essential | Position/velocity/time | `~/fix`, `~/velocity`, `~/time_reference` |
| `NAV_HPPOSLLH` | Recommended | High-precision position | Enhances `~/fix` |
| `NAV_POSECEF` | Optional | ECEF coordinates | — |
| `MON_RF` | RequiredForFeature | Antenna status, jamming indicator | `~/integrity` |
| `MON_COMMS` | RequiredForFeature | Communication port health | `~/integrity` |
| `SEC_SIG` | RequiredForFeature | Jamming/spoofing detection | `~/integrity` |
| `NAV_RELPOSNED` | RequiredForFeature | Moving base relative position | `~/baseline_pose` |
| `NAV_SAT` | Optional | Per-satellite info | `~/satellites` |
| `NAV_COV` | Optional | Covariance matrix (F9R/F9H only) | — |
| `SEC_SIGLOG` | Optional | Security event log | — |

### Recommended Configuration

**Use mode-based config** instead of manually specifying messages:

```yaml
# Minimal (position only)
mode: standalone

# RTK rover with integrity
mode: rover_ntrip
features:
  high_precision: true
  integrity: true

# Moving base/rover heading
mode: moving_base_rover
features:
  high_precision: true
```

---

## Advanced Configuration

For users who need fine-grained control, you can override mode defaults.

### Adding Extra Messages

```yaml
mode: rover_ntrip
features:
  high_precision: true
  integrity: true

device:
  ublox:
    messages:
      usb:
        # Add messages not in the mode preset
        NAV_SAT: 5          # Every 5th solution
        # Change rates of existing messages
        NAV_HPPOSLLH: 2     # Every 2nd solution
```

### GNSS Constellation Configuration

```yaml
device:
  ublox:
    signals:
      gps:
        enabled: true
        l1: true      # L1C/A
        l2: true      # L2C (dual-frequency)
      glonass:
        enabled: true
        l1: true
        l2: true
      galileo:
        enabled: true
        l1: true      # E1
        l2: true      # E5b
      beidou:
        enabled: true
        b1: true
        b2: true
      sbas:
        enabled: false
      qzss:
        enabled: false
```

### Measurement Rate

```yaml
device:
  ublox:
    rate:
      measurement_ms: 100   # 10 Hz
      nav_ratio: 1          # Nav solution per measurement
```

### Legacy Configuration

If you omit `mode:`, the driver uses legacy mode with explicit message configuration:

```yaml
# Legacy mode - full manual control
device:
  port: "/dev/gnss_f9p"
  ublox:
    messages:
      usb:
        NAV_PVT: 1
        NAV_HPPOSLLH: 1
        SEC_SIG: 1
```

---

## NTRIP Configuration

NTRIP provides RTK corrections for centimeter-level accuracy.

```yaml
ntrip:
  host: "ntrip.data.gnss.ga.gov.au"
  port: 2101
  mountpoint: "SWTC00AUS0"
  
  # TLS/HTTPS settings
  use_https: false              # Enable HTTPS (TLS) for connection
  tls_skip_verify: false        # Skip certificate verification (testing only!)
  
  # Protocol version
  ntrip_version: auto           # "1", "2", or "auto" (default)
  
  # Authentication
  username: "${NTRIP_USERNAME}"    # Environment variable
  password: "${NTRIP_PASSWORD}"
  
  # GGA position reporting
  send_gga: true
  gga_interval_secs: 10
  
  # Connection settings
  connection:
    timeout_secs: 10
    reconnect: true
    initial_delay_secs: 1
    max_delay_secs: 60
    backoff_reset_secs: 3600
```

### TLS/HTTPS Options

| Parameter | Default | Description |
|-----------|---------|-------------|
| `use_https` | `false` | Enable TLS encryption for the NTRIP connection. Set to `true` for casters that require HTTPS. |
| `tls_skip_verify` | `false` | **⚠️ Testing only!** Skip TLS certificate verification. Use only for self-signed certificates in development. Never enable in production. |
| `ntrip_version` | `auto` | NTRIP protocol version: `1` (legacy ICY), `2` (HTTP/1.1 chunked), or `auto` (detect from server). |

### Environment Variables

Credentials can use environment variables with `${VAR_NAME}` syntax:

```bash
export NTRIP_USERNAME="your_username"
export NTRIP_PASSWORD="your_password"
```

### Disabling NTRIP

Comment out or remove the entire `ntrip:` section to disable NTRIP.

---

## ROS2 Topics

### Published Topics

Topics are created based on your mode and feature configuration. Only enabled topics are advertised.

**Core Topics (always created):**

| Topic | Type | Source | Description |
|-------|------|--------|-------------|
| `~/fix` | `sensor_msgs/NavSatFix` | NAV_PVT (+HP if enabled) | Position with covariance |
| `~/velocity` | `geometry_msgs/TwistWithCovarianceStamped` | NAV_PVT | 3D velocity |
| `~/time_reference` | `sensor_msgs/TimeReference` | NAV_PVT | GPS time |
| `/diagnostics` | `diagnostic_msgs/DiagnosticArray` | Multiple | System diagnostics |

**Optional Topics (based on mode/features):**

| Topic | Type | Enabled By | Description |
|-------|------|------------|-------------|
| `~/integrity` | `oxide_gnss_msgs/OxideIntegrity` | `integrity: true` | Safety integrity status |
| `~/operational` | `std_msgs/Bool` | `integrity: true` | Go/no-go signal |
| `~/satellites` | `std_msgs/String` | `satellites: true` | Satellite info (JSON) |
| `~/baseline_pose` | `geometry_msgs/PoseWithCovarianceStamped` | `mode: moving_base_rover` | Baseline to moving base |

---

## Safety Integrity Monitoring

The driver provides safety-critical integrity monitoring by aggregating quality metrics from multiple UBX messages. This enables autonomous systems to make go/no-go decisions based on GNSS solution quality.

### Integrity Levels

| Level | Value | Meaning | Action |
|-------|-------|---------|--------|
| **OK** | 0 | All checks pass | Full operation permitted |
| **DEGRADED** | 1 | Some quality checks failed | Reduced speed/capability recommended |
| **CRITICAL** | 2 | Critical checks failed | Operation should stop |
| **FAILED** | 3 | System unavailable or data stale | No GNSS available |

The `~/operational` topic publishes `true` when level is OK or DEGRADED, `false` otherwise.

### Data Sources

Integrity is computed from these UBX messages:

| Message | Data Used | Checks |
|---------|-----------|--------|
| **NAV_PVT** | Fix type, satellites, accuracy, PDOP, carrier solution | Fix quality, satellite count, accuracy thresholds |
| **SEC_SIG** | Jamming state, spoofing state | Jamming/spoofing detection |
| **MON_RF** | Antenna status, jamming indicator | Antenna faults |
| **MON_COMMS** | Port errors | Communication health |
| **NAV_COV** | Covariance matrices | (Optional) Full covariance data |
| **SEC_SIGLOG** | Security event count | Security event alerting |
| **RXM_COR** | Correction status | RTCM/SPARTN reception |

### Integrity Checks

#### Critical Checks (Level → CRITICAL)

These indicate the GNSS solution cannot be trusted:

| Check | Condition | Message |
|-------|-----------|---------|
| No fix | `fix_type == NoFix` | "No GNSS fix" |
| Insufficient fix | `fix_type` is 2D, dead-reckoning, or time-only | "Insufficient fix type" |
| Too few satellites | `num_satellites < min_satellites_critical` | "Too few satellites" |
| Critical jamming | `jamming_state == Critical` | "Critical jamming detected" |
| Multiple spoofers | `spoofing_state == Multiple` | "Multiple spoofers detected" |
| Antenna short | `antenna_status == Short` | "Antenna short circuit" |
| Antenna open | `antenna_status == Open` | "Antenna open circuit" |

#### Quality Checks (Level → DEGRADED)

These indicate reduced solution quality:

| Check | Condition | Message |
|-------|-----------|---------|
| RTK not fixed | `carrier_solution < 2` when differential applied | "RTK not fixed" |
| Horizontal accuracy | `h_accuracy_m > max_h_accuracy_m` | "Horizontal accuracy exceeded" |
| Vertical accuracy | `v_accuracy_m > max_v_accuracy_m` | "Vertical accuracy exceeded" |
| High PDOP | `pdop > max_pdop` | "PDOP too high" |
| Low satellite count | `num_satellites < min_satellites_high` | "Low satellite count" |
| Correction age | `correction_age_s > max_correction_age_s` | "Correction age exceeded" |

#### Monitor Checks (No level change, logged only)

| Check | Condition | Message |
|-------|-----------|---------|
| Jamming warning | `jamming_state == Warning` | "Jamming warning" |
| Spoofing indicated | `spoofing_state == Indicated` | "Spoofing indicated" |
| Security events | `security_events > 0` | "Security events logged" |

### Configurable Thresholds

Thresholds are currently set to defaults in code. Future releases will expose these in config:

| Threshold | Default | Description |
|-----------|---------|-------------|
| `min_satellites_critical` | 4 | Below this → CRITICAL |
| `min_satellites_high` | 6 | Below this → DEGRADED |
| `max_h_accuracy_m` | 0.10 | Horizontal accuracy (10cm) |
| `max_v_accuracy_m` | 0.15 | Vertical accuracy (15cm) |
| `max_pdop` | 3.0 | Position DOP threshold |
| `max_correction_age_s` | 10.0 | RTK correction age |

### Recommended Message Configuration

For full integrity monitoring, enable these messages:

```yaml
messages:
  usb:
    NAV_PVT: 1        # Essential - position/velocity/time
    MON_RF: 1         # Antenna status, jamming indicator
    SEC_SIG: 1        # Jamming/spoofing detection
    MON_COMMS: 1      # Communication port health
```

Optional for enhanced monitoring:
```yaml
    NAV_COV: 1        # Full covariance (F9R/F9H only)
    SEC_SIGLOG: 1     # Security event log
```

### Output Topics

| Topic | Type | Content |
|-------|------|---------|
| `~/integrity` | `oxide_gnss_msgs/OxideIntegrity` | Full integrity state with all metrics |
| `~/operational` | `std_msgs/Bool` | Simple go/no-go signal |

### Example Integrity Message

```json
{
  "level": 0,
  "level_name": "OK",
  "status_message": "All integrity checks passed",
  "fix_type": 5,
  "carrier_solution": 2,
  "num_satellites": 14,
  "h_accuracy_m": 0.015,
  "v_accuracy_m": 0.025,
  "pdop": 1.2,
  "jamming_state": "Ok",
  "spoofing_state": "Ok",
  "antenna_status": "Ok",
  "correction_age_s": 1.5,
  "operational": true
}
```

### Integration with Autonomous Systems

```python
# Example: Subscribe to operational signal
def operational_callback(msg):
    if not msg.data:
        # GNSS not reliable - stop or switch to backup
        emergency_stop()

# Example: Use integrity level for speed limiting
def integrity_callback(msg):
    if msg.level == 0:  # OK
        max_speed = FULL_SPEED
    elif msg.level == 1:  # DEGRADED
        max_speed = REDUCED_SPEED
    else:  # CRITICAL or FAILED
        max_speed = 0
```

---

## Udev Rules

For consistent device naming, install the provided udev rules:

```bash
sudo cp udev/99-oxide-gnss.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

This creates symlinks like `/dev/gnss_f9p_SERIAL` for stable device identification.

Ensure your user is in the `dialout` group:
```bash
sudo usermod -aG dialout $USER
# Log out and back in
```
