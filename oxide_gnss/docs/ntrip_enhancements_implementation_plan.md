# NTRIP Client Enhancements - Implementation Plan

**Date:** December 2024  
**Author:** Cascade  
**Status:** All Phases Complete ✅

---

## Assessment

The existing implementation in `src/ntrip/client.rs` is well-structured with clean async architecture. The original planning document's staged approach is sensible, but this plan makes some adjustments based on practical considerations.

---

## Proposed Implementation Plan

### **Phase 1: Critical Security Fixes** *(~30 min)* ✅ COMPLETE
Do this immediately before any other work.

| Task | Description | Status |
|------|-------------|--------|
| **1.1** | Redact `Authorization` header in debug log at line 94 of `client.rs` | ✅ |
| **1.2** | Add validation error in `ntrip.rs` when `use_https: true` (temporary until TLS implemented) | ✅ |
| **1.3** | Trace `NtripMessage::RtcmReceived` to supervisor for `correction_age` diagnostics | ✅ |

---

### **Phase 2: Read Timeout** *(~1 hr)* ✅ COMPLETE
Critical for robustness—silent disconnects are a real operational problem.

| Task | Description | Status |
|------|-------------|--------|
| **2.1** | Add `read_timeout_secs: u32` to `NtripConnectionConfig` (default: 30) | ✅ |
| **2.2** | Wrap `read_chunk()` in `tokio::time::timeout()` | ✅ |
| **2.3** | Add `TimedOut` variant to `NtripState` for observability | ✅ |
| **2.4** | Treat timeout as disconnect → triggers existing reconnect logic | ✅ |

---

### **Phase 3: TLS/HTTPS Support** *(~3 hr)* ✅ COMPLETE
This unlocks commercial casters and secures credentials.

| Task | Description | Status |
|------|-------------|--------|
| **3.1** | Add `tokio-rustls` + `webpki-roots` to `Cargo.toml` | ✅ |
| **3.2** | Create `NtripStream` enum abstracting `TcpStream` vs `TlsStream` | ✅ |
| **3.3** | Implement `AsyncRead`/`AsyncWrite` for `NtripStream` | ✅ |
| **3.4** | Add `tls_skip_verify` config option for self-signed certs | ✅ |
| **3.5** | Remove validation error from Phase 1.2, wire up real TLS | ✅ |

**Key design decision:** Implement the stream abstraction like this:
```rust
enum NtripStream {
    Plain(TcpStream),
    Tls(tokio_rustls::client::TlsStream<TcpStream>),
}
```
Then use `pin_project` or manual trait impls to unify the read/write interface.

---

### **Phase 4: NTRIP v2 Protocol** *(~4 hr)* ✅ COMPLETE
Lower priority than TLS but needed for some modern casters.

| Task | Description | Status |
|------|-------------|--------|
| **4.1** | Add `ntrip_version` config: `"1"`, `"2"`, or `"auto"` (default: `"auto"`) | ✅ |
| **4.2** | Modify request builder for v2 headers (`Ntrip-Version: Ntrip/2.0`, HTTP/1.1) | ✅ |
| **4.3** | Implement chunked transfer-encoding decoder | ✅ |
| **4.4** | Auto-detect version from response (`ICY` vs `HTTP/1.1`) | ✅ |

**Complexity note:** Chunked decoding requires a small state machine. Create a `ChunkedDecoder` struct rather than inline logic.

---

### **Phase 5: Sourcetable & Discovery** *(~2 hr)* ✅ COMPLETE
Mountpoint discovery and nearest-base selection.

| Task | Description | Status |
|------|-------------|--------|
| **5.1** | Add `get_sourcetable()` method (GET `/` instead of `/{mountpoint}`) | ✅ |
| **5.2** | Parse `STR;...` lines into `StreamEntry` struct | ✅ |
| **5.3** | Add distance calculation (Haversine) for nearest-mountpoint suggestion | ✅ |

---

## Changes from Original Planning Document

1. **Merge Phases 0 and 1** — Read timeout is trivial and belongs with the critical fixes
2. **Skip Phase 5 (Enhanced Diagnostics) initially** — RTCM frame parsing is nice but not essential; can add later
3. **Environment variable credentials** — Defer this; config files are fine for now
4. **Test strategy** — Add integration test harness using a mock NTRIP server (lightweight TCP listener)

---

## Dependencies to Add

```toml
tokio-rustls = "0.26"
webpki-roots = "0.26"
```

---

## Estimated Total Effort

| Phase | Time |
|-------|------|
| Phase 1 (Critical) | 30 min |
| Phase 2 (Timeout) | 1 hr |
| Phase 3 (TLS) | 3 hr |
| Phase 4 (NTRIP v2) | 4 hr |
| Phase 5 (Sourcetable) | 2 hr |
| **Total** | **~10 hr** |

---

## Recommendation

Start with **Phases 1-3** as a coherent unit. These address the security concerns and make the client production-ready. Phase 4 (NTRIP v2) can follow based on actual user needs—many casters still support v1 fine.

---

## Files to Modify

| File | Phases |
|------|--------|
| `src/ntrip/client.rs` | 1, 2, 3, 4, 5 |
| `src/ntrip/task.rs` | 2 |
| `src/config/ntrip.rs` | 1, 2, 3, 4 |
| `src/state/ntrip_state.rs` | 2 |
| `src/main.rs` | 1 |
| `Cargo.toml` | 3 |

---

## Revision History

| Date | Author | Changes |
|------|--------|---------|
| 2024-12-20 | Cascade | Initial implementation plan |
| 2024-12-20 | Cascade | Completed Phases 1-3 |
| 2024-12-20 | Cascade | Completed Phase 4 (NTRIP v2) |
| 2024-12-20 | Cascade | Completed Phase 5 (Sourcetable) - All phases done |
