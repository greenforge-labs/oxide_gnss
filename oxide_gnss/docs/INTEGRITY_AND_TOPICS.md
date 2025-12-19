# Integrity & Topics Reference

This document describes:

- The ROS2 topics published by `oxide_gnss`
- Which u-blox UBX messages are required to support those topics
- How the integrity/operational signals are computed at a high level

This is intended to be a **public-facing** reference that stays aligned with the implementation.

---

## Published Topics

### Core topics (always published)

- `~/fix` (`sensor_msgs/NavSatFix`)
- `~/velocity` (`geometry_msgs/TwistWithCovarianceStamped`)
- `~/time_reference` (`sensor_msgs/TimeReference`)
- `/diagnostics` (`diagnostic_msgs/DiagnosticArray`)

Implementation:

- Topic creation: `ros/publishers.rs::GnssPublishers::new`
- Publishing: `ros/publishers.rs::GnssPublishers::{publish_pvt, publish_diagnostics}`

### Optional topics (mode/features)

- `~/integrity` (`oxide_gnss_msgs/OxideIntegrity`) — requires `features.integrity: true`
- `~/operational` (`std_msgs/Bool`) — requires `features.integrity: true`
- `~/satellites` (`std_msgs/String`) — requires `features.satellites: true`
- `~/baseline_pose` (`geometry_msgs/PoseWithCovarianceStamped`) — requires `mode: moving_base_rover`

Implementation:

- Topic creation: `ros/publishers.rs::GnssPublishers::new`
- Publishing:
  - Integrity: `ros/publishers.rs::GnssPublishers::publish_integrity`
  - Satellites: `ros/publishers.rs::GnssPublishers::publish_sat_info`
  - Baseline pose: `ros/publishers.rs::GnssPublishers::publish_baseline_pose`

---

## UBX Message Requirements (by topic)

The driver validates your UBX message configuration at startup.

Implementation:

- Message requirement table + validation: `config/ublox.rs` (`MESSAGE_REQUIREMENTS`, `UbloxConfig::validate`, `UbloxConfig::check_topic_availability`)

### `~/fix`, `~/velocity`, `~/time_reference`

- Required:
  - `NAV_PVT`

Parsed in:

- `device/ubx.rs::UbxHandler::process` → `UbxHandler::parse_nav_pvt`

### `~/integrity`, `~/operational`

- Required:
  - `NAV_PVT`
  - `SEC_SIG`
  - `MON_RF`
  - `MON_COMMS`

Optional inputs that improve integrity observability:

- `SEC_SIGLOG`
- `RXM_COR`
- `NAV_COV` (note: not available on all F9 variants)

Parsed in:

- `device/ubx.rs::UbxHandler::process` →
  - `UbxHandler::parse_sec_sig`
  - `UbxHandler::parse_mon_rf`
  - `UbxHandler::parse_mon_comms`
  - `UbxHandler::parse_rxm_cor`
  - `UbxHandler::parse_nav_cov`
  - `UbxHandler::parse_sec_siglog`

### `~/satellites`

- Required:
  - `NAV_SAT`

Parsed in:

- `device/ubx.rs::UbxHandler::process` → `UbxHandler::parse_nav_sat`

### `~/baseline_pose`

- Required:
  - `NAV_RELPOSNED`

Parsed in:

- `device/ubx.rs::UbxHandler::process` → `UbxHandler::parse_nav_rel_pos_ned`

---

## Integrity Model (high level)

The integrity system aggregates multiple receiver signals into a single integrity assessment.

Implementation:

- Aggregation: `state/integrity.rs::IntegrityAggregator`
- Updates fed by device task: `device/task.rs::DeviceTask::process_serial_data`

### Integrity levels

- `OK`
- `DEGRADED`
- `CRITICAL`
- `FAILED`

Implementation:

- Enum: `state/integrity.rs::IntegrityLevel`

### What drives the integrity level

- **Fix availability/type**
  - Derived from NAV-PVT in `device/ubx.rs::UbxHandler::parse_nav_pvt` and fed into `IntegrityAggregator::update_pvt`.
- **Satellite count / PDOP / accuracy**
  - From NAV-PVT and fed into `IntegrityAggregator::update_pvt`.
- **Jamming/spoofing**
  - From SEC-SIG and fed into `IntegrityAggregator::update_sec_sig`.
- **Antenna status**
  - From MON-RF (preferred) and fed into `IntegrityAggregator::update_mon_rf`.
- **Comms health**
  - From MON-COMMS and fed into `IntegrityAggregator::update_mon_comms`.

### Operational signal

`~/operational` is derived from integrity.

Implementation:

- `state/integrity.rs::IntegrityAggregator::is_operational`
- Published from `ros/publishers.rs::GnssPublishers::publish_integrity`

---

## Notes on intended semantics

- This integrity system is intended as **quality monitoring and gating** for autonomy stacks. It is not a certified safety monitor.
- If you enable `features.integrity`, you should ensure the required UBX messages are enabled. Startup validation will warn/error when configuration is incomplete.
