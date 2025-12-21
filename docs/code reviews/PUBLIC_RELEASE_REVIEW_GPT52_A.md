# Pre-release Review: oxide_gnss

Date: 2025-12-19

This document captures a pre-open-source critique of the `oxide_gnss` repository: architecture, correctness, reliability, ROS2 conventions, security posture, documentation/CI readiness, and maintainability.

The intent is constructive: highlight what’s strong, what’s likely to bite you once the repo is public, and provide actionable improvements.

---

## Executive Summary (What’s great / what to fix first)

### Strengths

- **Mode + feature configuration** is a big usability win and is clearly documented.
  - See `config/mod.rs::Config::resolve_ublox_config`, `config/modes.rs` (mode presets), and `config/ublox.rs::UbloxConfig::validate`.
- **Task-based architecture** is clean and easy to reason about.
  - Node entry + orchestration: `main.rs::main`.
  - Device: `device/task.rs::DeviceTask`.
  - NTRIP: `ntrip/task.rs::NtripTask`.
  - ROS publishers: `ros/task.rs::RosTask`, `ros/publishers.rs::GnssPublishers`.
- **Integrity aggregation concept** is pragmatic for autonomy consumers.
  - See `state/integrity.rs::IntegrityAggregator` and ROS publication in `ros/publishers.rs::GnssPublishers::publish_integrity`.

### Must-fix before public visibility (Correctness / trust)

1. **UBX ACK/NAK correlation is not validated**
   - `device/config.rs::DeviceConfigurator::wait_for_ack` currently accepts the first `ACK-ACK`/`ACK-NAK` observed by `device/ubx.rs::UbxHandler::process`.
   - `device/ubx.rs::UbxHandler` has `pending_ack` state (`expect_ack`, `clear_pending_ack`) but does not use it to filter ACKs.
   - Risk: mis-attributing an ACK to the wrong command can produce “config succeeded” while the receiver is misconfigured.

2. **Integrity operational signal lacks freshness/staleness handling**
   - `state/integrity.rs::IntegrityAggregator::compute` does not incorporate data age for PVT/SEC/MON/RXM inputs.
   - Risk: `~/operational` may remain `true` with stale integrity sources.

3. **Config surface includes options that are not implemented / are misleading**
   - Example: `use_https` exists in config docs but `ntrip/client.rs::NtripClient::connect` uses raw `TcpStream` and does not implement TLS.

---

## Repo Model (Entry points and data flow)

### Runtime topology

- **Node entry**: `main.rs::main`
  - Loads YAML config via `config/mod.rs::Config::from_file`.
  - Resolves mode/feature-driven UBX config via `Config::resolve_ublox_config`.
  - Creates ROS node + publishers via `ros/node.rs::GnssNode::new` and `ros/publishers.rs::GnssPublishers::new`.
  - Spawns tasks:
    - Device: `device/task.rs::spawn_device_task`
    - NTRIP: `ntrip/task.rs::spawn_ntrip_task`
    - ROS publishing: `ros/task.rs::spawn_ros_task`

### Message flow

- Device raw bytes → UBX parser:
  - Serial reads in `device/task.rs::DeviceTask::run_active_loop`
  - Parsing in `device/ubx.rs::UbxHandler::process`
- Device parsed output → message routing:
  - Device emits `device/task.rs::DeviceMessage`.
  - `main.rs::main` forwards select messages to supervisor via `DeviceMessage::into_gnss_message`.
- ROS topic publishing:
  - `ros/task.rs::RosTask::handle_message` routes to `ros/publishers.rs::GnssPublishers::*`.

---

## Architecture & Maintainability Review

### 1) “Supervisor” responsibility is split across `Supervisor` and `main.rs`

- `state/supervisor.rs::Supervisor` holds channels and shared state.
- However, cross-task routing is implemented in `main.rs::main` (two forwarding loops from device/NTRIP message channels into supervisor channels).

**Impact**

- New contributors will expect “the supervisor coordinates tasks” but coordination logic lives partly in `main.rs`.

**Recommendation**

- Either move message forwarding into `Supervisor` (e.g., helper methods or a `spawn_forwarders(...)` function), or simplify the story and call `Supervisor` “shared state + channel bundle”.

---

## Device + Serial Review

### 2) Serial reconnection logic exists but is not used effectively

- `device/serial.rs::SerialPort::try_reconnect` implements backoff reconnection.
- `device/task.rs::DeviceTask::run_active_loop` exits on any `serial.read(...)` error; reconnection is performed by restarting the device state machine via `DeviceTask::connect`.

**Impact**

- The presence of `SerialPort::try_reconnect` implies in-place reconnection, but the runtime behavior is “tear down and restart state machine”.

**Recommendation (choose one, then align code + docs)**

- **Option A (simplest)**: remove/trim `try_reconnect` and make `DeviceTask` the authoritative reconnection policy.
- **Option B (more robust)**: keep `DeviceTask` in Active state and attempt `SerialPort::try_reconnect()` inside the active loop rather than returning an error immediately.

### 3) RTCM injection path is simple and good

- NTRIP RTCM bytes are forwarded to device in `device/task.rs::DeviceTask::inject_rtcm`.
- This is straightforward and avoids additional buffering complexity.

---

## UBX Parsing + Configuration Review

### 4) ACK validation is currently incomplete (must-fix)

Relevant implementation:

- Waiting for ACK:
  - `device/config.rs::DeviceConfigurator::wait_for_ack`
- Parsing ACK/NAK:
  - `device/ubx.rs::UbxHandler::process` matches `PacketRef::AckAck` and `PacketRef::AckNak`
- Intended but unused correlation:
  - `device/ubx.rs::UbxHandler::{expect_ack, clear_pending_ack}` and `PendingAck`

**What’s wrong**

- The config step currently accepts the first ACK/NAK regardless of whether it matches the command sent.

**Recommendation**

- When sending a command, set the expected class/id (e.g., for CFG-VALSET).
- In `UbxHandler::process`, only emit `ProcessResult.ack` when ACK/NAK matches `pending_ack`.
- Consider clearing/draining parser/serial buffer at the start of configuration to reduce the chance of stale ACKs.

---

## Integrity / “Operational” Signal Review

### 5) Integrity aggregation is clear but missing staleness semantics (must-fix if you keep the safety framing)

Relevant implementation:

- `state/integrity.rs::IntegrityAggregator::{update_pvt, update_sec_sig, update_mon_rf, update_mon_comms, update_rxm_cor, compute}`
- Publication and rate limiting:
  - `device/task.rs::DeviceTask::process_serial_data` sends `DeviceMessage::Integrity` when updated
  - `ros/task.rs::RosTask::publish_integrity`
  - `ros/publishers.rs::GnssPublishers::publish_integrity`

**Gap**

- No timeouts / freshness checks per source message.
- No defined behavior for when a feature-required message is missing or stops arriving.

**Recommendation**

- Track last update time for key sources:
  - PVT is essential (must always be fresh).
  - SEC-SIG / MON-RF / MON-COMMS / RXM-COR should be fresh when integrity feature is enabled.
- Add “stale → FAILED (or at least DEGRADED)” semantics.
- Align docs to actual logic (your docs explicitly state FAILED can be “data stale/unavailable”).

---

## ROS2 Interface Review

### 6) Timestamp strategy mixes wall-clock and GNSS time

Relevant implementation:

- Wall-clock stamp helper: `ros/conversions.rs::now_timestamp`
- GNSS time in TimeReference: `ros/conversions.rs` impl `ToRosMessage<sensor_msgs::msg::TimeReference> for PvtData`

**Impact**

- NavSatFix / Twist messages are typically stamped with “time received” while TimeReference is GNSS UTC.
- That is acceptable, but should be explicitly stated for consumers who care about time alignment.

**Recommendation**

- Document the chosen policy (“stamp with reception time; GNSS time provided separately”).
- Avoid silently producing Unix epoch on invalid time; consider a diagnostics warning or an explicit “invalid time” path.

### 7) NavSatStatus mapping for RTK float/fixed is a hack (document it)

Relevant implementation:

- `ros/conversions.rs::fix_type_to_status`

**Impact**

- Mapping RTK float/fixed to SBAS/GBAS status is common but semantically inaccurate.

**Recommendation**

- Document this explicitly.
- Alternatively, set `STATUS_FIX` for all non-NoFix and expose RTK state via diagnostics/custom message.

### 8) PDOP scaling is potentially confusing

Relevant implementation:

- Stored as `PvtData.p_dop` in `device/ubx.rs::UbxHandler::parse_nav_pvt`
- Used in diagnostics as `p.p_dop / 100.0` in `ros/task.rs::RosTask::publish_diagnostics`

**Recommendation**

- Store PDOP in real units as `f32` if possible, or rename the field to indicate scaling (e.g., `pdop_x100`).

---

## NTRIP Review

### 9) `use_https` is documented but not implemented (must resolve)

Relevant implementation:

- `ntrip/client.rs::NtripClient::connect` uses `TcpStream`.

**Recommendation**

- Either implement TLS (tokio-rustls / native-tls) or document that `use_https` is currently unsupported and remove it from the config surface until it is.

### 10) Request headers and protocol notes

Relevant implementation:

- `ntrip/client.rs::NtripClient::connect` builds HTTP/1.0 request and includes `Connection: close`.

**Recommendation**

- Consider changing to `Connection: keep-alive` or omit the header to better reflect long streaming semantics.
- If you don’t implement chunked decoding, remove the claim from the module docs to avoid misleading readers.

### 11) GGA generation is good and test-covered

Relevant implementation:

- `ntrip/gga.rs::GgaSentence::{to_nmea, format_latitude, format_longitude}`

Note: time-of-day for GGA uses `Utc::now()` rather than GNSS time from PVT; that’s usually acceptable for caster selection but should be documented.

---

## CI / Open-source Readiness

### 12) CI is solid but under-documented

Relevant implementation:

- Main CI: `.github/workflows/ci.yml`
  - Runs `cargo fmt`.
  - Uses container image `ghcr.io/gsokoll/oxide_gnss-ci:jazzy` to run colcon build + clippy + tests.
- CI image builder: `.github/workflows/docker-ci-image.yml`
- CI image Dockerfile: `.github/docker/Dockerfile.ci`

**Documentation gap**

- `docs/DEVELOPMENT.md` covers local pre-commit checks but does not describe:
  - what CI runs,
  - why CI uses a container,
  - how to rebuild/update that container.

**Recommendation**

- Add a short “CI” section to `docs/DEVELOPMENT.md` explaining the above and how to trigger the Docker image workflow.

### 13) Reproducibility risk: unpinned git dependencies

Relevant implementation:

- `oxide_gnss/Cargo.toml` uses `ublox = { git = "https://github.com/ublox-rs/ublox.git" ... }` and also patches crates.io to `branch = "master"`.

**Impact**

- CI and user builds can break when upstream changes.

**Recommendation**

- Pin to a specific revision or a tagged release.

---

## Documentation Hygiene

### 14) Two READMEs may confuse users

- Top-level `README.md` and `oxide_gnss/README.md` are similar but not identical.

**Recommendation**

- Make one canonical README and keep the other as a short pointer.

---

## Suggested Release Roadmap

### Phase 0 (before going public)

- Implement ACK correlation (CFG-VALSET ACK/NAK match).
- Add integrity freshness/staleness.
- Resolve `use_https` mismatch (implement or remove).
- Add a short CI documentation section.

### Phase 1 (first public release)

- Clarify time stamping policy (ROS time vs GNSS time).
- Clarify RTK status mapping.
- Pin `ublox` dependency.

### Phase 2 (polish / extensibility)

- Consider typed satellite status message instead of JSON string.
- Consider a cleaner “supervisor owns routing” abstraction.

---

## Quick Reference: Code Locations Mentioned

- Entry point / orchestration:
  - `main.rs::main`
- Config:
  - `config/mod.rs::{Config::from_file, Config::resolve_ublox_config, Config::enabled_topics}`
  - `config/ublox.rs::{UbloxConfig::validate, UbloxConfig::check_topic_availability}`
- Device:
  - `device/task.rs::{DeviceTask::run, DeviceTask::run_state_machine, DeviceTask::run_active_loop, DeviceTask::process_serial_data}`
  - `device/serial.rs::{SerialPort::read, SerialPort::write, SerialPort::try_reconnect}`
  - `device/config.rs::{DeviceConfigurator::configure, DeviceConfigurator::configure_step, DeviceConfigurator::wait_for_ack}`
  - `device/ubx.rs::{UbxHandler::process, UbxHandler::expect_ack, UbxHandler::clear_pending_ack}`
- Integrity:
  - `state/integrity.rs::{IntegrityAggregator::update_*, IntegrityAggregator::compute}`
- ROS publishing:
  - `ros/task.rs::{RosTask::run, RosTask::handle_message, RosTask::publish_diagnostics, RosTask::publish_integrity}`
  - `ros/publishers.rs::{GnssPublishers::new, GnssPublishers::publish_pvt, GnssPublishers::publish_diagnostics, GnssPublishers::publish_integrity}`
  - `ros/conversions.rs::{now_timestamp, fix_type_to_status, ToRosMessage impls}`
- NTRIP:
  - `ntrip/task.rs::{NtripTask::run, NtripTask::stream_loop}`
  - `ntrip/client.rs::{NtripClient::connect, NtripClient::read_chunk, NtripClient::send_gga}`
  - `ntrip/gga.rs::{GgaSentence::to_nmea}`
- CI:
  - `.github/workflows/ci.yml`
  - `.github/workflows/docker-ci-image.yml`
  - `.github/docker/Dockerfile.ci`
