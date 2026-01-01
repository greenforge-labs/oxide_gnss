# Configuration Expansion Roadmap

This document analyzes the gap between the u-blox ZED-F9P's receiver functionality and what Oxide GNSS currently exposes through its simplified configuration scheme. It provides recommendations and a phased implementation roadmap for expanding configuration options.

**Document Version:** 1.0  
**Date:** 2026-01-01  
**Branch:** `feature/configuration-expansion-roadmap`

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current State Analysis](#current-state-analysis)
3. [F9P Capability Gap Analysis](#f9p-capability-gap-analysis)
4. [Recommendations](#recommendations)
5. [Implementation Roadmap](#implementation-roadmap)
6. [Design Considerations](#design-considerations)
7. [Appendix: CFG Key Reference](#appendix-cfg-key-reference)

---

## Executive Summary

Oxide GNSS provides a simplified, mode-based configuration system that abstracts away low-level u-blox UBX configuration. While this serves the common use case well (RTK rover with NTRIP), several F9P capabilities are not exposed that would benefit specific robotics applications.

**Key Findings:**
- The current configuration covers ~40% of F9P receiver functionality
- Several high-value features for robotics are missing (Dynamic Model, Timepulse, Time Mark)
- The simplified approach is correct—we should add features selectively, not expose everything

**Recommendation:** Implement a tiered expansion focusing on high-value robotics use cases while maintaining the clean, mode-based configuration philosophy.

---

## Current State Analysis

### What Oxide Currently Supports

| Category | Feature | Implementation |
|----------|---------|----------------|
| **Operating Modes** | Standalone, Rover (NTRIP/Radio), Moving Base, Static Base | ✅ Full |
| **RTK** | NTRIP client, UART2 corrections, RTCM output | ✅ Full |
| **Navigation Rate** | 1-25 Hz configurable | ✅ Full |
| **GNSS Signals** | GPS, GLONASS, Galileo, BeiDou, SBAS, QZSS | ✅ Full |
| **High Precision** | NAV-HPPOSLLH integration | ✅ Full |
| **Integrity** | Jamming/spoofing detection, protection levels | ✅ Full |
| **Satellites** | Per-satellite visibility (NAV-SAT) | ✅ Full |
| **Moving Base** | Heading from baseline (NAV-RELPOSNED) | ✅ Full |
| **Port Configuration** | USB, UART1, UART2, I2C, SPI protocols | ✅ Full |
| **Reconnection** | Exponential backoff, configurable retry | ✅ Full |

### Configuration Philosophy

Oxide uses a **mode + features** approach:

```yaml
mode: rover_ntrip
features:
  high_precision: true
  integrity: true
```

This deliberately hides complexity. Users don't need to know about `CFG-MSGOUT-UBX_NAV_HPPOSLLH_USB` or `CFG-USBINPROT-RTCM3X`.

---

## F9P Capability Gap Analysis

Based on the ZED-F9P Integration Manual (UBX-18010802), the following receiver functionality is **not currently exposed** in Oxide:

### Tier 1: High-Value Gaps (Recommended for Implementation)

| Feature | Manual Section | Use Case | Complexity |
|---------|----------------|----------|------------|
| **Dynamic Platform Model** | 3.1.8 | Motion dynamics optimization | Low |
| **Timepulse (PPS)** | 3.13.1 | Sensor synchronization | Medium |
| **Time Mark (EXTINT)** | 3.13.2 | External event timestamping | Medium |
| **Elevation Mask** | 3.1.8 | Multipath reduction | Low |

#### Dynamic Platform Model

The F9P's navigation filter can be optimized for different motion profiles:

| Model | Value | Description | Use Case |
|-------|-------|-------------|----------|
| `PORTABLE` | 0 | Low dynamics, no altitude constraints | General handheld |
| `STATIONARY` | 2 | No movement expected | Base stations |
| `PEDESTRIAN` | 3 | Walking speed (<30 km/h) | Wearables, carts |
| `AUTOMOTIVE` | 4 | Car-like dynamics (<100 km/h) | Ground vehicles |
| `SEA` | 5 | Sea-level, low vertical dynamics | Marine vessels |
| `AIRBORNE_1G` | 6 | <1g acceleration | Light aircraft, balloons |
| `AIRBORNE_2G` | 7 | <2g acceleration | General aviation |
| `AIRBORNE_4G` | 8 | <4g acceleration | High-performance aircraft |
| `WRIST` | 9 | Wrist-worn device | Smartwatches |
| `BIKE` | 10 | Bicycle dynamics | E-bikes, cycling |
| `MOWER` | 11 | Lawn mower (low speed, frequent turns) | Robotic mowers |
| `ESCOOTER` | 12 | E-scooter dynamics | Micro-mobility |
| `ROBOT` | 13 | Generic robot (medium dynamics) | **Most robotics** |

**Impact:** Incorrect dynamic model can cause position jumps, filter divergence, or excessive smoothing.

**CFG Key:** `CFG-NAVSPG-DYNMODEL`

#### Timepulse (PPS)

Critical for sensor fusion. The F9P can output a precise pulse (typically 1 PPS) synchronized to GPS time.

**Use Cases:**
- Camera trigger synchronization
- LiDAR time alignment
- IMU timestamp calibration
- Multi-receiver time sync

**CFG Keys:**
- `CFG-TP-PULSE_DEF` - Pulse definition (frequency/period)
- `CFG-TP-PULSE_LENGTH_DEF` - Pulse length definition
- `CFG-TP-FREQ_TP1` - Frequency in Hz
- `CFG-TP-TIMEGRID_TP1` - Time grid (UTC/GPS/GLONASS)
- `CFG-TP-POL_TP1` - Polarity (rising/falling edge)

#### Time Mark (EXTINT)

Timestamps external events (e.g., camera shutter closure) with nanosecond precision.

**Use Cases:**
- Photogrammetry / aerial survey
- Event logging with precise timestamps
- Trigger-based data acquisition

**UBX Message:** `TIM-TM2` (external time mark)

**CFG Keys:**
- `CFG-TP-EXTINT_TRIGGER` - Trigger edge selection
- Enable `TIM-TM2` message output

#### Elevation Mask

Reject satellites below a configurable elevation angle to reduce multipath.

**Use Cases:**
- Urban canyons (reject low satellites reflecting off buildings)
- Forested environments
- Precision applications

**CFG Key:** `CFG-NAVSPG-ELEV_MASK` (degrees, 0-90)

### Tier 2: Medium-Value Gaps

| Feature | Manual Section | Use Case | Complexity |
|---------|----------------|----------|------------|
| **AssistNow (MGA)** | 3.11 | Faster cold start | High |
| **PPP-RTK / SPARTN** | 3.1.6 | Global corrections | Medium |
| **Fixed Position Base** | 3.1.8 | Surveyed base station | Low |
| **Antenna Thresholds** | 3.10 | Custom antenna tuning | Low |
| **PDOP Mask** | 3.1.8 | Solution quality gate | Low |
| **OSNMA** | 3.14.5 | Galileo authentication | Medium |

#### AssistNow (MGA)

Reduces Time-To-First-Fix from ~30s (cold) to <3s with assistance data.

**Options:**
- **AssistNow Online:** Download current ephemeris from u-blox servers
- **AssistNow Offline:** Pre-load extended ephemeris (weeks ahead)
- **AssistNow Autonomous:** Receiver predicts orbits internally

**Complexity:** Requires external HTTP requests or file loading, which doesn't fit the current driver architecture cleanly.

#### PPP-RTK / SPARTN

Precise Point Positioning with RTK-like performance using satellite-delivered corrections (u-blox PointPerfect service).

**Considerations:**
- Requires SPARTN protocol input
- Subscription service (not free like NTRIP casters)
- Growing in popularity for applications without local base stations

#### Fixed Position Base Mode

For users with surveyed base station coordinates (skip survey-in delay).

**CFG Keys:**
- `CFG-TMODE-MODE` = 2 (Fixed)
- `CFG-TMODE-POS_TYPE` (LLH or ECEF)
- `CFG-TMODE-ECEF_X/Y/Z` or `CFG-TMODE-LAT/LON/HEIGHT`
- `CFG-TMODE-ECEF_X/Y/Z_HP` (high-precision components)

### Tier 3: Low-Value Gaps (Not Recommended)

| Feature | Reason for Low Priority |
|---------|------------------------|
| **Geofencing** | Niche use case, application-layer concern |
| **On-device Logging** | Most ROS users log via rosbag |
| **QZSS SLAS** | Japan-only regional service |
| **Low Power Mode** | Incompatible with continuous ROS publishing |
| **Odometer** | Easily computed from position stream |
| **Spectrum Analyzer** | Diagnostic tool, not operational |

---

## Recommendations

### Design Principle

Maintain the simplified configuration philosophy. New features should be:

1. **Optional** - Don't break existing configs
2. **Well-defaulted** - Sensible defaults for common cases
3. **Documented** - Clear explanation of when/why to use
4. **Validated** - Reject invalid combinations at config load

### Proposed Configuration Extensions

#### 1. Navigation Settings Extension

```yaml
device:
  navigation:
    rate_hz: 10
    min_satellites: 4
    max_hdop: 5.0
    max_pdop: 10.0
    # NEW: Dynamic platform model
    dynamic_model: robot        # portable, stationary, pedestrian, automotive,
                                # sea, airborne_1g, airborne_2g, airborne_4g,
                                # wrist, bike, mower, escooter, robot
    # NEW: Elevation mask (degrees)
    elevation_mask: 10          # 0-90, reject satellites below this elevation
    
    # NEW: PDOP mask (optional)
    pdop_mask: 6.0              # Reject solutions with PDOP above this
```

#### 2. Timepulse Configuration (New Section)

```yaml
device:
  timepulse:
    enabled: true
    frequency_hz: 1             # 1 = 1 PPS, can be higher
    pulse_length_us: 100000     # Pulse width in microseconds
    polarity: rising            # rising or falling edge
    time_grid: gps              # utc, gps, glonass, beidou, galileo
    lock_required: true         # Only pulse when position fix valid
```

#### 3. Time Mark Configuration (New Section)

```yaml
device:
  time_mark:
    enabled: true
    trigger_edge: rising        # rising, falling, or both
    # Enables TIM-TM2 message output
```

**New ROS Topic:** `~/time_mark` (`sensor_msgs/TimeReference` with nanosecond event time)

#### 4. Static Base Extension

```yaml
mode: static_base

device:
  base_position:
    mode: survey_in             # survey_in or fixed
    # Survey-in settings (when mode: survey_in)
    survey_in:
      min_duration_s: 60
      accuracy_limit_m: 2.0
    # Fixed position (when mode: fixed)
    fixed:
      latitude: -35.12345678
      longitude: 149.12345678
      height_m: 600.123
      # Optional high-precision offsets (mm)
      lat_hp_mm: 12
      lon_hp_mm: -5
      height_hp_mm: 8
```

---

## Implementation Roadmap

### Phase 1: Navigation Extensions (Low Effort, High Value)

**Timeline:** 1-2 days  
**Scope:**
- Dynamic platform model
- Elevation mask
- PDOP mask

**Implementation:**
1. Add fields to `NavigationConfig` struct
2. Add CFG key mappings in `cfg_key_mapping.rs`
3. Apply configuration in device setup
4. Update documentation

**Breaking Changes:** None (new optional fields)

### Phase 2: Timepulse Support (Medium Effort, High Value)

**Timeline:** 2-3 days  
**Scope:**
- Timepulse configuration section
- GPIO configuration if needed

**Implementation:**
1. Add `TimepulseConfig` struct
2. Map all `CFG-TP-*` keys
3. Apply configuration in device setup
4. Document hardware wiring requirements

**Breaking Changes:** None

### Phase 3: Time Mark / External Event (Medium Effort, High Value)

**Timeline:** 2-3 days  
**Scope:**
- Time mark configuration
- `TIM-TM2` message parsing (if not in ublox-rs)
- New `~/time_mark` topic

**Implementation:**
1. Add `TimeMarkConfig` struct
2. Implement `TIM-TM2` parser (check ublox-rs support)
3. Add ROS publisher for `~/time_mark`
4. Document EXTINT pin wiring

**Dependencies:** May require ublox-rs update if `TIM-TM2` not supported

### Phase 4: Static Base Enhancement (Low Effort, Medium Value)

**Timeline:** 1 day  
**Scope:**
- Fixed position base mode
- Survey-in parameter configuration

**Implementation:**
1. Add `BasePositionConfig` struct
2. Map `CFG-TMODE-*` keys
3. Apply configuration for `static_base` mode

**Breaking Changes:** None

### Phase 5: AssistNow (High Effort, Medium Value)

**Timeline:** 1-2 weeks  
**Scope:**
- AssistNow Online integration
- Optional: AssistNow Offline file loading

**Implementation:**
1. Add optional HTTP client dependency
2. Implement MGA-ANO/MGA-DBD message injection
3. Add configuration for u-blox token
4. Handle startup sequence (inject before nav starts)

**Considerations:**
- Adds network dependency
- Requires u-blox account/token
- May be better as separate optional feature/crate

### Future Considerations

| Feature | Trigger for Implementation |
|---------|---------------------------|
| PPP-RTK / SPARTN | User demand, PointPerfect adoption |
| OSNMA | F9P firmware update, security requirements |
| Advanced antenna config | User request for specific hardware |

---

## Design Considerations

### Backward Compatibility

All changes must be backward compatible:
- New config fields have defaults matching current behavior
- Existing YAML files continue to work unchanged
- No changes to existing ROS topic interfaces

### Validation

New configuration should be validated at load time:
- Dynamic model values must be valid enum variants
- Elevation mask must be 0-90
- Timepulse frequency must be positive
- Fixed base coordinates must be valid ranges

### Documentation

Each new feature requires:
- CONFIGURATION.md update with examples
- README.md mention if significant
- Inline code comments for CFG key mappings

### Testing

- Unit tests for config parsing
- Integration tests with mock device (if feasible)
- Hardware validation checklist for timepulse/time mark

---

## Appendix: CFG Key Reference

### Navigation Engine (CFG-NAVSPG-*)

| Key | Type | Description |
|-----|------|-------------|
| `CFG-NAVSPG-DYNMODEL` | U1 | Dynamic platform model (0-13) |
| `CFG-NAVSPG-ELEV_MASK` | I1 | Elevation mask (degrees, signed) |
| `CFG-NAVSPG-PDOP_MASK` | U2 | PDOP mask (scaled by 10) |
| `CFG-NAVSPG-PL_ENA` | L | Protection level enable |

### Timepulse (CFG-TP-*)

| Key | Type | Description |
|-----|------|-------------|
| `CFG-TP-PULSE_DEF` | L | Pulse definition (0=period, 1=freq) |
| `CFG-TP-PULSE_LENGTH_DEF` | L | Length def (0=ratio, 1=length) |
| `CFG-TP-FREQ_TP1` | U4 | Frequency in Hz |
| `CFG-TP-FREQ_LOCK_TP1` | U4 | Frequency when locked |
| `CFG-TP-LEN_TP1` | U4 | Pulse length (us or 2^-32) |
| `CFG-TP-LEN_LOCK_TP1` | U4 | Pulse length when locked |
| `CFG-TP-TIMEGRID_TP1` | E1 | Time grid (0=UTC, 1=GPS, etc.) |
| `CFG-TP-POL_TP1` | L | Polarity (0=falling, 1=rising) |
| `CFG-TP-ALIGN_TO_TOW_TP1` | L | Align to top of second |
| `CFG-TP-USE_LOCKED_TP1` | L | Use locked parameters |

### Time Mode (CFG-TMODE-*)

| Key | Type | Description |
|-----|------|-------------|
| `CFG-TMODE-MODE` | E1 | Mode (0=disabled, 1=survey-in, 2=fixed) |
| `CFG-TMODE-POS_TYPE` | E1 | Position type (0=ECEF, 1=LLH) |
| `CFG-TMODE-ECEF_X` | I4 | ECEF X coordinate (cm) |
| `CFG-TMODE-ECEF_Y` | I4 | ECEF Y coordinate (cm) |
| `CFG-TMODE-ECEF_Z` | I4 | ECEF Z coordinate (cm) |
| `CFG-TMODE-ECEF_X_HP` | I1 | High-precision X (0.1mm) |
| `CFG-TMODE-ECEF_Y_HP` | I1 | High-precision Y (0.1mm) |
| `CFG-TMODE-ECEF_Z_HP` | I1 | High-precision Z (0.1mm) |
| `CFG-TMODE-LAT` | I4 | Latitude (1e-7 degrees) |
| `CFG-TMODE-LON` | I4 | Longitude (1e-7 degrees) |
| `CFG-TMODE-HEIGHT` | I4 | Height (cm) |
| `CFG-TMODE-LAT_HP` | I1 | High-precision lat (1e-9 deg) |
| `CFG-TMODE-LON_HP` | I1 | High-precision lon (1e-9 deg) |
| `CFG-TMODE-HEIGHT_HP` | I1 | High-precision height (0.1mm) |
| `CFG-TMODE-SVIN_MIN_DUR` | U4 | Survey-in min duration (s) |
| `CFG-TMODE-SVIN_ACC_LIMIT` | U4 | Survey-in accuracy limit (0.1mm) |

---

## Summary

This roadmap provides a structured approach to expanding Oxide GNSS configuration while maintaining its core philosophy of simplicity. The phased approach allows incremental delivery of high-value features without disrupting existing users.

**Immediate next steps:**
1. Review and approve this roadmap
2. Begin Phase 1 implementation (Dynamic Platform Model)
3. Create tracking issues for each phase

---

*Document authored as part of feature branch `feature/configuration-expansion-roadmap`*
