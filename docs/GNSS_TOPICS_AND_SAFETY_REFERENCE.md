# GNSS Topics and Safety Reference

This document provides a comprehensive reference for GNSS ROS2 topics, safety considerations, and the strategic direction for oxide_gnss as a safety-critical GNSS driver.

---

## Table of Contents

1. [ublox_dgnss Topic Reference](#1-ublox_dgnss-topic-reference)
2. [ublox-rs Crate Support Status](#2-ublox-rs-crate-support-status)
3. [Standard GNSS Topics for Mobile Robotics](#3-standard-gnss-topics-for-mobile-robotics)
4. [Safety-Critical Integrity Monitoring](#4-safety-critical-integrity-monitoring)
5. [oxide_gnss Strategic Direction](#5-oxide_gnss-strategic-direction)

---

## 1. ublox_dgnss Topic Reference

The [ublox_dgnss](https://github.com/aussierobots/ublox_dgnss) driver publishes raw UBX message types. Topics are enabled by setting corresponding `CFG_MSGOUT_*_USB` parameters to >0.

### 1.1 NAV Class Messages (Navigation)

| Topic | Message Type | CFG Parameter | UBX ID | Description |
|-------|--------------|---------------|--------|-------------|
| `ubx_nav_clock` | `UBXNavClock` | `CFG_MSGOUT_UBX_NAV_CLOCK_USB` | 0x01 0x22 | Clock solution (bias, drift) |
| `ubx_nav_cov` | `UBXNavCov` | `CFG_MSGOUT_UBX_NAV_COV_USB` | 0x01 0x36 | Position/velocity covariance matrix |
| `ubx_nav_dop` | `UBXNavDOP` | `CFG_MSGOUT_UBX_NAV_DOP_USB` | 0x01 0x04 | Dilution of precision values |
| `ubx_nav_eoe` | `UBXNavEOE` | `CFG_MSGOUT_UBX_NAV_EOE_USB` | 0x01 0x61 | End of epoch marker |
| `ubx_nav_hp_pos_ecef` | `UBXNavHPPosECEF` | `CFG_MSGOUT_UBX_NAV_HPPOSECEF_USB` | 0x01 0x13 | High precision ECEF position |
| `ubx_nav_hp_pos_llh` | `UBXNavHPPosLLH` | `CFG_MSGOUT_UBX_NAV_HPPOSLLH_USB` | 0x01 0x14 | High precision geodetic position |
| `ubx_nav_odo` | `UBXNavOdo` | `CFG_MSGOUT_UBX_NAV_ODO_USB` | 0x01 0x09 | Odometer solution |
| `ubx_nav_orb` | `UBXNavOrb` | `CFG_MSGOUT_UBX_NAV_ORB_USB` | 0x01 0x34 | Orbit database info |
| `ubx_nav_sat` | `UBXNavSat` | `CFG_MSGOUT_UBX_NAV_SAT_USB` | 0x01 0x35 | Satellite information |
| `ubx_nav_sig` | `UBXNavSig` | `CFG_MSGOUT_UBX_NAV_SIG_USB` | 0x01 0x43 | Signal information |
| `ubx_nav_pos_ecef` | `UBXNavPosECEF` | `CFG_MSGOUT_UBX_NAV_POSECEF_USB` | 0x01 0x01 | ECEF position |
| `ubx_nav_pos_llh` | `UBXNavPosLLH` | `CFG_MSGOUT_UBX_NAV_POSLLH_USB` | 0x01 0x02 | Geodetic position |
| `ubx_nav_pvt` | `UBXNavPVT` | `CFG_MSGOUT_UBX_NAV_PVT_USB` | 0x01 0x07 | Position, velocity, time solution |
| `ubx_nav_rel_pos_ned` | `UBXNavRelPosNED` | `CFG_MSGOUT_UBX_NAV_RELPOSNED_USB` | 0x01 0x3C | Relative position (moving base/heading) |
| `ubx_nav_status` | `UBXNavStatus` | `CFG_MSGOUT_UBX_NAV_STATUS_USB` | 0x01 0x03 | Receiver navigation status |
| `ubx_nav_svin` | `UBXNavSvin` | N/A (polled) | 0x01 0x3B | Survey-in status |
| `ubx_nav_time_utc` | `UBXNavTimeUTC` | `CFG_MSGOUT_UBX_NAV_TIMEUTC_USB` | 0x01 0x21 | UTC time solution |
| `ubx_nav_vel_ecef` | `UBXNavVelECEF` | `CFG_MSGOUT_UBX_NAV_VELECEF_USB` | 0x01 0x11 | ECEF velocity |
| `ubx_nav_vel_ned` | `UBXNavVelNED` | `CFG_MSGOUT_UBX_NAV_VELNED_USB` | 0x01 0x12 | NED velocity |

### 1.2 RXM Class Messages (Receiver Manager)

| Topic | Message Type | CFG Parameter | UBX ID | Description |
|-------|--------------|---------------|--------|-------------|
| `ubx_rxm_cor` | `UBXRxmCor` | `CFG_MSGOUT_UBX_RXM_COR_USB` | 0x02 0x34 | Correction status (X20P) |
| `ubx_rxm_rtcm` | `UBXRxmRTCM` | `CFG_MSGOUT_UBX_RXM_RTCM_USB` | 0x02 0x32 | RTCM input status |
| `ubx_rxm_measx` | `UBXRxmMeasx` | `CFG_MSGOUT_UBX_RXM_MEASX_USB` | 0x02 0x14 | Multi-GNSS raw measurements |
| `ubx_rxm_rawx` | `UBXRxmRawx` | `CFG_MSGOUT_UBX_RXM_RAWX_USB` | 0x02 0x15 | Raw measurement data |
| `ubx_rxm_spartn` | `UBXRxmSpartn` | `CFG_MSGOUT_UBX_RXM_SPARTN_USB` | 0x02 0x33 | SPARTN input status |
| `ubx_rxm_spartnkey` | `UBXRxmSpartnKey` | N/A | 0x02 0x36 | SPARTN decryption keys |

### 1.3 ESF Class Messages (External Sensor Fusion)

| Topic | Message Type | CFG Parameter | UBX ID | Description |
|-------|--------------|---------------|--------|-------------|
| `ubx_esf_status` | `UBXEsfStatus` | `CFG_MSGOUT_UBX_ESF_STATUS_USB` | 0x10 0x10 | Sensor fusion status (F9R) |
| `ubx_esf_meas` | `UBXEsfMeas` | `CFG_MSGOUT_UBX_ESF_MEAS_USB` | 0x10 0x02 | External sensor measurements |

### 1.4 MON Class Messages (Monitoring)

| Topic | Message Type | CFG Parameter | UBX ID | Description |
|-------|--------------|---------------|--------|-------------|
| `ubx_mon_comms` | `UBXMonComms` | `CFG_MSGOUT_UBX_MON_COMMS_USB` | 0x0A 0x36 | Communication port status |

### 1.5 SEC Class Messages (Security)

| Topic | Message Type | CFG Parameter | UBX ID | Description |
|-------|--------------|---------------|--------|-------------|
| `ubx_sec_sig` | `UBXSecSig` | `CFG_MSGOUT_UBX_SEC_SIG_USB` | 0x27 0x09 | Jamming/spoofing indicators |
| `ubx_sec_sig_log` | `UBXSecSigLog` | `CFG_MSGOUT_UBX_SEC_SIGLOG_USB` | 0x27 0x10 | Security event log |

### 1.6 Other Topics

| Topic | Message Type | Description |
|-------|--------------|-------------|
| `rtcm` | `rtcm_msgs/Message` | Raw RTCM passthrough |

### 1.7 Composite Topics (Separate Node)

The `/fix` topic is published by `ublox_nav_sat_fix_hp_node`, which subscribes to multiple UBX topics:

| Topic | Message Type | Required UBX Messages |
|-------|--------------|----------------------|
| `/fix` | `sensor_msgs/NavSatFix` | NAV-HPPOSLLH + NAV-COV + NAV-STATUS |

### 1.8 Subscribed Topics

| Topic | Message Type | Description |
|-------|--------------|-------------|
| `/ubx_esf_meas_to_device` | `UBXEsfMeas` | External sensor data to device |
| `/ntrip_client/rtcm` | `rtcm_msgs/Message` | RTCM corrections input |

---

## 2. ublox-rs Crate Support Status

The [ublox-rs](https://github.com/ublox-rs/ublox) crate (v0.8.0) provides Rust parsing for UBX protocol.

### 2.1 Supported Messages

| UBX Message | ublox-rs Module | Notes |
|-------------|-----------------|-------|
| NAV-ATT | `nav_att` | Attitude (F9R) |
| NAV-CLOCK | `nav_clock` | ✅ |
| NAV-DOP | `nav_dop` | ✅ |
| NAV-EOE | `nav_other::NavEoe` | ✅ |
| NAV-HPPOSECEF | `nav_hp_pos_ecef` | ✅ |
| NAV-HPPOSLLH | `nav_hp_pos_llh` | ✅ |
| NAV-ODO | `nav_other::NavOdo` | ✅ |
| NAV-POSLLH | `nav_pos_llh` | ✅ |
| NAV-PVT | `nav_pvt` | ✅ |
| NAV-RELPOSNED | `nav_rel_pos_ned` | ✅ |
| NAV-SAT | `nav_sat` | ✅ |
| NAV-SIG | `nav_sig` | ✅ |
| NAV-SOL | `nav_sol` | Legacy |
| NAV-STATUS | `nav_status` | ✅ |
| NAV-TIMELS | `nav_time_ls` | Leap second |
| NAV-TIMEUTC | `nav_time_utc` | ✅ |
| NAV-VELECEF | `nav_other::NavVelECEF` | ✅ |
| NAV-VELNED | `nav_vel_ned` | ✅ |
| RXM-RAWX | `rxm_rawx` | ✅ |
| RXM-RTCM | `rxm_rtcm` | ✅ |
| RXM-SFRBX | `rxm_sfrbx` | Subframe |
| ESF-ALG | `esf_alg` | ✅ |
| ESF-INS | `esf_ins` | ✅ |
| ESF-MEAS | `esf_meas` | ✅ |
| ESF-RAW | `esf_raw` | ✅ |
| ESF-STATUS | `esf_status` | ✅ |
| MON-GNSS | `mon_gnss` | ✅ |
| MON-HW/HW2/HW3 | `mon_hw*` | ✅ |
| MON-RF | `mon_rf` | ✅ |
| MON-VER | `mon_ver` | ✅ |
| SEC-UNIQID | `sec_uniq_id` | ✅ |
| TIM-SVIN | `tim_svin` | ✅ |
| TIM-TM2/TOS/TP | `tim_*` | ✅ |
| MON-HW | `mon_hw` | ✅ |

### 2.2 Safety Messages (upstream ublox-rs master)

The following safety-related messages are supported in upstream ublox-rs (master branch) and used by oxide_gnss:

| UBX Message | UBX ID | Status | Use Case |
|-------------|--------|--------|----------|
| **NAV-COV** | 0x01 0x36 | ✅ Implemented | Full position/velocity covariance matrix |
| **NAV-POSECEF** | 0x01 0x01 | ✅ Implemented | ECEF position |
| **RXM-COR** | 0x02 0x34 | ✅ Implemented | Correction status (replaces RXM-RTCM) |
| **MON-COMMS** | 0x0A 0x36 | ✅ Implemented | Port diagnostics |
| **SEC-SIG** | 0x27 0x09 | ✅ Implemented | Jamming/spoofing detection |
| **SEC-SIGLOG** | 0x27 0x10 | ✅ Implemented | Security event log |

### 2.3 Still NOT Supported

| UBX Message | UBX ID | Priority | Use Case |
|-------------|--------|----------|----------|
| NAV-ORB | 0x01 0x34 | Low | Orbit database info |
| NAV-SVIN | 0x01 0x3B | Medium | Survey-in (different from TIM-SVIN) |
| RXM-MEASX | 0x02 0x14 | Low | Multi-GNSS measurements |
| RXM-SPARTN | 0x02 0x33 | Low | SPARTN reception |
| RXM-SPARTNKEY | 0x02 0x36 | Low | SPARTN keys |

---

## 3. Standard GNSS Topics for Mobile Robotics

### 3.1 Essential Topics (robot_localization / Nav2 Compatible)

| Topic | Message Type | Source | Notes |
|-------|--------------|--------|-------|
| `/gps/fix` | `sensor_msgs/NavSatFix` | NAV-PVT or NAV-HPPOSLLH | **Primary** - used by navsat_transform_node |
| `/gps/vel` | `geometry_msgs/TwistWithCovarianceStamped` | NAV-PVT | Velocity for EKF fusion |
| `/imu/data` | `sensor_msgs/Imu` | ESF or external | For sensor fusion |

### 3.2 Common Additional Topics

| Topic | Message Type | Source | Notes |
|-------|--------------|--------|-------|
| `/gps/time` | `sensor_msgs/TimeReference` | NAV-PVT/TIMEUTC | Time synchronization |
| `/gps/odometry` | `nav_msgs/Odometry` | Computed | Local frame position |
| `/diagnostics` | `diagnostic_msgs/DiagnosticArray` | Aggregated | Health monitoring |

### 3.3 Topic Naming Conventions

```
/namespace/fix           # NavSatFix (most common)
/namespace/vel           # TwistWithCovarianceStamped
/namespace/time          # TimeReference
/namespace/odometry      # Odometry (local frame)
/diagnostics             # Global diagnostics topic
```

---

## 4. Safety-Critical Integrity Monitoring

### 4.1 Data Sources for Integrity Monitoring

#### Position Quality

| Metric | UBX Source | Field(s) | Threshold Example |
|--------|------------|----------|-------------------|
| Fix Type | NAV-PVT | `fixType` | ≥ 3 (3D fix) |
| GNSS Fix Valid | NAV-PVT | `flags.gnssFixOK` | = true |
| RTK Solution | NAV-PVT | `flags.carrSoln` | = 2 (fixed) |
| Differential | NAV-PVT | `flags.diffSoln` | = true |
| # Satellites | NAV-PVT | `numSV` | ≥ 6 |
| Horizontal Accuracy | NAV-HPPOSLLH | `hAcc` | < 100mm |
| Vertical Accuracy | NAV-HPPOSLLH | `vAcc` | < 150mm |
| PDOP | NAV-DOP | `pDOP` | < 3.0 |
| Full Covariance | NAV-COV | 3x3 matrix | Valid |

#### RTK/Correction Quality

| Metric | UBX Source | Field(s) | Threshold Example |
|--------|------------|----------|-------------------|
| Correction Age | NAV-PVT (proto27+) | `lastCorrectionAge` | < 10s |
| RTCM Reception | RXM-RTCM | `msgUsed` | = true |
| RTCM CRC | RXM-RTCM | `crcFailed` | = false |

#### Jamming & Spoofing Detection

| Metric | UBX Source | Field(s) | Threshold Example |
|--------|------------|----------|-------------------|
| Jamming State | SEC-SIG | `jammingState` | ≠ CRITICAL (3) |
| Jamming Indicator | MON-HW | `jamInd` | < 200 |
| Spoofing State | SEC-SIG | `spoofingState` | = OK (1) |
| RF Jamming | MON-RF | `jammingState` | ≠ CRITICAL |

#### Hardware Health

| Metric | UBX Source | Field(s) | Threshold Example |
|--------|------------|----------|-------------------|
| Antenna Status | MON-HW | `aStatus` | = OK |
| Antenna Power | MON-HW | `aPower` | = ON |

#### Advanced ZED-F9P Leading Indicators

| Metric | UBX Source | Mechanism | Safety Action |
|--------|------------|-----------|---------------|
| **AGC Monitoring** | MON-HW | Monitor `agcCnt` (Automatic Gain Control). Significant drop (>20%) implies high-power RF interference. | **WARNING**: Pre-jamming detected. |
| **Noise Level** | MON-HW | Monitor `noisePerMS`. Rising noise floor precedes signal loss. | **WARNING**: RF environment degrading. |
| **Frequency Diversity** | NAV-SIG | Verify PVT uses at least 2 bands (L1 + L2/L5). | **DEGRADE**: If Single Frequency, treat as low integrity. |
| **Config Integrity** | CFG-VALGET | Periodically poll key config (e.g. DynModel). Mismatch implies silent reset. | **E-STOP**: Critical configuration fault. |
| **Frozen Receiver** | NAV-PVT | Detect if `iTOW` changes but `lat/lon` is identical >1s while moving. | **E-STOP**: Receiver output frozen. |
| **Correction Heartbeat** | NAV-PVT | Monitor `ageC` (Age of Corrections). | **WARNING**: If >2.0s, accuracy degrades exponentially. |

### 4.2 Fallback Strategies

#### 4.2.1 Fallback Logic
If the primary solution is flagged invalid or exceeds integrity bounds:
1.  **Alert** the vehicle supervisor to degrade autonomy level.
2.  **Publish** degraded integrity status via `/gnss/integrity` topic.

### 4.3 Safety Monitor Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          GNSS SAFETY MONITOR                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│          ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │
│          │  Position   │   │    RTK      │   │  Security   │                │
│          │  Quality    │   │  Quality    │   │  Monitor    │                │
│          └──────┬──────┘   └──────┬──────┘   └──────┬──────┘                │
│                 │                 │                 │                       │
│                 └────────────┬────┴────────┬────────┘                       │
│                              │             │                                │
│                              ▼             ▼                                │
│              ┌───────────────────────────────────────────┐                  │
│              │         INTEGRITY AGGREGATOR              │                  │
│              │                                           │                  │
│              │  • Combines all quality metrics           │                  │
│              │  • Applies safety thresholds              │                  │
│              │  • Determines operational level           │                  │
│              │  • Logs events for audit trail            │                  │
│              └───────────────────┬───────────────────────┘                  │
│                                  │                                          │
│                                  ▼                                          │
│              ┌───────────────────────────────────────────┐                  │
│              │         OUTPUT                            │                  │
│              │                                           │                  │
│              │  /gnss/integrity    → GnssIntegrity msg   │                  │
│              │  /diagnostics       → DiagnosticArray     │                  │
│              │  /gnss/operational  → Bool (go/no-go)     │                  │
│              └───────────────────────────────────────────┘                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.4 Recommended Integrity Check Levels

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LEVEL 0: CRITICAL (Operation MUST stop if failed)                          │
├─────────────────────────────────────────────────────────────────────────────┤
│  • fixType ≥ 3 (3D fix)                                                     │
│  • gnssFixOK = true                                                         │
│  • numSV ≥ 4                                                                │
│  • jammingState ≠ CRITICAL                                                  │
│  • spoofingState ≠ MULTIPLE_SPOOFERS                                        │
│  • antennaStatus = OK                                                       │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  LEVEL 1: HIGH (Degraded operation, reduced speed/capability)               │
├─────────────────────────────────────────────────────────────────────────────┤
│  • carrSoln = FIXED (2) [for RTK applications]                              │
│  • hAcc < configured threshold                                              │
│  • pDOP < 3.0                                                               │
│  • numSV ≥ 6 (for redundancy)                                               │
│  • correctionAge < 10s [for RTK applications]                               │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  LEVEL 2: MONITOR (Log and alert, no operational impact)                    │
├─────────────────────────────────────────────────────────────────────────────┤
│  • spoofingState = WARNING                                                  │
│  • jammingState = WARNING                                                   │
│  • CNO degradation                                                          │
│  • Large pseudorange residuals                                              │
│  • SEC-SIGLOG events                                                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.5 Relevant Standards

| Standard | Domain | Key Requirements |
|----------|--------|------------------|
| **ISO 25119** | Agricultural machinery | Functional safety for tractors/machinery |
| **ISO 18497** | Highly automated ag machines | Safety for autonomous agricultural equipment |
| **ISO 26262** | Automotive | Functional safety (ASIL levels) |
| **DO-229E** | Aviation GNSS | Protection levels, integrity concepts |
| **NIST IR 8323** | GNSS security | Jamming/spoofing mitigation guidelines |
| **IEC 61508** | Industrial | Generic functional safety |

---

## 5. oxide_gnss Strategic Direction

### 5.1 Vision

A high-reliability GNSS driver for safety-critical autonomous vehicles with:
- **Deep Optimization:** Tailored specifically for u-blox ZED-F9P.
- **Safety by Default:** Built-in integrity monitoring and cross-checking.
- **Standard ROS2 Interface:** Compatible with robot_localization and Nav2.

### 5.2 Design Principles

1. **Standard ROS Types First** - Use `sensor_msgs`, `geometry_msgs`, `nav_msgs`.
2. **Minimal Custom Messages** - Only for integrity data.
3. **ZED-F9P Specificity** - Leverage hardware-specific safety features.
4. **Audit Trail** - Log all safety-relevant events.

### 5.3 Proposed Topic Structure

#### Core Topics (Standard ROS Types)

| Topic | Message Type | Source | Notes |
|-------|--------------|--------|-------|
| `~/fix` | `sensor_msgs/NavSatFix` | Position solution | Primary position output |
| `~/velocity` | `geometry_msgs/TwistWithCovarianceStamped` | Velocity solution | Body-frame velocity |
| `~/time` | `sensor_msgs/TimeReference` | Time solution | GNSS time reference |
| `~/odometry` | `nav_msgs/Odometry` | Computed | Local frame (optional) |
| `/diagnostics` | `diagnostic_msgs/DiagnosticArray` | Aggregated | Standard ROS diagnostics |

#### Safety/Integrity Topics (Custom)

| Topic | Message Type | Content |
|-------|--------------|---------|
| `~/integrity` | `oxide_gnss_msgs/GnssIntegrity` | All integrity metrics |
| `~/operational` | `std_msgs/Bool` | Go/no-go signal |

### 5.4 Proposed GnssIntegrity Message

```
# oxide_gnss_msgs/msg/GnssIntegrity

std_msgs/Header header

# Overall status
uint8 level                    # 0=OK, 1=DEGRADED, 2=CRITICAL, 3=FAILED
string status_message          # Human-readable status

# Position quality
uint8 fix_type                 # 0=none, 1=dead-reck, 2=2D, 3=3D, 4=GNSS+DR, 5=time-only
uint8 carrier_solution         # 0=none, 1=float, 2=fixed
bool differential_applied
uint8 num_satellites
float32 h_accuracy_m           # Horizontal accuracy (m)
float32 v_accuracy_m           # Vertical accuracy (m)
float32 pdop
float32[9] position_covariance # ENU covariance matrix (row-major)

# RTK status
float32 correction_age_s       # -1 if no RTK
bool rtcm_received
bool rtcm_used

# Security
uint8 jamming_state            # 0=unknown, 1=ok, 2=warning, 3=critical
uint8 spoofing_state           # 0=unknown, 1=ok, 2=single, 3=multiple
uint8 jamming_indicator        # 0-255 scale

# Hardware
uint8 antenna_status           # 0=unknown, 1=ok, 2=open, 3=short
```

### 5.5 Implementation Priorities

#### Phase 1: Core Safety Framework ✅ COMPLETED
- [x] Define `GnssIntegrity` message (`src/state/integrity.rs`)
- [x] Implement integrity aggregation (`IntegrityAggregator`)
- [x] Add configurable safety thresholds (`IntegrityThresholds`)
- [x] Publish `~/integrity` and `~/operational` topics

#### Phase 2: u-blox Completeness ✅ COMPLETED
- [x] Add NAV-COV parsing (full covariance)
- [x] Add NAV-POSECEF parsing (ECEF position)
- [x] Add SEC-SIG parsing (jamming/spoofing)
- [x] Add SEC-SIGLOG parsing (security event log)
- [x] Add RXM-COR parsing (correction status)
- [x] Add MON-COMMS parsing (communication port status)
- [x] Add MON-HW parsing (antenna status, jamming indicator)

---

## References

- [ublox_dgnss GitHub](https://github.com/aussierobots/ublox_dgnss)
- [ublox-rs GitHub](https://github.com/ublox-rs/ublox)
- [u-blox F9 Interface Description](https://www.u-blox.com/en/docs/UBX-18010854)
- [robot_localization](http://docs.ros.org/en/noetic/api/robot_localization/html/)
- [ISO 25119 Agricultural Machinery Safety](https://www.iso.org/standard/61120.html)
- [NIST IR 8323 GNSS Security](https://csrc.nist.gov/publications/detail/nistir/8323/final)
