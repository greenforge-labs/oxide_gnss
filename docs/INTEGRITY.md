# Integrity Monitoring

This document describes the GNSS integrity monitoring system in `oxide_gnss`.

**Implementation:** `state/integrity.rs`

---

## Overview

The integrity system aggregates multiple receiver signals into a single integrity assessment, published on the `~/integrity` and `~/operational` topics.

This is intended as **quality monitoring and gating** for autonomy stacks. It is **not** a certified safety monitor.

---

## 1. Integrity Levels

| Level | Value | Meaning |
|-------|-------|---------|
| `OK` | 0 | All checks pass; full operation permitted |
| `DEGRADED` | 1 | Some quality checks failed; reduced-capability operation |
| `CRITICAL` | 2 | Critical checks failed; operation should stop |
| `FAILED` | 3 | System failed or data unavailable |

The integrity level is computed as the **maximum** (worst) of all individual check results.

---

## 2. Required UBX Messages

The integrity feature requires these UBX messages to be enabled:

| Message | Purpose |
|---------|---------|
| `NAV_PVT` | Fix type, accuracy, satellites, correction age |
| `NAV_PL` | Protection levels, TMIR (ISO 26262 integrity bounds) |
| `SEC_SIG` | Jamming/spoofing detection |
| `MON_RF` | Antenna status, jamming indicator |
| `MON_COMMS` | Communication port health |
| `NAV_SAT` | Per-satellite C/N0 for signal quality |

Optional messages that improve integrity observability:

| Message | Purpose |
|---------|---------|
| `SEC_SIGLOG` | Security event logging |
| `RXM_COR` | Correction status (SPARTN only) |
| `NAV_COV` | Position/velocity covariance (not available on all F9 variants) |

---

## 3. Per-Field Integrity Criteria

### 3.1 NAV-PVT Fields

| Field | Threshold | Integrity Contribution |
|-------|-----------|------------------------|
| `fixType` | `NoFix` | → **CRITICAL** ("No GNSS fix") |
| `fixType` | `Fix2D`, `DeadReckoning`, `TimeOnly` | → **CRITICAL** ("Insufficient fix type") |
| `fixType` | `Fix3D`, `GnssDr`, `RtkFloat`, `RtkFixed` | → OK (pass) |
| `numSV` | `< min_satellites_critical` (default: 4) | → **CRITICAL** ("Too few satellites") |
| `numSV` | `< min_satellites_high` (default: 6) | → **DEGRADED** ("Low satellite count") |
| `hAcc` | `> max_h_accuracy_m` (default: 0.10 m) | → **DEGRADED** ("Horizontal accuracy exceeded") |
| `vAcc` | `> max_v_accuracy_m` (default: 0.15 m) | → **DEGRADED** ("Vertical accuracy exceeded") |
| `pDOP` | `> max_pdop` (default: 3.0) | → **DEGRADED** ("PDOP too high") |
| `carrSoln` | `< 2` (not fixed) AND `differential_applied` | → **DEGRADED** ("RTK not fixed") |

### 3.2 SEC-SIG Fields

| Field | Value | Integrity Contribution |
|-------|-------|------------------------|
| `jammingState` | `Critical` (3) | → **CRITICAL** ("Critical jamming detected") |
| `jammingState` | `Warning` (2) | → OK, but logged ("Jamming warning") |
| `jammingState` | `Ok` (1) or `Unknown` (0) | → OK (pass) |
| `spoofingState` | `Multiple` (3) | → **CRITICAL** ("Multiple spoofers detected") |
| `spoofingState` | `Indicated` (2) | → OK, but logged ("Spoofing indicated") |
| `spoofingState` | `Ok` (1) or `Unknown` (0) | → OK (pass) |

### 3.3 MON-RF Fields

| Field | Value | Integrity Contribution |
|-------|-------|------------------------|
| `antStatus` | `Short` (3) | → **CRITICAL** ("Antenna short circuit") |
| `antStatus` | `Open` (4) | → **CRITICAL** ("Antenna open circuit") |
| `antStatus` | `Ok` (2), `Unknown` (1), `Init` (0) | → OK (pass) |
| `jamInd` | 0-255 | Published only; no integrity level impact |

### 3.4 Correction Age (Device-Reported)

The correction age is reported by the device in NAV-PVT `flags3.lastCorrectionAge` (bits 4..1).
This is a 4-bit index into non-linear age bands, converted to the upper bound in seconds:

| Index | Age Band | Upper Bound (s) |
|-------|----------|----------------|
| 0 | Not available | - |
| 1 | 0-1s | 1 |
| 2 | 1-2s | 2 |
| 3 | 2-5s | 5 |
| 4 | 5-10s | 10 |
| 5 | 10-15s | 15 |
| 6 | 15-20s | 20 |
| 7 | 20-30s | 30 |
| 8 | 30-45s | 45 |
| 9 | 45-60s | 60 |
| 10 | 60-90s | 90 |
| 11 | 90-120s | 120 |
| ≥12 | >120s | 255 |

| Condition | Integrity Contribution |
|-----------|------------------------|
| `differential_applied` AND `correction_age_s > max_correction_age_s` (default: 10.0 s) | → **DEGRADED** ("Correction age exceeded") |

### 3.5 Signal Quality (NAV-SAT)

Signal quality metrics are derived from satellites used in the navigation solution:

| Field | Threshold | Integrity Contribution |
|-------|-----------|------------------------|
| `min_cno` | `< min_cno_degraded` (default: 25 dB-Hz) | → **DEGRADED** ("Weak satellite signal") |
| `mean_cno` | `< min_mean_cno_degraded` (default: 35.0 dB-Hz) | → **DEGRADED** ("Low mean signal quality") |

### 3.6 Security Events (SEC-SIGLOG)

| Condition | Integrity Contribution |
|-----------|------------------------|
| `security_events > 0` | → OK, but logged ("Security events logged") |

### 3.7 Protection Levels (NAV-PL)

Protection levels provide statistically-bounded error estimates with a specified Target Misleading Information Risk (TMIR), enabling ISO 26262/ISO 21448 (SOTIF) compliant integrity monitoring.

| Metric | Threshold | Integrity Contribution |
|--------|-----------|------------------------|
| `horizontal_pl_m` | `> max_horizontal_pl_m` (default: 0.50m) | → **DEGRADED** ("Horizontal PL exceeds alert limit") |
| `vertical_pl_m` | `> max_vertical_pl_m` (default: 1.00m) | → **DEGRADED** ("Vertical PL exceeds alert limit") |
| `velocity_pl_ms` | `> max_velocity_pl_ms` (default: 0.10 m/s) | → **DEGRADED** ("Velocity PL exceeds alert limit") |
| `tmir` | `> max_tmir_per_epoch` (default: 1e-5) | → **DEGRADED** ("TMIR exceeds threshold") |
| `pos_valid` | `false` AND `require_valid_pl: true` | → **CRITICAL** ("Protection level invalid") |

**Note:** When NAV-PL is unavailable (older firmware), the system falls back to hAcc/vAcc threshold checks.

---

## 4. Algorithm

The overall integrity level is computed by evaluating checks in priority order:

### 4.1 Step 0: Staleness Check

If no PVT data has been received, or if the last PVT is older than `max_pvt_age_s` (default: 2.0s), return **FAILED** immediately.

This detects device disconnection, serial failures, or firmware crashes.

### 4.2 Step 1: Critical Checks

Any of these failing sets level to **CRITICAL**:
- Fix type: NoFix, Fix2D, DeadReckoning, TimeOnly
- Satellite count below critical threshold
- Critical jamming detected
- Multiple spoofers detected
- Antenna short or open circuit

### 4.3 Step 2: Degraded Checks

Only evaluated if not already CRITICAL:
- RTK not fixed (when differential applied)
- Horizontal/vertical accuracy exceeded
- PDOP too high
- Low satellite count
- Correction age exceeded
- Weak satellite signal (min C/N0)
- Low mean signal quality (mean C/N0)

### 4.4 Step 3: Monitor Checks

These do NOT change the level, only add to the status message:
- Jamming warning
- Spoofing indicated
- Security events logged

### 4.5 Pseudocode

```
function compute_integrity():
    level = OK
    issues = []

    # Step 0: Staleness
    if no PVT received OR pvt_age > max_pvt_age_s:
        return FAILED

    # Step 1: Critical checks
    for each critical_check:
        if failed: level = max(level, CRITICAL)

    # Step 2: Degraded checks (only if not critical)
    if level < CRITICAL:
        for each degraded_check:
            if failed: level = max(level, DEGRADED)

    # Step 3: Monitor checks (log only)
    for each monitor_check:
        if triggered: issues.append(warning)

    return level, issues
```

---

## 5. Operational Signal

The `~/operational` topic publishes a boolean derived from integrity:

```
operational = (integrity_level <= operational_threshold)
```

| `operational_threshold` | Meaning |
|-------------------------|---------|
| 0 | Only `OK` is operational (strictest) |
| 1 | `OK` or `DEGRADED` is operational (default) |
| 2 | `OK`, `DEGRADED`, or `CRITICAL` is operational (permissive) |

---

## 6. Configuration

All thresholds can be customized via YAML:

```yaml
integrity:
  thresholds:
    min_satellites_critical: 4      # Below this → CRITICAL
    min_satellites_high: 6          # Below this → DEGRADED
    max_h_accuracy_m: 0.10          # Above this → DEGRADED
    max_v_accuracy_m: 0.15          # Above this → DEGRADED
    max_pdop: 3.0                   # Above this → DEGRADED
    max_correction_age_s: 10.0      # Above this → DEGRADED (RTK)
    min_cno_degraded: 25            # Below this → DEGRADED (weakest sat)
    min_mean_cno_degraded: 35.0     # Below this → DEGRADED (mean C/N0)
    max_pvt_age_s: 2.0              # Above this → FAILED (staleness)
    operational_threshold: 1        # 0=strict, 1=default, 2=permissive
    
    # Protection Level thresholds (NAV-PL)
    max_horizontal_pl_m: 0.50       # Above this → DEGRADED
    max_vertical_pl_m: 1.00         # Above this → DEGRADED
    max_velocity_pl_ms: 0.10        # Above this → DEGRADED
    max_tmir_per_epoch: 1.0e-5      # Above this → DEGRADED
    require_valid_pl: false         # If true, invalid PL → CRITICAL
```

All fields are optional - omitted fields use defaults.

---

## 7. Output Message

The `~/integrity` topic publishes `oxide_gnss_msgs/OxideIntegrity`:

### 7.1 Status Fields

| Field | Type | Description |
|-------|------|-------------|
| `level` | uint8 | Overall integrity (0=OK, 1=DEGRADED, 2=CRITICAL, 3=FAILED) |
| `status_message` | string | Human-readable summary of issues |
| `operational` | bool | Whether system is operational per threshold |

### 7.2 Check Result Fields (Diagnostic Visibility)

These boolean fields indicate pass/fail status for each individual check, enabling visualization tools like Foxglove to display a grid of pass/fail indicators:

| Field | Description | Level if Failed |
|-------|-------------|-----------------|
| `check_fix_type_ok` | Fix type is acceptable | CRITICAL |
| `check_satellites_ok` | Satellite count meets threshold | CRITICAL/DEGRADED |
| `check_h_accuracy_ok` | Horizontal accuracy within limit | DEGRADED |
| `check_v_accuracy_ok` | Vertical accuracy within limit | DEGRADED |
| `check_pdop_ok` | PDOP within limit | DEGRADED |
| `check_carrier_ok` | Carrier solution is RTK Fixed | DEGRADED |
| `check_correction_age_ok` | Correction age within limit | DEGRADED |
| `check_signal_quality_ok` | Signal quality (C/N0) within limits | DEGRADED |
| `check_jamming_ok` | No critical jamming detected | CRITICAL |
| `check_spoofing_ok` | No multiple spoofers detected | CRITICAL |
| `check_antenna_ok` | Antenna status OK (not open/short) | CRITICAL |
| `check_pl_horizontal_ok` | Horizontal PL within alert limit | DEGRADED |
| `check_pl_vertical_ok` | Vertical PL within alert limit | DEGRADED |
| `check_pl_velocity_ok` | Velocity PL within alert limit | DEGRADED |
| `check_pl_valid_ok` | Protection level validity passed | CRITICAL/DEGRADED |

### 7.3 Position Quality Fields

| Field | Type | Description |
|-------|------|-------------|
| `fix_type` | uint8 | Current fix type |
| `carrier_solution` | uint8 | 0=none, 1=float, 2=fixed |
| `differential_applied` | bool | Whether RTK/DGNSS is in use |
| `num_satellites` | uint8 | Satellites used in solution |
| `h_accuracy_m` | float32 | Horizontal accuracy (m) |
| `v_accuracy_m` | float32 | Vertical accuracy (m) |
| `pdop` | float32 | Position DOP |
| `covariance_valid` | bool | Whether NAV-COV data was received (matrices in NavSatFix/Twist) |

### 7.4 RTK/Correction Fields

| Field | Type | Description |
|-------|------|-------------|
| `correction_age_s` | float32 | Age of RTK corrections (-1 if N/A) |
| `correction_received` | bool | Whether corrections have been received |
| `correction_used` | bool | Whether corrections were used |

### 7.5 Security/Hardware Fields

| Field | Type | Description |
|-------|------|-------------|
| `jamming_state` | uint8 | 0=unknown, 1=ok, 2=warning, 3=critical |
| `spoofing_state` | uint8 | 0=unknown, 1=ok, 2=indicated, 3=multiple |
| `security_events` | uint32 | Number of security events logged |
| `jamming_indicator` | uint8 | CW jamming indicator (0-255) |
| `antenna_status` | uint8 | 0=unknown, 1=ok, 2=open, 3=short |
| `comm_ports` | uint8 | Number of communication ports |
| `comm_tx_errors` | uint8 | TX errors detected |

### 7.6 Signal Quality Fields

| Field | Type | Description |
|-------|------|-------------|
| `mean_cno` | float32 | Mean C/N0 of satellites used (dB-Hz) |
| `min_cno` | uint8 | Minimum C/N0 among used satellites (dB-Hz) |
| `sats_above_cno_threshold` | uint8 | Satellites with C/N0 >= 30 dB-Hz |

### 7.7 Protection Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `protection_level_valid` | bool | PL data valid and current |
| `horizontal_pl_m` | float32 | Horizontal protection level (m) |
| `vertical_pl_m` | float32 | Vertical protection level (m) |
| `velocity_pl_ms` | float32 | Velocity protection level (m/s) |
| `target_mir` | float64 | Target Misleading Information Risk |
| `pl_frame` | uint8 | PL reference frame |
| `pl_invalidity_reason` | uint8 | Reason if PL invalid |

---

## 8. Update Flow

```
UBX Messages from Device
         │
         ├── NAV_PVT ──────► update_pvt()
         ├── NAV_PL ───────► update_nav_pl()
         ├── NAV_SAT ──────► update_signal_quality()
         ├── SEC_SIG ──────► update_sec_sig()
         ├── MON_RF ───────► update_mon_rf()
         ├── MON_COMMS ────► update_mon_comms()
         ├── NAV_COV ──────► update_covariance()
         ├── RXM_COR ──────► update_rxm_cor()
         └── SEC_SIGLOG ───► update_sec_siglog()
                   │
                   ▼
         IntegrityAggregator.compute()
                   │
                   ▼
            GnssIntegrity
                   │
         ┌────────┴────────┐
         ▼                 ▼
   ~/integrity      ~/operational
```

---

## Notes

- The algorithm runs each time a NAV-PVT message is received (typically at the navigation rate, e.g., 5-10 Hz).
- Monitor-level checks (jamming warning, spoofing indicated) are logged but do not affect the integrity level, allowing operators to be informed without prematurely stopping operations.
- If you enable `features.integrity: true`, ensure the required UBX messages are enabled. Startup validation will warn when configuration is incomplete.
