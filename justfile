# oxide_gnss task runner
# Run `just` or `just --list` to see available recipes

# Default recipe - show available commands
default:
    @just --list

# ============================================================================
# Formatting
# ============================================================================

# Format all code
fmt:
    cargo fmt --all

# Check formatting (CI mode - fails if not formatted)
fmt-check:
    cargo fmt --all -- --check

# ============================================================================
# Linting & Checks
# ============================================================================

# Run all pre-commit checks (format, check, clippy)
check: fmt-check
    cargo check --features ros2

# Run clippy lints (treats warnings as errors)
clippy:
    cargo clippy --features ros2 -- -D warnings

# ============================================================================
# Testing
# ============================================================================

# Run all tests
test:
    cargo test --features ros2

# Run tests with output shown
test-verbose:
    cargo test --features ros2 -- --nocapture

# Run a specific test
test-one NAME:
    cargo test --features ros2 {{NAME}} -- --nocapture

# ============================================================================
# Building
# ============================================================================

# Build debug binary
build:
    cargo build --features ros2

# Build release binary
build-release:
    cargo build --release --features ros2

# ============================================================================
# CI Recipes (match GitHub Actions workflow)
# ============================================================================

# Run all CI checks (format, clippy, test)
ci: fmt-check clippy test
    @echo "✅ All CI checks passed!"

# ============================================================================
# Documentation
# ============================================================================

# Generate and open documentation
doc:
    cargo doc --features ros2 --open

# Generate documentation without opening
doc-build:
    cargo doc --features ros2

# ============================================================================
# Development Helpers
# ============================================================================

# Clean build artifacts
clean:
    cargo clean

# Watch for changes and run check (requires cargo-watch)
watch:
    cargo watch -x 'check --features ros2'

# Watch for changes and run tests (requires cargo-watch)
watch-test:
    cargo watch -x 'test --features ros2'

# Show outdated dependencies (requires cargo-outdated)
outdated:
    cargo outdated

# ============================================================================
# ROS2 / Colcon Integration
# ============================================================================

# Build with colcon (from workspace root)
# Usage: just colcon-build
# Note: Run from ~/ros2_ws after sourcing ROS2
[no-cd]
colcon-build:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${ROS_DISTRO:-}" ]; then
        echo "❌ ROS2 not sourced. Run: cd ~/ros2_ws && pixi shell"
        exit 1
    fi
    cd "$(git rev-parse --show-toplevel)/.."
    colcon build --packages-select oxide_gnss_msgs oxide_gnss

# Build messages only with colcon
[no-cd]
colcon-msgs:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${ROS_DISTRO:-}" ]; then
        echo "❌ ROS2 not sourced. Run: cd ~/ros2_ws && pixi shell"
        exit 1
    fi
    cd "$(git rev-parse --show-toplevel)/.."
    colcon build --packages-select oxide_gnss_msgs
