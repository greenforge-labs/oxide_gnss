# Quality Declaration — oxide_gnss

This document declares the quality level of the `oxide_gnss` package per
[REP-2004](https://www.ros.org/reps/rep-2004.html).

**Claimed quality level: 4**

## Quality Level 4 Requirements

### Version Policy

`oxide_gnss` follows [Semantic Versioning 2.0.0](https://semver.org/). The current
version is declared in `Cargo.toml` and `package.xml`.

### Change Control

All changes are made through pull requests on
[GitHub](https://github.com/gsokoll/oxide_gnss). Each PR must pass CI before merging.

### Documentation

- User manual: [`docs/USER_MANUAL.md`](docs/USER_MANUAL.md)
- Configuration guide: [`docs/CONFIGURATION.md`](docs/CONFIGURATION.md)
- ROS 2 topics reference: [`docs/TOPICS.md`](docs/TOPICS.md)
- Integrity monitoring: [`docs/INTEGRITY.md`](docs/INTEGRITY.md)
- Development guide: [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Changelog: [`CHANGELOG.md`](CHANGELOG.md)

### License

`oxide_gnss` is licensed under the [MIT License](LICENSE).

### Testing

Unit and integration tests are included and run in CI:

```bash
cargo test --features ros2
```

Static analysis is enforced in CI:

```bash
cargo clippy --features ros2 -- -D warnings
cargo fmt --all -- --check
```

### Platform Support

| Platform | Architecture | ROS 2 Distro | CI Status |
|---|---|---|---|
| Ubuntu 24.04 (Noble) | amd64 | Jazzy, Kilted | Tested |
| Ubuntu 22.04 (Jammy) | amd64 | Humble | Tested |
| Ubuntu 24.04 (Noble) | arm64 | Jazzy, Kilted | Not yet tested |

## Roadmap to Quality Level 3

The following items are needed to claim Level 3:

- [ ] Add arm64 CI testing
- [ ] Add code coverage reporting
- [ ] Document public API with `cargo doc` completeness
- [ ] Formalise version stability and deprecation policy
