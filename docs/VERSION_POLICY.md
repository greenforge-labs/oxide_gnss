# Version Policy

## Semantic Versioning

oxide_gnss follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** (x.0.0): Incompatible API changes
- **MINOR** (0.x.0): New functionality, backwards-compatible
- **PATCH** (0.0.x): Backwards-compatible bug fixes

## Pre-1.0 Stability

While the version is below 1.0.0, the public API is not considered stable.
Breaking changes may occur in minor version bumps (e.g. 0.1.0 to 0.2.0).
Patch releases (e.g. 0.1.0 to 0.1.1) will remain backwards-compatible.

Once 1.0.0 is released, the full SemVer guarantees apply.

## Deprecation Process

1. Deprecated items are marked with `#[deprecated(since = "x.y.z", note = "...")]`
2. Deprecated items are kept for at least one minor release before removal
3. Removals are documented in the [CHANGELOG](../CHANGELOG.md) under a **Removed** section

## Minimum Supported Rust Version (MSRV)

The MSRV is declared in `Cargo.toml` as the `rust-version` field (currently 1.80).

- MSRV bumps are treated as minor version changes
- The MSRV will not exceed the Rust version available in the oldest supported
  Ubuntu LTS (currently Ubuntu 22.04 / Humble)

## ROS 2 Distro Support

Supported distros are listed in the [README](../README.md#supported-ros-2).
Dropping support for a ROS 2 distro that has reached end-of-life is not
considered a breaking change.
