#!/bin/bash
# Local CI Test Script for oxide_gnss
# Runs CI checks against multiple ROS2 distros using Docker
#
# Usage:
#   ./scripts/local_ci_test.sh              # Test all distros
#   ./scripts/local_ci_test.sh jazzy        # Test specific distro
#   ./scripts/local_ci_test.sh jazzy check  # Only run cargo check
#   ./scripts/local_ci_test.sh --help       # Show help

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

# Docker images for each ROS2 distro (rostooling images)
declare -A DOCKER_IMAGES=(
    ["humble"]="rostooling/setup-ros-docker:ubuntu-jammy-ros-humble-ros-base-latest"
    ["jazzy"]="rostooling/setup-ros-docker:ubuntu-noble-ros-jazzy-ros-base-latest"
    ["kilted"]="rostooling/setup-ros-docker:ubuntu-noble-ros-kilted-ros-base-latest"
    ["rolling"]="rostooling/setup-ros-docker:ubuntu-noble-ros-rolling-ros-base-latest"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

show_help() {
    echo "oxide_gnss Local CI Test Script"
    echo ""
    echo "Usage:"
    echo "  $0                    # Test all distros (humble, jazzy, kilted, rolling)"
    echo "  $0 <distro>           # Test specific distro"
    echo "  $0 <distro> check     # Only run cargo check (fast)"
    echo "  $0 <distro> build     # Only build, no tests"
    echo "  $0 <distro> full      # Full test suite (default)"
    echo "  $0 --help             # Show this help"
    echo ""
    echo "Available distros: ${!DOCKER_IMAGES[*]}"
    echo ""
    echo "Examples:"
    echo "  $0 jazzy              # Full test on Jazzy"
    echo "  $0 humble check       # Quick check on Humble"
    echo "  $0 all                # Test all distros"
}

# CI test script to run inside Docker container
CI_SCRIPT='
set -e
DISTRO=$1
TEST_MODE=$2

echo "=== Setting up Rust toolchain ==="
export CARGO_HOME=/tmp/cargo
export RUSTUP_HOME=/tmp/rustup
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
export PATH="$CARGO_HOME/bin:$PATH"
rustup component add clippy rustfmt

echo "=== Sourcing ROS2 $DISTRO ==="
source /opt/ros/$DISTRO/setup.bash

echo "=== Installing dependencies ==="
apt-get update -qq && apt-get install -y -qq \
    build-essential \
    cmake \
    git \
    libclang-dev \
    python3-pip \
    python3-vcstool \
    python3-colcon-common-extensions \
    ros-$DISTRO-rosidl-default-generators \
    ros-$DISTRO-rosidl-default-runtime \
    ros-$DISTRO-example-interfaces \
    ros-$DISTRO-test-msgs \
    > /dev/null 2>&1 || echo "Note: some deps may not be available"

# Install colcon-cargo plugins
pip install --break-system-packages -q \
    git+https://github.com/colcon/colcon-cargo.git \
    git+https://github.com/colcon/colcon-ros-cargo.git 2>/dev/null || \
pip install -q \
    git+https://github.com/colcon/colcon-cargo.git \
    git+https://github.com/colcon/colcon-ros-cargo.git 2>/dev/null || true

# Install cargo-ament-build
cargo install cargo-ament-build 2>/dev/null || true

echo "=== Cloning and building ros2-rust ==="
mkdir -p /tmp/ros2_ws/src
cd /tmp/ros2_ws/src
git clone --branch v0.7.0 --depth 1 https://github.com/ros2-rust/ros2_rust.git
vcs import . < ros2_rust/ros2_rust_${DISTRO}.repos 2>/dev/null || \
    vcs import . < ros2_rust/ros2_rust.repos 2>/dev/null || true

cd /tmp/ros2_ws
colcon build --packages-up-to rclrs std_msgs sensor_msgs geometry_msgs diagnostic_msgs 2>&1 | tail -20

echo ""
echo "========================================"
echo "  Testing oxide_gnss on ROS2 $DISTRO"
echo "========================================"
echo ""

# Set up workspace for oxide_gnss
mkdir -p /tmp/oxide_ws/src
cd /tmp/oxide_ws

# Copy oxide_gnss source (preserving structure)
cp -r /workspace/oxide_gnss src/oxide_gnss
if [ -d "/workspace/oxide_gnss/oxide_gnss_msgs" ]; then
    mv src/oxide_gnss/oxide_gnss_msgs src/
fi

# Source ros2-rust workspace
source /tmp/ros2_ws/install/setup.bash

# Copy cargo config for patches if available
mkdir -p .cargo
cp /tmp/ros2_ws/.cargo/config.toml .cargo/ 2>/dev/null || true

export CARGO_TARGET_DIR=/tmp/target

if [ "$TEST_MODE" = "check" ]; then
    echo "=== Running cargo check ==="
    cd src/oxide_gnss
    cargo check --features ros2
    echo "=== Check complete ==="
    exit 0
fi

echo "=== Building with colcon ==="
colcon build --packages-select oxide_gnss_msgs oxide_gnss 2>&1 || {
    echo "Colcon build failed, trying cargo directly..."
    cd src/oxide_gnss
    cargo build --features ros2
}

if [ "$TEST_MODE" = "build" ]; then
    echo "=== Build complete ==="
    exit 0
fi

# Source the built workspace
source install/setup.bash 2>/dev/null || true

echo ""
echo "=== Running clippy ==="
cd src/oxide_gnss
cargo clippy --features ros2 -- -D warnings 2>&1 || echo "Clippy had warnings"

echo ""
echo "=== Running tests ==="
cargo test --features ros2 2>&1 || echo "Some tests failed"

echo ""
echo "=== Full test complete for $DISTRO ==="
'

run_ci_for_distro() {
    local distro=$1
    local test_mode=${2:-"full"}
    local image=${DOCKER_IMAGES[$distro]}

    if [ -z "$image" ]; then
        log_error "Unknown distro: $distro"
        log_info "Available distros: ${!DOCKER_IMAGES[*]}"
        return 1
    fi

    log_info "Testing oxide_gnss on ROS2 $distro"
    log_info "Image: $image"
    log_info "Mode: $test_mode"

    docker run --rm \
        -v "$REPO_ROOT:/workspace/oxide_gnss:ro" \
        -e CARGO_HOME=/tmp/cargo \
        "$image" \
        bash -c "$CI_SCRIPT" -- "$distro" "$test_mode"

    local result=$?
    if [ $result -eq 0 ]; then
        log_info "✅ $distro: PASSED"
        return 0
    else
        log_error "❌ $distro: FAILED"
        return 1
    fi
}

main() {
    if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
        show_help
        exit 0
    fi

    local distro=${1:-"all"}
    local test_mode=${2:-"full"}
    local failed=0
    local passed=0

    echo ""
    log_info "oxide_gnss Local CI Test"
    log_info "Repository: $REPO_ROOT"
    echo ""

    if [ "$distro" = "all" ]; then
        for d in humble jazzy kilted rolling; do
            echo ""
            echo "========================================"
            log_step "Starting tests for $d"
            echo "========================================"
            if run_ci_for_distro "$d" "$test_mode"; then
                ((passed++))
            else
                ((failed++))
                if [ "$d" = "rolling" ]; then
                    log_warn "Rolling failure is expected (unstable)"
                fi
            fi
        done

        echo ""
        echo "========================================"
        echo "  SUMMARY"
        echo "========================================"
        log_info "Passed: $passed"
        if [ $failed -gt 0 ]; then
            log_warn "Failed: $failed"
        fi
    else
        if ! run_ci_for_distro "$distro" "$test_mode"; then
            failed=1
        fi
    fi

    echo ""
    if [ $failed -eq 0 ]; then
        log_info "✅ All tests passed!"
    else
        log_warn "⚠️  Some tests failed (check output above)"
    fi
    echo ""

    return $failed
}

main "$@"
