# Issue Resolution Plan (`oxide_gnss`)

Date: 2025-12-20  
Author: Cascade

## Purpose

This document turns the findings from `DEFINITIVE_RELEASE_REVIEW.md` into an actionable implementation plan (code + docs + release hygiene), with explicit sequencing and validation steps.

## Inputs / References

- `docs/DEFINITIVE_RELEASE_REVIEW.md`
- Prior review set:
  - `docs/PUBLIC_RELEASE_REVIEW_*.md`
  - `docs/PUBLIC_RELEASE_REVIEW_SYNTHESIS.md`

## Maintainer decisions already made (assumptions)

- **HP position topic**: No separate `~/hp_pos` topic; HP data enhances `~/fix` when available/enabled.
- **Integrity intent**: Integrity is intended to evolve toward **stronger safety-oriented semantics**.
- **Correction age policy**: Host-side **time since last RTCM received**.
- **NTRIP dependency source**: `oxide_gnss` should consume `ntrip-core` via crates.io (published crate), not via git/path.
- **Dependency preference**: Prefer crates.io releases over git dependencies where possible (including `ublox`).
- **Operational semantics (current)**: `IntegrityLevel::Degraded` implies `~/operational = false` (implement this via a central policy/mapping to keep future changes localized).
- **Satellites topic**: Output schema changes are allowed.
- **`SecSigDetails`**: Keep internal/not published for now; document as such.
- **OSS governance docs**: Adopt standard docs/templates, but keep them pragmatic/minimal.
- **README strategy**: Root README is canonical. A separate crate/package README should be removed or reduced to a stub pointing to the root to avoid divergence.

## Guiding principles

- Treat P0 items as release blockers for “safety-oriented integrity” positioning.
- Prefer deterministic builds (pin dependencies, document update process).
- Prefer crates.io dependencies over git dependencies; avoid floating branches.
- Prefer monotonic time (`Instant`) for internal staleness/correction-age logic; translate to ROS timestamps at publish boundaries.
- Make behavior match docs, or update docs to match behavior.

## Milestones (proposed)

### Milestone P0: Release blockers (correctness + reproducibility)

#### P0.1 Pin `ublox` dependency (build reproducibility / supply-chain)

- **Goal**: Remove `branch = "master"` dependency and prefer crates.io for reproducible builds.
- **Implementation**:
  - First choice (preferred): switch to a crates.io `ublox` release that contains the required message support.
  - Remove the `[patch.crates-io] ublox = { git = ..., branch = "master" }` override once a suitable crates.io release is available.
  - If crates.io is missing required support, use a *temporary* pinned git `rev = "<commit>"` (not a branch) while upstreaming changes and/or waiting for a new crates.io release.
  - Add a short note to docs (or README) describing the chosen source and how/when the dependency is updated.
- **Validation**:
  - CI builds clean from scratch.
  - `cargo build --locked` is stable across time.

#### P0.2 Implement UBX ACK/NAK correlation (device configuration correctness)

- **Goal**: Ensure that configuration success/failure corresponds to the specific command sent.
- **Implementation approach (recommended)**:
  - When sending each UBX config command, set an “expected ACK” (class + msg_id) and only accept `ACK-ACK`/`ACK-NAK` matching that expectation.
  - Prefer to handle this in `UbxHandler` so ACK filtering is centralized.
- **Candidate touch points**:
  - `oxide_gnss/src/device/config.rs`
    - Ensure that after writing a UBX command, the expected ACK is set (e.g., CFG-VALSET class/id).
    - Consider draining buffered input prior to config sequence.
  - `oxide_gnss/src/device/ubx.rs`
    - Use the existing `PendingAck` fields to filter.
    - When an ACK does not match the pending expectation, treat it as “unrelated” (log at trace/debug), not as completion.
- **Validation**:
  - Unit/integration tests with recorded UBX streams:
    - stale ACK preceding a command must not satisfy wait.
    - wrong ACK must not satisfy wait.
    - correct ACK/NAK must satisfy wait.
- **Policy (initial)**:
  - Fail fast on first NAK (no retries by default). If retries are added later, gate behind a clear policy/config.

#### P0.3 Make integrity `FAILED` reachable via explicit staleness semantics

- **Goal**: Align implementation with “strong safety-oriented semantics” intent.
- **Implementation approach (recommended)**:
  - Track last-update timestamps for each required signal source (at minimum `NAV_PVT`, plus any sources that materially affect integrity decisions).
  - Add stale thresholds and define exact behavior:
    - stale → `IntegrityLevel::Failed`
    - publish `~/operational = false`
  - Ensure integrity updates even when data stops (periodic reevaluation).
- **Candidate touch points**:
  - `oxide_gnss/src/state/integrity.rs`
    - Add per-source `last_*_update: Option<Instant>` fields.
    - Add `update_*()` methods to set these timestamps.
    - In `compute()`, check staleness first and return `Failed` with clear status.
  - `oxide_gnss/src/device/task.rs`
    - Add a periodic timer (e.g., `tokio::time::interval`) that triggers integrity recomputation/publish even without new UBX messages.
- **Validation**:
  - Add tests for:
    - “no PVT ever received” → `Failed`.
    - “PVT stops for > threshold” → transitions to `Failed`.
    - recovery when data resumes.
- **Initial defaults (approved)**:
  - Staleness thresholds:
    - `NAV_PVT` stale if `now - last_pvt > max(1.0s, 5 × expected_pvt_period)`.
    - Other integrity-related signals (SEC/MON/RXM) may be tracked as “monitor staleness” and mapped to at least `Degraded` (implementation choice), but `NAV_PVT` staleness is the primary driver for `FAILED`.
  - Startup behavior (approved; no grace period):
    - Before the first `NAV_PVT` is received, integrity should remain `FAILED` with a clear status (e.g., “Waiting for GNSS data”), and `~/operational = false`.
    - Once `NAV_PVT` is received, staleness checks use the normal thresholds above.
  - Operational mapping:
    - `Ok` → operational `true`
    - `Degraded`/`Critical`/`Failed` → operational `false`
  - For future flexibility, implement operational mapping as a single policy function/struct so it can evolve to a more nuanced “degradation index” later.

### Milestone P1: Wire correction age into integrity (per decided policy)

#### P1.1 Use host-side “time since last RTCM received” as correction age for integrity

- **Goal**: Ensure the correction-age threshold in integrity is effective.
- **Implementation approach options**:
  - **Option A (recommended)**: Track `last_rtcm_received: Option<Instant>` in the component that already sees RTCM data continuously (either NTRIP task or device task), and feed computed age into `IntegrityAggregator::set_correction_age()`.
  - **Option B**: Store the timestamp in supervisor shared state and feed it into integrity computations.
- **Candidate touch points**:
  - `oxide_gnss/src/ntrip/task.rs` emits `NtripMessage::RtcmReceived`.
  - `oxide_gnss/src/main.rs` already forwards `RtcmReceived` into supervisor.
  - `oxide_gnss/src/ros/task.rs` updates correction age for diagnostics.
  - `oxide_gnss/src/device/task.rs` is where integrity is computed/published today.
- **Recommended wiring**:
  - Add a `last_rtcm_received: Option<Instant>` to `DeviceTask` and update it when RTCM arrives on `rtcm_rx`.
  - On the periodic integrity tick, compute `correction_age_s = now - last_rtcm_received` (or `inf` if none) and call `integrity.set_correction_age(correction_age_s)` before `compute()`.
- **Validation**:
  - Simulate RTCM reception and verify integrity transitions as correction age exceeds threshold.

### Milestone P2: Interface correctness + documentation alignment

#### P2.1 Remove `~/hp_pos` topic claims and align validation

- **Goal**: Prevent users from expecting a topic that won’t exist.
- **Implementation**:
  - Update:
    - `config/ublox.rs::MESSAGE_REQUIREMENTS` to remove `~/hp_pos` and replace with something like `~/fix` (or a note that HP enhances `~/fix`).
    - Both READMEs and `docs/CONFIGURATION.md` to describe HP behavior accurately.
- **Validation**:
  - Docs build/read-through; config validation no longer flags `~/hp_pos` availability.

#### P2.2 Fix satellites “active SV” computation

- **Goal**: Make `~/satellites` output meaningful.
- **Implementation options**:
  - Populate `SatStatus.flags` correctly in `device/ubx.rs::parse_nav_sat` by reconstructing the raw bitfield from the `sv.flags()` accessors.
  - Or change internal representation to store `sv_used: bool` and compute active SVs from that.
- **Validation**:
  - Unit test on a sample NAV-SAT message with known used/unused SVs.
- **Schema policy (approved)**:
  - Schema changes are allowed. If fields are changed/added, update docs to describe the JSON schema and consider versioning the schema (even informally) to help downstream consumers.

#### P2.3 Decide `SecSigDetails` publishing vs removal

- **Goal**: Keep the interface honest while preserving future capability.
- **Decision (approved)**:
  - Keep `SecSigDetails` internal/not published for now.
- **Implementation**:
  - Document that the message exists but is currently not published on a ROS topic.
  - Ensure READMEs/config docs do not imply it is published.

#### P2.4 Document NTRIP TLS options

- **Goal**: Make security posture explicit and avoid accidental insecure configs.
- **Implementation**:
  - Update `docs/CONFIGURATION.md` to document:
    - `use_https`
    - `tls_skip_verify` with a prominent warning (testing only)
    - NTRIP protocol/version selection if exposed

### Milestone P3: Open-source hygiene improvements

#### P3.1 Fix `package.xml` maintainer metadata

- **Goal**: Remove placeholder `@example.com` and ensure contact is correct.

#### P3.2 Add standard community files

- **Goal**: Meet common OSS expectations.
- **Files** (recommended):
  - `CONTRIBUTING.md`
  - `SECURITY.md`
  - `CODE_OF_CONDUCT.md`
  - `CHANGELOG.md`
- **Policy (approved)**:
  - Adopt standard templates (e.g., Contributor Covenant for CoC) but keep content minimal and pragmatic for a small team.

#### P3.3 Consolidate READMEs

- **Goal**: Reduce divergence between repo root `README.md` and package `oxide_gnss/README.md`.
- **Policy (approved)**:
  - Root `README.md` is canonical.
  - If a package/crate README is kept, reduce it to a short stub pointing to the root README to avoid duplication.

## Proposed execution order (summary)

1. P0.1 `ublox` pin
2. P0.2 ACK correlation
3. P0.3 integrity staleness/`FAILED`
4. P1.1 correction age wired into integrity (host-side)
5. P2 docs/interface alignment (`~/hp_pos`, satellites)
6. P2 SecSigDetails decision
7. P3 repo hygiene items

## Approval checklist (what I suggest you verify)

- The P0 items match your release bar for “safety-oriented integrity”.
- The chosen approach for correction age (host-side) matches how you want to communicate it publicly.
- Any interface changes are acceptable for your downstream consumers.
