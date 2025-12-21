# Public Release Review: `oxide_gnss`

This document is a deep critique of the *concept*, *architecture*, and *implementation* of `oxide_gnss` with the assumption that you’re preparing to make the repository publicly visible.

It is intentionally candid and action-oriented. Where possible it references specific code elements by module/type/function so the feedback is easy to verify and apply.

---

## Executive Summary

**Overall**: The project is well-structured for an early-stage Rust + ROS 2 driver, with solid foundations (async task decomposition, config modeling, CI, a clear docs story). The “safety/integrity” direction is promising, but there are several **high-severity correctness and security gaps** that would likely surprise users and undermine trust if published as-is.

### Highest priority issues to address pre-public

- **Credential leakage risk**: `oxide_gnss::ntrip::client::NtripClient::connect` logs the full HTTP request at `debug` level, which includes a `Basic` Authorization header derived from the configured username/password.
- **Broken correction-age reporting (and “RTCM received” plumbing)**: `oxide_gnss::ntrip::task::NtripTask` emits `NtripMessage::RtcmReceived`, but `src/main.rs`’s NTRIP forwarder drops it, meaning `oxide_gnss::ros::task::RosTask` never receives `GnssMessage::RtcmReceived` and diagnostics never show correction age.
- **Integrity “FAILED” state is not actually computed**: `oxide_gnss::state::integrity::IntegrityLevel` includes `Failed`, and `GnssIntegrity::default()` starts at `Failed`, but `IntegrityAggregator::compute` never returns `Failed` (only `Ok`, `Degraded`, `Critical`). There is also no data staleness/timeout logic.
- **Docs / interface mismatches**:
  - The codebase references `~/hp_pos` in multiple places (`oxide_gnss::config::ublox::MESSAGE_REQUIREMENTS`), but the node does **not** publish a `~/hp_pos` topic.
  - `oxide_gnss_msgs::msg::SecSigDetails` exists and `oxide_gnss::ros::conversions::ToRosMessage` implements conversion, but **no publisher** exists and `oxide_gnss::ros::task::RosTask::handle_message` explicitly ignores `GnssMessage::SecSig`.
  - `oxide_gnss::ros::publishers::GnssPublishers::publish_sat_info` tries to compute “active sats” based on `SatStatus.flags`, but `oxide_gnss::device::ubx::UbxHandler::parse_nav_sat` sets `flags: 0` as a placeholder.
- **Configuration flow mismatch**: `oxide_gnss::device::config::ConfigStep` implies multiple steps, but `DeviceConfigurator::configure` effectively runs one “bulk CFG-VALSET” step while still presenting “step/total_steps” state.

If you publish publicly now, you should keep the strong **“Under Development / not production-ready”** message, but also consider explicitly stating the **above limitations** (or fixing them first) to prevent incorrect expectations.

---

## What’s Strong (Keep / Build On)

- **Clear module decomposition**
  - Config: `oxide_gnss::config`
  - Device I/O + UBX parsing: `oxide_gnss::device`
  - NTRIP: `oxide_gnss::ntrip`
  - ROS integration: `oxide_gnss::ros`
  - State/integrity: `oxide_gnss::state`

- **Async task separation is appropriate**
  - Device loop: `oxide_gnss::device::task::DeviceTask::run` / `run_active_loop`
  - NTRIP streaming loop: `oxide_gnss::ntrip::task::NtripTask::run` / `stream_loop`
  - ROS publishing loop: `oxide_gnss::ros::task::RosTask::run`

- **Mode + feature abstraction is a strong UX idea**
  - `oxide_gnss::config::Config::resolve_ublox_config`
  - `oxide_gnss::config::modes::{OperatingMode, Feature, ModePreset}`

- **CI strategy is pragmatic**
  - Docker-based ROS2 + ros2-rust image build: `.github/workflows/docker-ci-image.yml` + `.github/docker/Dockerfile.ci`
  - CI runs `fmt`, `clippy`, and tests: `.github/workflows/ci.yml`

---

## Public Release Blockers (High Severity)

### 1) Credential leakage via debug logging

- **Where**: `oxide_gnss::ntrip::client::NtripClient::connect`
  - It constructs an HTTP request with `Authorization: Basic <base64(user:pass)>`.
  - It logs the entire request via `debug!(request = ?request, ...)`.

**Why this matters**

- Base64 is not encryption; it is reversible.
- A user running with `RUST_LOG=debug` (common when troubleshooting NTRIP) will leak credentials to console logs.
- In ROS, logs often end up in bagfiles, shared logs, CI artifacts, etc.

**Recommendation**

- Redact sensitive headers before logging.
- Prefer logging only non-sensitive metadata (host, mountpoint, user-agent, connection state, status code).

### 2) `use_https` configuration is misleading / not implemented

- **Where**:
  - Config: `oxide_gnss::config::NtripConfig::use_https`
  - Client: `oxide_gnss::ntrip::client::NtripClient::connect` uses `tokio::net::TcpStream` and sends plaintext HTTP.

**Why this matters**

- A user might set `use_https: true` and assume transport encryption.
- If they point at port 443 without TLS, it will fail or behave unpredictably.

**Recommendation**

- Either implement TLS (e.g., `tokio-rustls`) or remove/disable `use_https` until supported.
- If you keep the field, fail validation in `NtripConfig::validate` when `use_https == true` with a clear error.

### 3) Correction age is effectively broken

- **Where**:
  - Producer: `oxide_gnss::ntrip::task::NtripTask::handle_rtcm_data` sends `NtripMessage::RtcmReceived { bytes }`.
  - Bridge: `src/main.rs` has an NTRIP forwarder loop that **only forwards** `NtripMessage::StateChanged` into `GnssMessage::NtripStateChanged`.
  - Consumer: `oxide_gnss::ros::task::RosTask::handle_message` expects `GnssMessage::RtcmReceived` to set `last_correction_received`, which is then used by `RosTask::publish_diagnostics`.

**Impact**

- `correction_age` in diagnostics is likely always absent.
- You also lose observability into RTCM throughput.

**Recommendation**

- Forward `NtripMessage::RtcmReceived` to `GnssMessage::RtcmReceived`.
- Consider also forwarding “connected/disconnected” events (currently `NtripMessage::Connected` / `Disconnected` exist but are not bridged).

### 4) Integrity “FAILED” is not defined operationally

- **Where**:
  - `oxide_gnss::state::integrity::IntegrityLevel` includes `Failed`.
  - `oxide_gnss::state::integrity::GnssIntegrity::default()` starts as `Failed`.
  - `oxide_gnss::state::integrity::IntegrityAggregator::compute` never emits `Failed`.

**Impact**

- Your README/docs describe a `FAILED` level (“System unavailable or data stale”), but the code does not implement the concept of *staleness*.

**Recommendation**

- Define *exactly* what constitutes `Failed`:
  - No PVT received for `T` seconds.
  - Integrity-relevant messages stale.
  - Device state not `Active`.
  - RTCM stream stale when operating mode requires RTK.
- Implement staleness timers using `PvtData.received_at` (already available in `oxide_gnss::device::ubx::PvtData`).

---

## Architecture & Safety Model Critique

### 1) The “Supervisor” is not currently a supervisor

- **Where**:
  - `oxide_gnss::state::Supervisor` provides shared state and channels.
  - `oxide_gnss::state::Supervisor::{set_device_state, set_ntrip_state, set_fix_type}` exist.
  - In `src/main.rs`, tasks publish `DeviceMessage::StateChanged` and `NtripMessage::StateChanged` which go to ROS publishing.
  - However, nothing appears to call `Supervisor::set_device_state` / `set_ntrip_state` / `set_fix_type` during runtime.

**Impact**

- The “Supervisor” is mostly a channel bundle, not a logic-bearing coordinator.
- You have *two* state systems:
  - A “shared supervisor state” that is unused.
  - A “ROS task local shadow state” (`oxide_gnss::ros::task::RosTask::{last_device_state,last_ntrip_state,last_fix_type}`) used for diagnostics.

**Recommendation**

- Decide what the authoritative state store is:
  - Option A: Make `Supervisor` authoritative: it consumes messages and maintains state; ROS task reads snapshots.
  - Option B: Delete shared state from `Supervisor` and treat it purely as a channel router.

### 2) Safety signals are published, but not linked to lifecycle enforcement

- You publish `~/operational` as a Bool computed in `oxide_gnss::ros::publishers::GnssPublishers::publish_integrity`.
- But the node does not enforce *any* behavior (e-stop, rate limiting, etc.). That’s fine (drivers often shouldn’t enforce), but **public documentation should clearly state** that `~/operational` is only a signal.

### 3) Cross-check / multi-receiver concept is not present here

Your current code implements single-receiver integrity aggregation (good), but there is no multi-device fusion or baseline cross-checking. If the project’s public positioning is “safety-focused,” consider clarifying:

- Whether this repo is a single-receiver driver building block.
- Or whether it’s aiming to be a full dual-receiver safety GNSS solution.

If the latter, you’ll need architectural additions (multi-device config, synchronization, baseline checks, etc.).

---

## Correctness & ROS Interface Issues

### 1) Topic/interface mismatches

- **`~/hp_pos` topic**
  - `oxide_gnss::config::ublox::MESSAGE_REQUIREMENTS` lists `NAV_HPPOSLLH` and associates it with `~/hp_pos`.
  - `oxide_gnss::ros::publishers::GnssPublishers` does not create/publish a `~/hp_pos` topic.
  - Your top-level README claims HP-enhanced `~/fix` (which is true in `GnssPublishers::publish_pvt` when `use_hp_for_fix` is enabled).

**Recommendation**

- Either:
  - Publish a real `~/hp_pos` topic (and document it), or
  - Remove `~/hp_pos` references from requirements/docs and treat HP strictly as an enhancer.

- **`oxide_gnss_msgs::msg::SecSigDetails` exists but is unused**
  - Conversion exists: `oxide_gnss::ros::conversions::ToRosMessage<oxide_gnss_msgs::msg::SecSigDetails> for SecSigData`.
  - Publishing does not exist, and `RosTask::handle_message` ignores `GnssMessage::SecSig`.

**Recommendation**

- Either publish a `~/sec_sig_details` topic (or similar), or remove the message type + conversion to avoid dead weight.

### 2) Satellite info is currently misleading

- `oxide_gnss::device::ubx::UbxHandler::parse_nav_sat` sets `SatStatus.flags = 0` as a placeholder.
- `oxide_gnss::ros::publishers::GnssPublishers::publish_sat_info` tries to compute active satellites from `flags`.

**Impact**

- `active_svs` will likely always be `0`.

**Recommendation**

- Either populate flags properly from the ublox crate’s per-sv flags, or publish a simpler/accurate representation (or omit active_svs).

### 3) ROS time and timestamps

- `oxide_gnss::ros::conversions::now_timestamp` uses `SystemTime::now()`.

**Impact**

- It won’t align with ROS time / simulated time when `use_sim_time` is enabled.

**Recommendation**

- Use ROS time from the node clock if available in `rclrs` for message stamps.

### 4) Fix type -> NavSatStatus mapping is non-standard

- `oxide_gnss::ros::conversions::fix_type_to_status` maps:
  - `FixType::RtkFloat` => `STATUS_SBAS_FIX`
  - `FixType::RtkFixed` => `STATUS_GBAS_FIX`

**Impact**

- Tools may interpret SBAS/GBAS literally.

**Recommendation**

- Consider leaving `STATUS_FIX` and encode RTK state in diagnostics or a custom message.

---

## Configuration & UX Review

### 1) Environment substitution behavior can be surprising

- `oxide_gnss::config::Config::substitute_env_vars` replaces `${VAR}` if present, otherwise leaves `${VAR}` unchanged.

**Impact**

- If a user forgets to export `NTRIP_PASSWORD`, the literal string `${NTRIP_PASSWORD}` becomes the password, causing confusing auth failures.

**Recommendation**

- Add an option to treat unresolved variables as errors (at least for `ntrip.username/password`).

### 2) Device configuration “steps” don’t match the implementation

- `oxide_gnss::device::config::ConfigStep` implies:
  - Navigation rate
  - Enable NAV-PVT
  - Protocol settings
- But `oxide_gnss::device::config::DeviceConfigurator::configure` sends *all* CFG-VAL values in one call (`configure_step(..., ConfigStep::EnableNavPvt, &cfg_vals)`), and `ConfigStep::total()` returns `3` despite the enum having a `Complete` variant.

**Recommendation**

- Either rework the state machine to reflect actual steps, or simplify state reporting.

---

## NTRIP Protocol/Robustness Notes

- `oxide_gnss::ntrip::client::NtripClient::connect` uses a “manual HTTP/1.0 GET” which is common for NTRIP v1.
- However, the request includes `Connection: close` even though the stream is intended to remain open. Many servers tolerate this, but it’s semantically odd.

**Recommendation**

- Consider `Connection: keep-alive` (or omit the header for NTRIP v1 style).
- Add explicit parsing/handling for caster behaviors (sourcetable, redirects, chunked encoding) if you want to claim broad caster support.

---

## Testing & CI Readiness

What you have is a good start:

- Unit tests exist throughout modules.
- CI enforces formatting and clippy.

What’s missing for public trust:

- **Integration tests** (even minimal) for:
  - Config loading + env substitution edge cases.
  - NTRIP parsing against captured header samples.
  - UBX parsing against known binary fixtures.

- **Hardware-in-the-loop strategy**
  - Even if you can’t do real hardware in CI, you can capture UBX logs and replay them into `UbxHandler::process`.

---

## Packaging / Public Repo Hygiene

- `oxide_gnss/package.xml` uses a placeholder maintainer email (`gsokoll@example.com`). This will look suspicious in public. Update it.
- Add the standard public repo docs:
  - `CONTRIBUTING.md`
  - `CODE_OF_CONDUCT.md`
  - `SECURITY.md` (especially since you handle credentials)
  - `CHANGELOG.md` or at least a “Release notes” section.

---

## Suggested Roadmap (Prioritized)

### P0 (Must fix before public visibility)

- Remove/redact credential-bearing logs:
  - `oxide_gnss::ntrip::client::NtripClient::connect`
- Fix RTCM received forwarding so correction age works:
  - `src/main.rs` NTRIP forwarder should forward `NtripMessage::RtcmReceived`.
- Clarify or enforce `use_https` (remove or implement).
- Fix docs/topic mismatches (`~/hp_pos`, `SecSigDetails`, satellite flags).

### P1 (Strongly recommended)

- Implement integrity staleness/timeout logic (`IntegrityLevel::Failed`).
- Decide and document what the “Supervisor” is responsible for.
- Add at least one UBX fixture/replay test.

### P2 (Polish)

- Improve ROS timestamping using ROS time.
- Improve `NavSatStatus` mapping semantics.
- Consider more structured satellite and security publishing rather than JSON-in-String.

---

## Closing

The repo is close to being a *great public learning resource* for Rust-based ROS 2 drivers, and a credible foundation for a safety-focused GNSS stack. The biggest risk for public release is not code style—it’s **incorrect/incomplete safety semantics and inadvertent credential exposure**.

If you want, I can follow up with a small PR-style set of changes that addresses the P0 items with minimal disruption.
