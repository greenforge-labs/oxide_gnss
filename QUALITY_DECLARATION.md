# Quality Declaration — oxide_gnss

This document declares the quality level of the `oxide_gnss` package per
[REP-2004](https://www.ros.org/reps/rep-2004.html).

**Claimed quality level: 3**

## Version Policy

`oxide_gnss` follows [Semantic Versioning 2.0.0](https://semver.org/). The current
version is declared in `Cargo.toml` and `package.xml`.

Full version and deprecation policy: [`docs/VERSION_POLICY.md`](docs/VERSION_POLICY.md)

## Change Control

All changes are made through pull requests on
[GitHub](https://github.com/greenforge-labs/oxide_gnss). Each PR must pass CI before merging.

## Documentation

- User manual: [`docs/USER_MANUAL.md`](docs/USER_MANUAL.md)
- Configuration guide: [`docs/CONFIGURATION.md`](docs/CONFIGURATION.md)
- ROS 2 topics reference: [`docs/TOPICS.md`](docs/TOPICS.md)
- Integrity monitoring: [`docs/INTEGRITY.md`](docs/INTEGRITY.md)
- Development guide: [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)
- Version policy: [`docs/VERSION_POLICY.md`](docs/VERSION_POLICY.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Changelog: [`CHANGELOG.md`](CHANGELOG.md)

## License

`oxide_gnss` is licensed under the [MIT License](LICENSE).

## Testing

Unit and integration tests are included and run in CI:

```bash
cargo test --features ros2
```

Static analysis is enforced in CI:

```bash
cargo clippy --features ros2 -- -D warnings
cargo fmt --all -- --check
```

Code coverage is reported via `cargo-llvm-cov` and uploaded to Codecov.

## Public API Documentation

All public items are documented. The `#![warn(missing_docs)]` lint is enabled
to enforce documentation completeness.

```bash
cargo doc --features ros2
```

## Platform Support

| Platform | Architecture | ROS 2 Distro | CI Status |
|---|---|---|---|
| Ubuntu 24.04 (Noble) | amd64 | Jazzy, Kilted | Tested |
| Ubuntu 24.04 (Noble) | arm64 | Jazzy, Kilted | Tested |
| Ubuntu 22.04 (Jammy) | amd64 | Humble | Tested |

## Roadmap to Quality Level 2

The following items would be needed to claim Level 2:

- [ ] Code coverage tracking with minimum threshold
- [ ] Vulnerability disclosure policy
- [ ] System-level integration tests
- [ ] All runtime dependencies at Level 2 or higher
