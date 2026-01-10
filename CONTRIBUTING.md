# Contributing to oxide_gnss

Thank you for your interest in contributing to oxide_gnss!

## Communication

- **Bug Reports & Feature Requests**: [GitHub Issues](https://github.com/gsokoll/oxide_gnss/issues)
- **Questions & Discussions**: [GitHub Discussions](https://github.com/gsokoll/oxide_gnss/discussions)

## Areas Where Contributions Are Welcome

- **Documentation**: Improving guides, adding examples, fixing typos
- **Testing**: Adding test coverage, especially for error paths
- **New UBX Messages**: Adding support for additional u-blox protocol messages
- **Bug Fixes**: Fixing reported issues
- **Performance**: Optimizations that don't sacrifice readability

## Getting Started

1. Fork the repository
2. Clone your fork and set up the development environment (see [Development Guide](docs/DEVELOPMENT.md))
3. Create a feature branch: `git checkout -b feature/your-feature-name`

## Development

### Prerequisites

- ROS2 Humble, Jazzy, or Rolling
- Rust stable toolchain
- libclang-dev
- Docker (for local CI testing)

### Building

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash
colcon build --packages-up-to oxide_gnss
```

### Running Tests

The project uses a `justfile` for common tasks. Install with `cargo install just`.

```bash
cd ~/ros2_ws/src/oxide_gnss

just ci          # Run all CI checks (format, clippy, tests)
just test        # Run tests only
just clippy      # Run clippy lints
just fmt         # Format code
```

Or run cargo directly:
```bash
cargo test --features ros2
cargo clippy --features ros2 -- -D warnings
cargo fmt --all -- --check
```

### Local CI Testing (Docker)

Test against multiple ROS2 distros locally before pushing:

```bash
./scripts/local_ci_test.sh              # Test all distros (humble, jazzy, kilted, rolling)
./scripts/local_ci_test.sh jazzy        # Test specific distro
./scripts/local_ci_test.sh jazzy check  # Quick cargo check only
```

This mirrors the GitHub Actions CI environment.

### Code Style

- Follow Rust conventions (`cargo fmt`, `cargo clippy`)
- Keep commits focused and atomic
- Write descriptive commit messages

## Submitting Changes

### Before Submitting

1. **Run local CI checks**: `just ci` or `./scripts/local_ci_test.sh jazzy`
2. **Ensure no `unwrap()` or `expect()`** in production code paths
3. **Add tests** for new functionality
4. **Update documentation** if adding/changing public APIs

### Pull Request Process

1. Push to your fork and open a Pull Request
2. Fill out the PR template with a description of changes
3. CI will automatically run format, clippy, and tests against multiple ROS2 distros
4. Address any review feedback
5. Maintainer will merge once approved and CI passes

### Commit Message Format

Use clear, descriptive commit messages:
```
feat: Add support for NAV-EOE message
fix: Handle disconnection during RTCM streaming
docs: Update CONFIGURATION.md with new threshold options
test: Add integration tests for device reconnection
```

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests
- Include reproduction steps, expected vs actual behavior
- For hardware-specific issues, include device model and firmware version

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
