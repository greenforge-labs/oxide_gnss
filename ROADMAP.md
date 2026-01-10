# Roadmap

This document outlines the planned development direction for oxide_gnss.

## Current Status: v0.1.0 (Pre-Release)

oxide_gnss is approaching its first stable public release. The core functionality is complete and tested.

## v0.1.0 - Initial Release

**Status: Ready for Release**

- [x] Mode-based configuration system
- [x] NTRIP client with TLS support
- [x] Integrity monitoring with configurable thresholds
- [x] Core UBX message support (NAV-PVT, NAV-HPPOSLLH, NAV-SAT, etc.)
- [x] Protection level support (NAV-PL)
- [x] Jamming/spoofing detection (SEC-SIG)
- [x] Comprehensive documentation
- [x] Comprehensive test coverage
- [x] Public API documentation (rustdoc)

## v0.2.0 - Enhanced Features

**Status: Planned**

- [ ] **Dead Reckoning Support**: ESF and HNR messages for IMU fusion
- [ ] **Runtime Parameter Reconfiguration**: ROS2 parameter callbacks
- [ ] **Metrics Export**: Optional Prometheus endpoint for monitoring
- [ ] **Enhanced Diagnostics**: More detailed diagnostic messages
- [ ] **ROS2 Lifecycle Node**: Support for managed node lifecycle

## v0.3.0 - Extended Hardware Support

**Status: Future**

- [ ] **Additional u-blox Receivers**: Support for F9R, M9N, other ZED variants
- [ ] **RTCM Output**: Base station mode with RTCM correction output
- [ ] **Multi-band Configuration**: L1/L2/L5 band selection

## Future Considerations

These are ideas under consideration but not yet committed:

- **Web Configuration UI**: Browser-based configuration and monitoring
- **Survey-in Mode**: Automatic base station survey for static positioning
- **Time Server**: NTP/PTP time source from GNSS
- **Additional GNSS Chipsets**: Support beyond u-blox (Septentrio, Trimble, etc.)

## Non-Goals

The following are explicitly out of scope for oxide_gnss:

- **Dual-Receiver Fusion**: For safety-critical applications requiring dual-redundant receivers with fusion, see [ferrous_gnss](https://github.com/gsokoll/ferrous_gnss) (private)
- **RTK Processing**: oxide_gnss relies on the receiver's internal RTK engine; it does not implement RTK algorithms
- **Post-Processing**: No support for PPK or other post-processing workflows

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to get involved. Feature requests and bug reports are welcome via GitHub Issues.

## Versioning

oxide_gnss follows [Semantic Versioning](https://semver.org/):
- **MAJOR**: Breaking API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

Until v1.0.0, the API may change between minor versions.
