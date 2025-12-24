# ROS2 Topics Reference

This document describes the ROS2 topics published by `oxide_gnss`.

**Implementation:** `ros/publishers.rs`

---

## Core Topics (Always Published)

### `~/fix`

**Type:** `sensor_msgs/NavSatFix`

Position fix in WGS84 coordinates.

- **Source:** `NAV_PVT` (required), `NAV_HPPOSLLH` (optional, for high-precision)
- **Rate:** Navigation rate (e.g., 5-10 Hz)

### `~/velocity`

**Type:** `geometry_msgs/TwistWithCovarianceStamped`

Velocity in the configured coordinate frame (ENU or NED).

- **Source:** `NAV_PVT`
- **Rate:** Navigation rate
- **Frame:** Configurable via `device.frame` (default: ENU)

### `~/time_reference`

**Type:** `sensor_msgs/TimeReference`

GPS time reference for time synchronization.

- **Source:** `NAV_PVT`
- **Rate:** Navigation rate

### `/diagnostics`

**Type:** `diagnostic_msgs/DiagnosticArray`

Diagnostic information about device and NTRIP status.

- **Rate:** Configurable via `ros.rates.diagnostics_hz` (default: 1.0 Hz)

---

## Optional Topics

### `~/integrity`

**Type:** `oxide_gnss_msgs/OxideIntegrity`

Integrity monitoring data including fix quality, jamming/spoofing status, and signal quality.

- **Requires:** `features.integrity: true`
- **UBX Messages:** `NAV_PVT`, `SEC_SIG`, `MON_RF`, `MON_COMMS`, `NAV_SAT`
- **Rate:** Configurable via `ros.rates.integrity_hz` (default: 1.0 Hz)

See [INTEGRITY.md](INTEGRITY.md) for detailed documentation.

### `~/operational`

**Type:** `std_msgs/Bool`

Simple go/no-go signal derived from integrity level.

- **Requires:** `features.integrity: true`
- **Rate:** Same as `~/integrity`

### `~/satellites`

**Type:** `std_msgs/String` (JSON)

Per-satellite signal information (SNR, elevation, azimuth).

- **Requires:** `features.satellites: true`
- **UBX Messages:** `NAV_SAT`
- **Rate:** Navigation rate (can be high bandwidth)

### `~/baseline_pose`

**Type:** `geometry_msgs/PoseWithCovarianceStamped`

Baseline vector from moving base to rover.

- **Requires:** `mode: moving_base_rover`
- **UBX Messages:** `NAV_RELPOSNED`
- **Rate:** Navigation rate

---

## UBX Message Requirements Summary

| Topic | Required Messages | Optional Messages |
|-------|-------------------|-------------------|
| `~/fix`, `~/velocity`, `~/time_reference` | `NAV_PVT` | `NAV_HPPOSLLH` |
| `~/integrity`, `~/operational` | `NAV_PVT`, `SEC_SIG`, `MON_RF`, `MON_COMMS` | `NAV_PL`, `NAV_SAT`, `SEC_SIGLOG`, `RXM_COR`, `NAV_COV` |
| `~/satellites` | `NAV_SAT` | - |
| `~/baseline_pose` | `NAV_RELPOSNED` | - |

---

## Configuration Example

```yaml
mode: rover_ntrip

features:
  high_precision: true   # Use NAV_HPPOSLLH for ~/fix
  integrity: true        # Enable ~/integrity, ~/operational
  satellites: false      # Enable ~/satellites (high bandwidth)

ros:
  rates:
    diagnostics_hz: 1.0
    integrity_hz: 1.0
```

---

## Notes

- The driver validates UBX message configuration at startup and warns if required messages are missing.
- High-precision mode (`features.high_precision: true`) uses `NAV_HPPOSLLH` for centimeter-level position in `~/fix`.
- The `~/satellites` topic can generate significant bandwidth with many visible satellites; enable only if needed.
