# Definitive Release Review: `oxide_gnss`

Date: 2025-12-20  
Reviewer: Cascade

## Scope

This report is a definitive pre–public-release review of:

- `oxide_gnss` (ROS2 GNSS driver crate + launch/config/docs)
- `oxide_gnss_msgs` (ROS message definitions)
- `ntrip-core` (extracted NTRIP client crate; intended to be consumed via crates.io) and the `oxide_gnss` ↔ `ntrip-core` integration

Prior review inputs incorporated:

- `docs/PUBLIC_RELEASE_REVIEW_GEMINI3.md`
- `docs/PUBLIC_RELEASE_REVIEW_OPUS45.md`
- `docs/PUBLIC_RELEASE_REVIEW_GPT52_A.md`
- `docs/PUBLIC_RELEASE_REVIEW_GPT52_B.md`
- `docs/PUBLIC_RELEASE_REVIEW_SYNTHESIS.md`

Constraints: this review does **not** modify code.

## Executive Summary

**Overall:** Conditionally ready to open-source. The `ntrip-core` migration fixed multiple high-severity issues from the earlier reviews, but a small number of **high-impact correctness + release-hygiene gaps remain**.

### Net improvements since the prior reviews

- **NTRIP credential logging risk: addressed**
  - `ntrip-core` redacts the Basic Auth header in request logging and redacts the password in `Debug` for config.
- **`use_https` mismatch: addressed**
  - TLS/HTTPS support exists in `ntrip-core` (rustls-based), and `oxide_gnss` maps `use_https` to TLS.
- **“Correction age is broken” (RTCM receipt not forwarded): addressed**
  - `oxide_gnss` now forwards `RtcmReceived` into the supervisor/ROS task, enabling correction-age tracking.

### Release blockers given stated maintainer intent

- **Unpinned `ublox` dependency**: build reproducibility and supply-chain risk.
- **UBX device config ACK/NAK correlation not enforced**: correctness risk during receiver configuration.
- **Integrity safety semantics gaps**: `FAILED`/staleness semantics are not implemented and correction-age is not currently wired into integrity (intended policy is host-side “time since last RTCM received”).

## Status of Previously-Identified High-Severity Items

| Item (from synthesis) | Current status | Notes |
|---|---:|---|
| Credential exposure in NTRIP debug logs | Resolved | `ntrip-core` redacts Authorization and password in Debug. |
| `use_https` config misleading / HTTPS not implemented | Resolved | TLS is implemented in `ntrip-core`; `oxide_gnss` maps `use_https` to TLS. |
| RTCM forwarding / correction-age plumbing broken | Resolved | `RtcmReceived` is forwarded to the supervisor and used in ROS task. |
| UBX ACK/NAK correlation incomplete | Still applies | Pending-ACK fields exist but are not used to filter/validate ACKs. |
| Integrity “FAILED” / data staleness semantics missing | Still applies | `FAILED` exists but is not reached by `IntegrityAggregator::compute()`. |
| Dependency pinning concerns (ublox) | Still applies | `ublox` is sourced from `branch = "master"`. |

## Findings and Recommendations

### 1) Build reproducibility / supply-chain: `ublox` on unpinned git branch (HIGH)

- **Evidence**: `oxide_gnss/Cargo.toml` patches `ublox` from git `branch = "master"`.
- **Why it matters**:
  - Builds can break non-deterministically.
  - You cannot reliably reproduce fielded binaries.
- **Recommendation**:
  - Pin to a specific commit (`rev = "…"`) or a tagged release.
  - If you need unreleased messages, still pin to a known-good commit and update intentionally.

### 2) Device configuration correctness: UBX ACK/NAK correlation not validated (HIGH)

- **Evidence**:
  - `device/ubx.rs` contains `PendingAck` + `expect_ack()` but ACK/NAK handling currently returns `AckResult` for any ACK/NAK seen.
  - `device/config.rs` waits for an ACK/NAK but does not ensure it matches the command that was just sent.
- **Risk**:
  - Configuration can “succeed” due to a stale or unrelated ACK.
  - A NAK could be misattributed.
- **Recommendation**:
  - Track expected ACK class/id for each command and only accept matching ACK/NAK.
  - Consider draining input or resetting parser state at the start of configuration.

### 3) Integrity semantics: `FAILED`/stale-data semantics not implemented (HIGH)

- **Evidence**:
  - `IntegrityLevel::Failed` exists and is the default level.
  - `IntegrityAggregator::compute()` initializes to `Ok` and only transitions among `Ok`, `Degraded`, and `Critical` based on checks.
  - There is no explicit staleness evaluation (e.g., “no PVT for N seconds”).
- **Why it matters**:
  - Public-facing docs describe `FAILED = unavailable or data stale`, but the implementation does not enforce that.
  - Operational “go/no-go” can be misleading if upstream data becomes stale.
- **Recommendation**:
  - Add timestamps per signal source and explicit stale thresholds.
  - Make `FAILED` a reachable state.
  - Ensure `~/operational` reflects staleness and not just last-computed integrity.

#### 3a) Integrity correction age is unwired; policy is host-side time since last RTCM received (MEDIUM)

- **Evidence**:
  - `IntegrityAggregator` supports `set_correction_age()` and `compute()` checks correction age.
  - The runtime path updates correction age for diagnostics (via `RtcmReceived`), but integrity does not obviously consume that value.
- **Recommendation**:
  - Use host-side “time since last RTCM received” as the source of truth.
  - Feed this value into `IntegrityAggregator::set_correction_age()` before each `compute()` (or on a periodic timer), using the same timestamp source currently used for diagnostics.

### 4) ROS/topic and validation mismatches (MEDIUM)

#### 4a) `~/hp_pos` is referenced but not actually published

- **Evidence**:
  - `config/ublox.rs::MESSAGE_REQUIREMENTS` associates `NAV_HPPOSLLH` with `~/hp_pos`.
  - READMEs reference `~/hp_pos`.
  - Implementation uses HP position to *enhance* `~/fix` when enabled; there is no `~/hp_pos` publisher.
- **Maintainer decision**:
  - Do **not** publish `~/hp_pos`; treat `NAV_HPPOSLLH` as an enhancement path for `~/fix` when enabled.
- **Recommendation**:
  - Update docs and `MESSAGE_REQUIREMENTS` to remove the `~/hp_pos` topic claim and instead document the HP-enhanced `~/fix` behavior (including any enabling config such as `use_hp_for_fix`).

#### 4b) `~/satellites` “active satellite count” is incorrect

- **Evidence**:
  - `device/ubx.rs::parse_nav_sat` sets `SatStatus.flags = 0` as a placeholder.
  - `ros/publishers.rs` derives `active_svs` from `flags`.
- **Recommendation**:
  - Populate flags (or compute active SVs from `sv.flags()` directly) so published values are meaningful.

### 5) Unused message surface: `oxide_gnss_msgs/SecSigDetails` (LOW-MEDIUM)

- **Evidence**:
  - Message exists in `oxide_gnss_msgs` and conversion exists in `ros/conversions.rs`.
  - `RosTask` ignores `GnssMessage::SecSig`, so this message is not published.
- **Recommendation**:
  - Either publish it (with clear topic name + enabling rule), or document that it is reserved/not exposed yet.

### 6) Documentation/config completeness gaps (LOW-MEDIUM)

- **NTRIP TLS options not documented**:
  - `docs/CONFIGURATION.md` shows `use_https` but does not mention `tls_skip_verify` or NTRIP protocol selection.
- **Two READMEs diverge**:
  - Top-level `README.md` vs `oxide_gnss/README.md` have overlapping but not identical interface claims.

## `ntrip-core` Crate Review (Security, Correctness, Release Hygiene)

### Security

- **TLS**: Implemented via `tokio-rustls` + `webpki-roots` (no OpenSSL).
- **Credential handling**:
  - Authorization header is redacted in logged requests.
  - Password is redacted in `Debug` for `NtripConfig`.
- **Risk knobs**:
  - `tls_skip_verify` is supported; this is inherently unsafe. It is acceptable as a feature but should be clearly documented as “testing only”.

### Correctness / robustness

- **Protocol support**: NTRIP v1 (ICY) and v2 (HTTP/1.1 + chunked) support is present.
- **Timeouts and reconnection**:
  - `ntrip-core` supports retry/reconnect behavior.
  - `oxide_gnss` explicitly calls `without_reconnect()` and implements its own reconnection loop in `ntrip/task.rs`.

### Release hygiene

- Crate has internal tests for key parsing/formatting utilities.
- API surface is reasonably small and documented in `lib.rs`.
- **Dependency source**: `oxide_gnss` already depends on `ntrip-core` via crates.io (`ntrip-core = "0.1"`); no git/path override is present in `oxide_gnss/Cargo.toml`.

## Recommended Release Checklist

### P0 (before public visibility)

- Pin `ublox` dependency (`rev` or tag).
- Implement UBX ACK/NAK correlation (or constrain claims and document limitations clearly).
- Implement integrity staleness semantics and make `FAILED` reachable.
- Resolve `~/hp_pos` documentation/validation mismatch (HP enhances `~/fix`, no standalone topic) and fix `~/satellites` active-count computation.
- Fix `oxide_gnss/package.xml` maintainer metadata.

### P1 (first follow-up release)

- Wire correction age into integrity using host-side “time since last RTCM received” (source: `RtcmReceived`).
- Document NTRIP TLS options (`tls_skip_verify`, protocol version selection) in `CONFIGURATION.md`.
- Decide and document timestamp policy (ROS time vs wall clock).

## Maintainer decisions captured

- `~/hp_pos` will **not** be published; HP data is used to enhance `~/fix` when available/enabled.
- “Integrity” is intended to evolve toward **stronger safety-oriented semantics**; current gaps are treated as future improvement work items.
- Integrity “correction age” should be host-side **time since last RTCM received**.
- `oxide_gnss` should consume `ntrip-core` as the published crate dependency (crates.io), not as a git/path dependency.
