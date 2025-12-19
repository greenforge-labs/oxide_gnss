# NTRIP Client Enhancements

**Date:** December 2024  
**Branch:** `ntrip_enhancements`  
**Status:** Planning

---

## Overview

The oxide_gnss package includes a custom-written NTRIP client implementation. While functional for basic use cases, it lacks several features found in mature NTRIP clients such as [pygnssutils](https://github.com/semuconsulting/pygnssutils). This document outlines recommended enhancements based on:

1. Findings from the public release code review (`PUBLIC_RELEASE_REVIEW_SYNTHESIS.md`)
2. Feature comparison with pygnssutils `GNSSNTRIPClient`
3. Real-world operational requirements

The goal is to progressively enhance the NTRIP client while maintaining its key advantage: tight integration with the oxide_gnss async architecture.

---

## Part 1: Analysis & Discussion

### 1.1 Current Implementation

The existing NTRIP client (`src/ntrip/client.rs`, `src/ntrip/task.rs`) provides:

| Feature | Status |
|---------|--------|
| NTRIP v1 protocol | ✅ Implemented |
| Raw TCP socket connection | ✅ Implemented |
| ICY 200 OK response handling | ✅ Implemented |
| HTTP/1.0 request construction | ✅ Implemented |
| Basic Authentication | ✅ Implemented |
| GGA position reporting | ✅ Implemented |
| Automatic reconnection with backoff | ✅ Implemented |
| Connection state monitoring | ✅ Implemented |

**Strengths:**
- Clean async/await architecture using Tokio
- Well-integrated with supervisor pattern and state management
- Exponential backoff with configurable reset period
- GGA sourced directly from receiver position

**Limitations:**
- NTRIP v1 only (no v2 support)
- No TLS/HTTPS support despite config option existing
- No sourcetable retrieval
- No read timeout (can hang on silent connection loss)
- Credential exposure in debug logs

### 1.2 Review Findings

The public release review identified several NTRIP-related issues:

#### Critical Issues
1. **`use_https` configuration is misleading** — The config option exists but TLS is not implemented. Users may believe their credentials are encrypted when they are not.

2. **Credential exposure via debug logging** — The full HTTP request including `Authorization: Basic <base64>` is logged at debug level.

#### Medium Priority Issues
3. **RTCM received events not forwarded** — `NtripMessage::RtcmReceived` is generated but not bridged to the supervisor, so `correction_age` diagnostics are never updated.

### 1.3 Feature Comparison with pygnssutils

| Feature | oxide_gnss | pygnssutils | Notes |
|---------|------------|-------------|-------|
| **Protocol Support** |
| NTRIP v1 | ✅ | ✅ | Legacy ICY protocol |
| NTRIP v2 | ❌ | ✅ | Proper HTTP/1.1, chunked transfer |
| **Security** |
| TLS/HTTPS | ❌ | ✅ | Required by some commercial services |
| Self-signed certs | ❌ | ✅ | Via custom cert path |
| Credential env vars | ❌ | ✅ | `PYGPSCLIENT_USER`, `PYGPSCLIENT_PASSWORD` |
| **Discovery** |
| Sourcetable retrieval | ❌ | ✅ | Query available mountpoints |
| Closest mountpoint | ❌ | ✅ | Auto-select by distance |
| **Data Handling** |
| RTCM3 streaming | ✅ | ✅ | Binary passthrough |
| SPARTN support | ❌ | ✅ | For PointPerfect etc. |
| Message type logging | ❌ | ✅ | Per-message type diagnostics |
| **Robustness** |
| Auto-reconnect | ✅ | ✅ | With backoff |
| Read timeout | ❌ | ✅ | Detect silent disconnects |
| GGA reporting | ✅ | ✅ | Periodic position updates |

### 1.4 Enhancement Rationale

#### 1.4.1 TLS/HTTPS Support

**Why it matters:**
- Commercial NTRIP services increasingly require HTTPS
- Credentials transmitted in plaintext over HTTP are vulnerable to interception
- The existing `use_https` config option creates a false sense of security

**Implementation approach:**
- Use `tokio-rustls` for async TLS (pure Rust, no OpenSSL dependency)
- Wrap `TcpStream` in `TlsStream` when `use_https: true`
- Consider supporting custom CA certificates for enterprise deployments

#### 1.4.2 NTRIP v2 Support

**Why it matters:**
- Better compatibility with modern casters and proxies
- Proper HTTP/1.1 semantics
- Some services only support v2

**Protocol differences:**
```
NTRIP v1:
  Request:  GET /mountpoint HTTP/1.0
  Response: ICY 200 OK

NTRIP v2:
  Request:  GET /mountpoint HTTP/1.1
            Ntrip-Version: Ntrip/2.0
  Response: HTTP/1.1 200 OK
            Transfer-Encoding: chunked
```

**Implementation approach:**
- Add `ntrip_version` config option (default v2, fallback to v1)
- Implement chunked transfer decoding for v2
- Detect protocol version from response

#### 1.4.3 Read Timeout

**Why it matters:**
- TCP connections can become "half-open" without explicit close
- Without timeout, client hangs indefinitely waiting for data
- Affects integrity monitoring — stale data not detected

**Implementation approach:**
- Add `read_timeout_secs` config option (default: 30s)
- Use `tokio::time::timeout` around read operations
- Treat timeout as disconnection, trigger reconnect

#### 1.4.4 Sourcetable Retrieval

**Why it matters:**
- Network RTK services have many mountpoints
- VRS services require selecting nearest base
- Useful for diagnostics and configuration validation

**Implementation approach:**
- Add `get_sourcetable()` method to client
- Parse sourcetable format: `STR;mountpoint;location;format;...`
- Optional: Calculate distances and suggest closest mountpoint

#### 1.4.5 Credential Security

**Why it matters:**
- Debug logs often end up in shared locations (CI, support tickets, ROS bags)
- Base64 encoding is trivially reversible

**Implementation approach:**
- Redact `Authorization` header before logging
- Support environment variables for credentials
- Document secure credential management

---

## Part 2: Staged Implementation Plan

### Stage 0: Critical Fixes (Pre-release)

**Scope:** Address review P0 items with minimal changes.

| Task | File | Effort | Description |
|------|------|--------|-------------|
| 0.1 | `client.rs` | 5 min | Redact Authorization header from debug logs |
| 0.2 | `ntrip.rs` | 15 min | Fail validation when `use_https: true` with clear error |
| 0.3 | `main.rs` | 15 min | Forward `NtripMessage::RtcmReceived` to supervisor |

**Acceptance criteria:**
- [ ] `RUST_LOG=debug` does not expose credentials
- [ ] Setting `use_https: true` produces actionable error message
- [ ] `correction_age` updates in diagnostics

---

### Stage 1: Read Timeout & Robustness

**Scope:** Improve connection reliability and failure detection.

| Task | File | Effort | Description |
|------|------|--------|-------------|
| 1.1 | `ntrip.rs` | 10 min | Add `read_timeout_secs` config option (default: 30) |
| 1.2 | `client.rs` | 30 min | Wrap `read_chunk` with timeout, return timeout error |
| 1.3 | `task.rs` | 15 min | Handle read timeout as disconnect, trigger reconnect |
| 1.4 | `ntrip_state.rs` | 15 min | Add `TimedOut` variant to connection state |

**Acceptance criteria:**
- [ ] Client reconnects within 30s of caster going silent
- [ ] Timeout events logged and reflected in state
- [ ] Configurable timeout value

---

### Stage 2: TLS/HTTPS Support

**Scope:** Implement secure connections.

| Task | File | Effort | Description |
|------|------|--------|-------------|
| 2.1 | `Cargo.toml` | 5 min | Add `tokio-rustls` dependency |
| 2.2 | `client.rs` | 2 hr | Abstract stream type, implement TLS handshake |
| 2.3 | `ntrip.rs` | 15 min | Add `tls_skip_verify` config option for testing |
| 2.4 | — | 30 min | Test against real HTTPS NTRIP caster |

**Implementation notes:**
```rust
// Stream abstraction
enum NtripStream {
    Plain(TcpStream),
    Tls(tokio_rustls::client::TlsStream<TcpStream>),
}

impl NtripStream {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { ... }
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> { ... }
}
```

**Acceptance criteria:**
- [ ] `use_https: true` establishes TLS connection
- [ ] Certificate validation works with public CAs
- [ ] Clear error messages for TLS failures
- [ ] Update documentation with HTTPS examples

---

### Stage 3: NTRIP v2 Protocol

**Scope:** Add NTRIP v2 support with chunked transfer encoding.

| Task | File | Effort | Description |
|------|------|--------|-------------|
| 3.1 | `ntrip.rs` | 10 min | Add `ntrip_version` config (1, 2, or auto) |
| 3.2 | `client.rs` | 1 hr | Construct v2 request with proper headers |
| 3.3 | `client.rs` | 2 hr | Implement chunked transfer decoding |
| 3.4 | `client.rs` | 30 min | Auto-detect version from response |
| 3.5 | — | 1 hr | Test against v1-only and v2-only casters |

**Chunked transfer format:**
```
<chunk-size-hex>\r\n
<chunk-data>\r\n
...
0\r\n
\r\n
```

**Acceptance criteria:**
- [ ] Works with NTRIP v1 casters (backward compatible)
- [ ] Works with NTRIP v2-only casters
- [ ] Auto-detection correctly identifies protocol version
- [ ] Chunked encoding properly decoded

---

### Stage 4: Sourcetable & Discovery

**Scope:** Add mountpoint discovery capabilities.

| Task | File | Effort | Description |
|------|------|--------|-------------|
| 4.1 | `client.rs` | 1 hr | Add `get_sourcetable()` method |
| 4.2 | `ntrip/` | 1 hr | Add `Sourcetable` struct and parser |
| 4.3 | `ntrip/` | 30 min | Add distance calculation for mountpoint selection |
| 4.4 | — | 30 min | Add CLI tool or ROS service for sourcetable query |

**Sourcetable format:**
```
STR;ALIC00AUS0;Alice Springs;RTCM 3.2;1005(30),1077(1)...;2;GPS+GLO;...;-23.67;133.88;...
```

**Acceptance criteria:**
- [ ] Can retrieve and parse sourcetable from caster
- [ ] Sourcetable entries include position, format, carrier info
- [ ] Optional: Suggest nearest mountpoint based on reference position

---

### Stage 5: Enhanced Diagnostics (Optional)

**Scope:** Improve observability and debugging.

| Task | File | Effort | Description |
|------|------|--------|-------------|
| 5.1 | `ntrip/` | 2 hr | Add RTCM3 frame detection (sync byte 0xD3, length) |
| 5.2 | `task.rs` | 30 min | Count messages by type, expose via handle |
| 5.3 | `ros/` | 30 min | Publish NTRIP diagnostics (msg counts, latency) |

**Acceptance criteria:**
- [ ] Per-message-type statistics available
- [ ] Diagnostic topic shows correction stream health

---

## Appendix A: Configuration Changes

### Current Configuration
```yaml
ntrip:
  host: "caster.example.com"
  port: 2101
  mountpoint: "MOUNT"
  username: "user"
  password: "pass"
  use_https: false
  send_gga: true
  gga_interval_secs: 10
  connection:
    timeout_secs: 10
    reconnect: true
    initial_delay_secs: 1
    max_delay_secs: 60
```

### Proposed Configuration (Post-Enhancements)
```yaml
ntrip:
  host: "caster.example.com"
  port: 2101
  mountpoint: "MOUNT"
  # Credentials can also be set via OXIDE_GNSS_NTRIP_USER / _PASSWORD
  username: "user"
  password: "pass"
  
  # Protocol settings
  use_https: true              # Now functional (Stage 2)
  ntrip_version: auto          # "1", "2", or "auto" (Stage 3)
  tls_skip_verify: false       # For self-signed certs (Stage 2)
  
  # GGA settings
  send_gga: true
  gga_interval_secs: 10
  
  # Connection settings
  connection:
    timeout_secs: 10           # Connection timeout
    read_timeout_secs: 30      # Data read timeout (Stage 1)
    reconnect: true
    initial_delay_secs: 1
    max_delay_secs: 60
```

---

## Appendix B: Dependencies

### Current
- `tokio` (async runtime)
- `base64` (auth encoding)
- `tracing` (logging)

### Proposed Additions
| Crate | Version | Purpose | Stage |
|-------|---------|---------|-------|
| `tokio-rustls` | 0.25+ | TLS support | 2 |
| `webpki-roots` | 0.26+ | Root CA certificates | 2 |

---

## Appendix C: References

- [NTRIP Protocol Specification](https://igs.bkg.bund.de/ntrip/download)
- [pygnssutils GNSSNTRIPClient](https://github.com/semuconsulting/pygnssutils)
- [swift-nav/ntripping](https://github.com/swift-nav/ntripping)
- [RTCM Standard 10403.x](https://rtcm.myshopify.com/)

---

## Revision History

| Date | Author | Changes |
|------|--------|---------|
| 2024-12 | Cascade | Initial planning document |
