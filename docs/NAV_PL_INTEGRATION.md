# NAV-PL Integration for oxide_gnss

This document describes the integration of UBX-NAV-PL (Protection Level) message support
into oxide_gnss for enhanced GNSS integrity monitoring in autonomous vehicle applications.

**Created:** 2024-12-24  
**Status:** In Progress  
**Feature Branch:** `feature/nav-pl-integration`

---

## Overview

NAV-PL provides statistically-bounded Protection Levels with an associated Target Misleading
Information Risk (TMIR), enabling ISO 26262/ISO 21448 (SOTIF) compliant integrity monitoring.

### Key Benefits

- **Statistically rigorous bounds** — Unlike hAcc/vAcc (68% confidence), NAV-PL provides
  TMIR-bounded protection levels (e.g., 10^-7/h integrity risk)
- **ISO 26262 alignment** — ASIL-B compatible integrity information
- **Explicit invalidity reasons** — Clear failure mode visibility
- **Receiver-computed** — Reduces driver computational burden

---

## NAV-PL Message Structure

From the forked ublox crate (`gsokoll/ublox:feature/nav-pl-support`):

| Field | Type | Description |
|-------|------|-------------|
| `version` | u8 | Message version (0x01) |
| `tmir_coeff` | u8 | TMIR coefficient (TMIR = coeff × 10^exp) |
| `tmir_exp` | i8 | TMIR exponent |
| `pl_pos_valid` | enum | Position PL validity (Invalid/Valid) |
| `pl_pos_frame` | enum | Position frame (Invalid/NED/LongLatVert/Ellipse) |
| `pl_vel_valid` | enum | Velocity PL validity |
| `pl_vel_frame` | enum | Velocity frame |
| `pl_time_valid` | enum | Time PL validity |
| `pl_pos_invalidity_reason` | enum | Why position PL is invalid |
| `pl_vel_invalidity_reason` | enum | Why velocity PL is invalid |
| `pl_time_invalidity_reason` | enum | Why time PL is invalid |
| `itow` | u32 | GPS time of week (ms) |
| `pl_pos1/2/3` | u32 | Position PL per axis (mm) |
| `pl_vel1/2/3` | u32 | Velocity PL per axis (mm/s) |
| `pl_pos_horiz_orient` | u16 | Horizontal ellipse orientation (0.01°) |
| `pl_vel_horiz_orient` | u16 | Velocity ellipse orientation (0.01°) |
| `pl_time` | u32 | Time PL (ns) |

### Protection Level Frames

- **NED** — North, East, Down
- **LongLatVert** — Longitudinal, Lateral, Vertical (vehicle frame)
- **HorizSemiMajorMinorVert** — Error ellipse semi-major/minor + vertical

### Invalidity Reasons

| Value | Meaning |
|-------|---------|
| 0 | Not available |
| 1-29 | Solution not trustworthy |
| 30-100 | PL not verified for this receiver configuration |

---

## Current Integrity Approach vs NAV-PL

### Current Implementation

The existing `IntegrityAggregator` in `src/state/integrity.rs` uses heuristic checks:

| Signal Source | Current Use |
|---------------|-------------|
| NAV-PVT `hAcc`/`vAcc` | Accuracy threshold checks (68% confidence) |
| NAV-PVT `numSV`, `fixType` | Solution quality indicators |
| NAV-PVT `pDOP` | Geometry quality |
| SEC-SIG | Jamming/spoofing detection |
| MON-RF | Antenna status, CW jamming |
| NAV-SAT | Per-satellite C/N0 |
| NAV-COV | Position/velocity covariance |

### With NAV-PL Integration

| Signal Source | New Use |
|---------------|---------|
| **NAV-PL** | Primary integrity bounds (TMIR-qualified) |
| NAV-PVT `hAcc`/`vAcc` | Fallback/cross-check only |
| NAV-PVT `numSV`, `fixType` | Retained for solution type |
| SEC-SIG | Retained (PL doesn't cover security) |
| MON-RF | Retained (PL doesn't cover antenna) |
| NAV-SAT | Retained for signal quality monitoring |
| NAV-COV | Optional (PL provides tighter bounds) |

---

## Recommended Message Set

| Message | Rate | Purpose | Notes |
|---------|------|---------|-------|
| NAV-PVT | 5-10 Hz | Core PVT, fix type, numSV | Essential |
| NAV-PL | 1-5 Hz | Protection levels, TMIR | **NEW** |
| SEC-SIG | 1 Hz | Jamming/spoofing | Security layer |
| MON-RF | 1 Hz | Antenna, jamming indicator | Hardware layer |
| NAV-SAT | 1 Hz | Signal quality | Monitoring |

**Demoted/Optional:**
- NAV-DOP — Redundant (PDOP in NAV-PVT)
- NAV-COV — Optional if NAV-PL available

---

## Configuration Additions

New thresholds for `integrity.thresholds` in YAML config:

```yaml
integrity:
  thresholds:
    # Existing thresholds (retained for fallback)
    max_h_accuracy_m: 0.10
    max_v_accuracy_m: 0.15
    max_pdop: 3.0
    
    # NEW: Protection Level Alert Limits
    max_horizontal_pl_m: 0.50      # Horizontal PL alert limit (meters)
    max_vertical_pl_m: 1.00        # Vertical PL alert limit (meters)
    max_velocity_pl_ms: 0.10       # Velocity PL alert limit (m/s)
    
    # NEW: TMIR threshold
    max_tmir_per_epoch: 1.0e-5     # Reject if TMIR worse than this
    
    # NEW: Require NAV-PL for operational status
    require_valid_pl: false        # If true, invalid PL → CRITICAL
```

---

## OxideIntegrity.msg Extensions

New fields to add:

```msg
# Protection Level data (from NAV-PL)
bool protection_level_valid         # True if PL data is valid and current
float32 horizontal_pl_m             # Horizontal protection level (m)
float32 vertical_pl_m               # Vertical protection level (m)
float32 velocity_pl_ms              # Velocity protection level magnitude (m/s)
float64 target_mir                  # Target Misleading Information Risk
uint8 pl_frame                      # 0=invalid, 1=NED, 2=LLV, 3=ellipse
uint8 pl_invalidity_reason          # 0=valid, 1-29=not trustworthy, 30+=not verified

uint8 PL_FRAME_INVALID = 0
uint8 PL_FRAME_NED = 1
uint8 PL_FRAME_LONG_LAT_VERT = 2
uint8 PL_FRAME_ELLIPSE = 3

uint8 PL_REASON_VALID = 0
uint8 PL_REASON_NOT_TRUSTWORTHY = 1
uint8 PL_REASON_NOT_VERIFIED = 30
```

---

## Implementation Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Use forked ublox crate with NAV-PL |
| `src/device/ubx.rs` | Add `NavPlData` struct and parsing |
| `src/device/mod.rs` | Export new types |
| `src/state/integrity.rs` | Add PL to `IntegrityAggregator` |
| `src/config/integrity.rs` | Add PL threshold config |
| `oxide_gnss_msgs/msg/OxideIntegrity.msg` | Add PL fields |
| `src/ros/conversions.rs` | Map PL to ROS message |
| `config/rover_integrity.yaml` | Add PL thresholds |
| `docs/INTEGRITY.md` | Document PL integration |

---

## Device Compatibility

NAV-PL is available on:

| Device | Firmware | Protocol |
|--------|----------|----------|
| ZED-F9P | HPG 1.30+ | 27+ |
| ZED-F9R | HPS 1.30+ | 33+ |
| M10 | SPG 5.10+ | 34+ |
| NEO-F10N | SPG 6.00+ | 40+ |

**Note:** X20/F20 devices do NOT support NAV-PL.

---

## References

1. **u-blox PointSafe** — [u-blox.com/product/pointsafe](https://www.u-blox.com/en/product/pointsafe)
   - "Target Integrity Risk up to 10^-7/h"
   - "ASIL-B (ISO 26262) and SOTIF (ISO 21448) certified"

2. **u-blox Functional Safety Blog** — [u-blox.com/blogs/tech/functional-safety-sotif-protection-level](https://www.u-blox.com/en/blogs/tech/functional-safety-sotif-protection-level)
   - "Protection Level (PL) is an instantaneous actual error bound based on precise statistical modeling"
   - "TIR represents the maximum tolerable rate of hazardous misleading events"

3. **ISO 26262:2018** — Functional safety for road vehicles

4. **ISO 21448:2022** (SOTIF) — Safety of the Intended Functionality

5. **NovAtel Functional Safety** — [novatel.com/autonomy/functional-safety](https://novatel.com/an-introduction-to-gnss/autonomy/functional-safety)
   - "Integrity is a measure of trust for the information supplied by the positioning solution"

6. **u-blox ZED-F9P Interface Description (UBX-18010854)** — NAV-PL message specification

---

## Integrity Algorithm Update

### Current Algorithm (Pseudocode)

```
if hAcc > max_h_accuracy_m → DEGRADED
if vAcc > max_v_accuracy_m → DEGRADED
if fixType == NoFix → CRITICAL
if jamming == Critical → CRITICAL
...
```

### Updated Algorithm with NAV-PL

```
# Step 0: Staleness check (unchanged)

# Step 1: Critical checks
if nav_pl.pos_valid == Invalid AND require_valid_pl:
    level = CRITICAL
    issues.push("Protection level invalid")
    
if nav_pl.invalidity_reason in [1..29]:
    level = CRITICAL  
    issues.push("Solution not trustworthy")

# Existing critical checks (fix type, jamming, antenna)...

# Step 2: Degraded checks  
if nav_pl.pos_valid == Valid:
    horizontal_pl = sqrt(pl_pos1² + pl_pos2²) / 1000.0  # mm to m
    vertical_pl = pl_pos3 / 1000.0
    
    if horizontal_pl > max_horizontal_pl_m:
        level = max(level, DEGRADED)
        issues.push("Horizontal PL exceeds alert limit")
        
    if vertical_pl > max_vertical_pl_m:
        level = max(level, DEGRADED)
        issues.push("Vertical PL exceeds alert limit")
else:
    # Fallback to hAcc/vAcc if PL not available
    if hAcc > max_h_accuracy_m:
        level = max(level, DEGRADED)
        issues.push("Horizontal accuracy exceeded (fallback)")

# TMIR check
tmir = nav_pl.tmir_coeff * 10^nav_pl.tmir_exp
if tmir > max_tmir_per_epoch:
    level = max(level, DEGRADED)
    issues.push("TMIR exceeds threshold")
```

---

## Testing Considerations

1. **Unit tests** for NAV-PL parsing with known payloads
2. **Integration tests** with live F9P receiver (HPG 1.30+)
3. **Fallback behavior** when NAV-PL not available (older firmware)
4. **Frame conversion tests** (NED → ENU for ROS compatibility)
5. **Threshold boundary tests** for PL alert limits
