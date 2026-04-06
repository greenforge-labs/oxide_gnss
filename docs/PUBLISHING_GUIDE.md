# Publishing Guide

Strategy and checklist for releasing oxide_gnss as an open-source ROS2 driver.

## Distribution Channels

### Official ROS2 Binary Distribution (bloom/rosdistro)

The standard ROS2 release pipeline is:

```
source repo → bloom-release → rosdistro PR → build farm → apt packages → index.ros.org
```

**Status: Blocked.** No Rust-based ROS2 package has ever been bloom-released. The ROS2
build farm does not support Cargo, and `rclrs` itself is not in rosdistro. This is blocked
on upstream ros2-rust work with no announced timeline.

**Action:** Monitor the [ros2-rust](https://github.com/ros2-rust/ros2_rust) project for
rosdistro/bloom support.

### crates.io

oxide_gnss cannot be published to crates.io as a complete package because:

- crates.io forbids `version = "*"` dependencies resolved via workspace patches
- `oxide_gnss_msgs` and standard ROS2 message crates are not on crates.io
- The package requires a colcon workspace with ros2-rust bindings

**Medium-term opportunity:** The UBX parser and NTRIP client modules have no ROS2
dependency and could be extracted as standalone crates. These would be independently
useful to the broader Rust embedded/GNSS community.

### Source Distribution (Current Standard)

Every ros2-rust package — including `rclrs` itself — distributes via GitHub with build
instructions. This is the standard and expected approach for the ros2-rust ecosystem today.

**This is the primary release mechanism for oxide_gnss.**

## ROS2 Quality Standards

### REP-2004: Package Quality Categories

REP-2004 defines five quality levels. Current status and targets:

| Requirement | Level 4 | Level 3 | oxide_gnss |
|---|---|---|---|
| Open-source license | Required | Required | MIT |
| CI pipeline | — | Required | Multi-distro CI |
| Static analysis | — | Required | clippy -D warnings |
| Code formatting | — | Recommended | cargo fmt --check |
| Change control (PRs) | — | Required | GitHub PRs |
| Platform statement | — | Required | Needed |
| Version policy | — | Required | SemVer (0.1.0) |
| Feature documentation | — | — | README + docs/ |
| QUALITY_DECLARATION.md | Recommended | Required | Needed |

**Current level: 4** (all requirements met).
**Target: Level 3** — requires adding a platform support statement and QUALITY_DECLARATION.md.

### REP-2000: Target Platforms

Tier 1 platforms for Jazzy and Kilted:

- Ubuntu 24.04 (Noble) — amd64 and arm64
- CI currently covers Humble, Jazzy, and Kilted on amd64

### REP-2005: Common Packages

Defines requirements for packages in the ROS2 desktop install. Not directly applicable
to community drivers but useful as a quality reference.

## Publishing Checklist

### Pre-Release

- [ ] Hardware test all configuration modes (rover_ntrip, moving_base, etc.)
- [ ] Ensure CHANGELOG.md is current for v0.1.0
- [ ] Standardise maintainer email across package.xml files
- [ ] Add a `QUALITY_DECLARATION.md` claiming Level 4, roadmap to Level 3
- [ ] Add platform support statement (Ubuntu 24.04, amd64/arm64, Jazzy/Kilted)
- [ ] Review and update README.md for first-time users
- [ ] Verify all docs/ files are current

### GitHub Release

- [ ] Tag v0.1.0 on the master branch
- [ ] Create a GitHub release with release notes
- [ ] Add GitHub repository topics: `ros2`, `gnss`, `ublox`, `ntrip`, `rust`, `rtk`

### Community Visibility

- [ ] Post announcement on [ROS Discourse](https://discourse.ros.org/) (Show & Tell or Drivers category)
- [ ] Submit PR to [awesome-ros2](https://github.com/fkromer/awesome-ros2) drivers section
- [ ] Consider posting to r/ROS and r/rust subreddits

### Post-Release

- [ ] Monitor GitHub issues for build/install problems
- [ ] Respond to Discourse thread feedback
- [ ] Update awesome-ros2 entry if the listing format changes

## Medium-Term Roadmap

### Standalone Crate Extraction

The following modules could be published to crates.io independently:

1. **UBX parser** (`device/ubx.rs`) — UBX protocol parsing for u-blox receivers.
   No ROS2 dependency. Useful for any Rust project working with u-blox hardware.

2. **NTRIP client** (`ntrip/`) — Async NTRIP v2 client with GGA feedback.
   No ROS2 dependency. Useful for any RTK positioning application.

These extractions would increase the project's reach and provide building blocks for
other ROS2 (or non-ROS2) GNSS projects.

### Bloom Release

When ros2-rust gains rosdistro/bloom support:

1. Register oxide_gnss with bloom
2. Create release repository under the appropriate GitHub organisation
3. Submit rosdistro PR for target distribution(s)
4. Verify build farm produces working packages

## Reference Links

- [REP-2004: Package Quality Categories](https://www.ros.org/reps/rep-2004.html)
- [REP-2000: ROS2 Releases and Target Platforms](https://www.ros.org/reps/rep-2000.html)
- [ROS2 First Time Release Guide](https://docs.ros.org/en/rolling/How-To-Guides/Releasing/First-Time-Release.html)
- [ros2-rust](https://github.com/ros2-rust/ros2_rust)
- [awesome-ros2](https://github.com/fkromer/awesome-ros2)
- [ROS Index](https://index.ros.org/)
- [ROS Discourse](https://discourse.ros.org/)
