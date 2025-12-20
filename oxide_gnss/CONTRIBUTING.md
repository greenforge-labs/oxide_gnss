# Contributing to oxide_gnss

Thank you for your interest in contributing to oxide_gnss!

## Getting Started

1. Fork the repository
2. Clone your fork and set up the development environment (see [Development Guide](docs/DEVELOPMENT.md))
3. Create a feature branch: `git checkout -b feature/your-feature-name`

## Development

### Prerequisites

- ROS2 Jazzy (Ubuntu 24.04)
- Rust stable toolchain
- libclang-dev

### Building

```bash
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash
colcon build --packages-up-to oxide_gnss
```

### Running Tests

The project uses a `justfile` for common tasks. Install with `cargo install just`.

```bash
cd ~/ros2_ws/src/oxide_gnss/oxide_gnss

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

### Code Style

- Follow Rust conventions (`cargo fmt`, `cargo clippy`)
- Keep commits focused and atomic
- Write descriptive commit messages

## Submitting Changes

1. Run all CI checks: `just ci`
2. Push to your fork and open a Pull Request
3. CI will automatically run format, clippy, and tests

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests
- Include reproduction steps, expected vs actual behavior
- For hardware-specific issues, include device model and firmware version

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
