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

The array always includes device/NTRIP/fix substatuses. In addition, when the receiver has emitted at least one `NAV-SVIN` packet (i.e. `static_base` mode running survey-in) the array includes a `{node}: SurveyIn` substatus so operators can monitor survey-in progress without a separate topic:

| Level | Condition |
|-------|-----------|
| `OK` | `valid = true` — survey-in complete |
| `WARN` | `active = true, valid = false` — survey-in in progress |
| `STALE` | `active = false, valid = false` — idle (last survey-in seen was not running) |

Key/value fields on the SurveyIn substatus:

| Key | Description |
|-----|-------------|
| `active` | Whether the receiver is currently running survey-in |
| `valid` | Whether the survey-in solution has met the configured convergence criteria |
| `mean_acc_mm` | Mean 3-D accuracy of the accumulated position (mm, 1 decimal) |
| `duration_s` | Elapsed survey-in time in seconds |
| `observations` | Number of position observations accumulated |

The SurveyIn substatus is omitted entirely in modes that do not run survey-in, so it never appears as a filler entry for rover/moving-base configs.

---

## Optional Topics

### `~/integrity`

**Type:** `oxide_gnss_msgs/OxideIntegrity`

Integrity monitoring data including fix quality, jamming/spoofing status, signal quality, and **individual check results** for diagnostic visibility.

- **Requires:** `features.integrity: true`
- **UBX Messages:** `NAV_PVT`, `SEC_SIG`, `MON_RF`, `MON_COMMS`, `NAV_SAT`
- **Rate:** Configurable via `ros.rates.integrity_hz` (default: 1.0 Hz)

**Key Fields:**
- `level` — Integrity level (0=OK, 1=DEGRADED, 2=CRITICAL, 3=FAILED)
- `operational` — Boolean go/no-go signal
- `check_*` — 15 boolean fields showing pass/fail for each individual check (enables Foxglove pass/fail grids)
- Position quality, signal quality, security status, protection levels

See [INTEGRITY.md](INTEGRITY.md) for detailed field documentation.

### `~/operational`

**Type:** `std_msgs/Bool`

Simple go/no-go signal derived from integrity level.

- **Requires:** `features.integrity: true`
- **Rate:** Same as `~/integrity`

### `~/satellites`

**Type:** `oxide_gnss_msgs/OxideSatellites`

Per-satellite signal information including constellation, signal strength, elevation, azimuth, and usage status.

- **Requires:** `features.satellites: true`
- **UBX Messages:** `NAV_SAT`
- **Rate:** Navigation rate (can be high bandwidth)

**Message Fields:**
- `num_satellites` — Total satellites tracked
- `num_used` — Satellites used in navigation solution
- `satellites[]` — Per-satellite details (`OxideSatellite`)
- `mean_cno`, `min_cno`, `sats_above_threshold` — Signal quality metrics
- `num_gps`, `num_glonass`, `num_galileo`, `num_beidou`, `num_sbas`, `num_qzss` — Per-constellation counts

**OxideSatellite Fields:**
- `gnss_id` — Constellation (GPS=0, SBAS=1, Galileo=2, BeiDou=3, QZSS=5, GLONASS=6, NAVIC=7)
- `sv_id` — Satellite vehicle ID
- `cno` — Carrier-to-noise ratio (dB-Hz)
- `elevation`, `azimuth` — Sky position (degrees)
- `used_in_solution` — Whether satellite contributes to fix
- `signal_quality`, `health`, `orbit_source` — Quality indicators

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
