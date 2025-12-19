#!/bin/bash
#
# NTRIP Client Comprehensive Test Suite
#
# This script exercises the NTRIP client against various public casters
# with different protocols, ports, and authentication methods.
#
# CREDENTIAL HANDLING:
# 1. Create a file: examples/ntrip_credentials.env (gitignored)
# 2. Or export environment variables before running
#
# Example ntrip_credentials.env:
#   export NTRIP_RTK2GO_USER="your.email@example.com"
#   export NTRIP_RTK2GO_PASS=""
#   export NTRIP_EUREF_USER="your_username"
#   export NTRIP_EUREF_PASS="your_password"
#   export NTRIP_AUSCORS_USER="your_username"
#   export NTRIP_AUSCORS_PASS="your_password"
#
# Usage:
#   ./examples/ntrip_test_suite.sh [--quick|--full|--connect]
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
PASS=0
FAIL=0
SKIP=0

# Load credentials from file if it exists
CREDS_FILE="$SCRIPT_DIR/ntrip_credentials.env"
if [[ -f "$CREDS_FILE" ]]; then
    echo -e "${BLUE}Loading credentials from $CREDS_FILE${NC}"
    source "$CREDS_FILE"
else
    echo -e "${YELLOW}No credentials file found at $CREDS_FILE${NC}"
    echo -e "${YELLOW}Stream connection tests will be skipped unless env vars are set${NC}"
fi

# Build the test binary
echo -e "\n${BLUE}=== Building ntrip_test ===${NC}\n"
cd "$PROJECT_DIR"
cargo build --example ntrip_test --release 2>/dev/null || cargo build --example ntrip_test

NTRIP_TEST="$PROJECT_DIR/target/debug/examples/ntrip_test"
if [[ -f "$PROJECT_DIR/target/release/examples/ntrip_test" ]]; then
    NTRIP_TEST="$PROJECT_DIR/target/release/examples/ntrip_test"
fi

# Helper functions
run_test() {
    local name="$1"
    shift
    echo -e "\n${BLUE}>>> $name${NC}"
    if timeout 30 "$NTRIP_TEST" "$@" 2>&1; then
        echo -e "${GREEN}✓ PASS${NC}"
        ((PASS++))
    else
        echo -e "${RED}✗ FAIL${NC}"
        ((FAIL++))
    fi
}

run_test_expect_fail() {
    local name="$1"
    shift
    echo -e "\n${BLUE}>>> $name (expecting failure)${NC}"
    if timeout 30 "$NTRIP_TEST" "$@" 2>&1; then
        echo -e "${YELLOW}? Unexpected success${NC}"
        ((PASS++))
    else
        echo -e "${GREEN}✓ PASS (failed as expected)${NC}"
        ((PASS++))
    fi
}

skip_test() {
    local name="$1"
    local reason="$2"
    echo -e "\n${YELLOW}>>> $name - SKIPPED: $reason${NC}"
    ((SKIP++))
}

section() {
    echo -e "\n${BLUE}════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
}

# ============================================================================
# QUICK TESTS - No authentication required (sourcetable only)
# ============================================================================

quick_tests() {
    section "QUICK TESTS - Sourcetable Retrieval (No Auth)"

    # RTK2go - Large community caster
    run_test "RTK2go sourcetable" sourcetable rtk2go.com

    # Test NTRIP versions
    run_test "RTK2go v1 protocol" sourcetable rtk2go.com --v1
    run_test "RTK2go v2 protocol" sourcetable rtk2go.com --v2

    # European casters
    run_test "EUREF-IP sourcetable" sourcetable euref-ip.net
    run_test "IGS-IP sourcetable" sourcetable igs-ip.net

    # Test locations feature
    run_test "Test locations (RTK2go)" test-locations rtk2go.com

    # Nearest mountpoint tests
    run_test "Nearest to Brisbane" nearest rtk2go.com -27.47 153.02
    run_test "Nearest to London" nearest euref-ip.net 51.51 -0.13
    run_test "Nearest to Berlin" nearest euref-ip.net 52.52 13.41
}

# ============================================================================
# FULL TESTS - All casters, multiple protocols
# ============================================================================

full_tests() {
    section "FULL TESTS - All Public Casters"

    # Test all known casters
    run_test "All casters scan" test-casters

    section "Protocol Version Tests"

    # Version comparison on multiple casters
    run_test "RTK2go versions" test-versions rtk2go.com
    run_test "EUREF versions" test-versions euref-ip.net

    section "HTTPS/TLS Tests"

    # HTTPS connections (port 443)
    run_test "EUREF HTTPS" sourcetable euref-ip.net 443 --https
    run_test "IGS HTTPS" sourcetable products.igs-ip.net 443 --https

    section "Regional Casters"

    # Australia
    run_test "AUSCORS sourcetable" sourcetable auscors.ga.gov.au

    # Germany
    run_test "BKG Germany sourcetable" sourcetable igs.bkg.bund.de

    # France
    run_test "Centipede France sourcetable" sourcetable caster.centipede.fr

    section "Location-Based Tests"

    # Test from multiple world locations
    run_test "Sydney nearest (RTK2go)" nearest rtk2go.com -33.87 151.21
    run_test "Alice Springs nearest" nearest rtk2go.com -23.70 133.88
    run_test "San Francisco nearest" nearest rtk2go.com 37.77 -122.42
    run_test "Tokyo nearest" nearest rtk2go.com 35.68 139.65
    run_test "Paris nearest (EUREF)" nearest euref-ip.net 48.86 2.35

    section "Edge Cases"

    # Invalid host
    run_test_expect_fail "Invalid host" sourcetable nonexistent.invalid.host

    # Wrong port
    run_test_expect_fail "Wrong port" sourcetable rtk2go.com 9999
}

# ============================================================================
# CONNECT TESTS - Require credentials
# ============================================================================

connect_tests() {
    section "STREAM CONNECTION TESTS (Require Credentials)"

    # RTK2go - open with email
    if [[ -n "$NTRIP_RTK2GO_USER" ]]; then
        echo -e "${GREEN}RTK2go credentials available${NC}"
        
        # First get a mountpoint from the sourcetable
        echo "Finding a mountpoint to test..."
        MOUNTPOINT=$("$NTRIP_TEST" nearest rtk2go.com -27.47 153.02 2>/dev/null | grep "Recommended:" | awk '{print $2}' || echo "")
        
        if [[ -n "$MOUNTPOINT" ]]; then
            run_test "RTK2go stream connect ($MOUNTPOINT)" \
                connect rtk2go.com "$MOUNTPOINT" --user="$NTRIP_RTK2GO_USER" --pass="$NTRIP_RTK2GO_PASS"
        else
            skip_test "RTK2go stream connect" "Could not find mountpoint"
        fi
        
        # Test v1 vs v2 connection
        if [[ -n "$MOUNTPOINT" ]]; then
            run_test "RTK2go v1 stream" \
                connect rtk2go.com "$MOUNTPOINT" --user="$NTRIP_RTK2GO_USER" --v1
            run_test "RTK2go v2 stream" \
                connect rtk2go.com "$MOUNTPOINT" --user="$NTRIP_RTK2GO_USER" --v2
        fi
    else
        skip_test "RTK2go stream tests" "NTRIP_RTK2GO_USER not set"
    fi

    # EUREF - requires registration
    if [[ -n "$NTRIP_EUREF_USER" && -n "$NTRIP_EUREF_PASS" ]]; then
        echo -e "${GREEN}EUREF credentials available${NC}"
        
        MOUNTPOINT=$("$NTRIP_TEST" nearest euref-ip.net 52.52 13.41 2>/dev/null | grep "Recommended:" | awk '{print $2}' || echo "")
        
        if [[ -n "$MOUNTPOINT" ]]; then
            run_test "EUREF stream connect ($MOUNTPOINT)" \
                connect euref-ip.net "$MOUNTPOINT" --user="$NTRIP_EUREF_USER" --pass="$NTRIP_EUREF_PASS"
        fi
    else
        skip_test "EUREF stream tests" "NTRIP_EUREF_USER/PASS not set"
    fi

    # AUSCORS - requires registration
    if [[ -n "$NTRIP_AUSCORS_USER" && -n "$NTRIP_AUSCORS_PASS" ]]; then
        echo -e "${GREEN}AUSCORS credentials available${NC}"
        
        MOUNTPOINT=$("$NTRIP_TEST" nearest auscors.ga.gov.au -27.47 153.02 2>/dev/null | grep "Recommended:" | awk '{print $2}' || echo "")
        
        if [[ -n "$MOUNTPOINT" ]]; then
            run_test "AUSCORS stream connect ($MOUNTPOINT)" \
                connect auscors.ga.gov.au "$MOUNTPOINT" --user="$NTRIP_AUSCORS_USER" --pass="$NTRIP_AUSCORS_PASS"
        fi
    else
        skip_test "AUSCORS stream tests" "NTRIP_AUSCORS_USER/PASS not set"
    fi

    # Centipede - open community
    echo -e "${BLUE}Centipede (open community)${NC}"
    MOUNTPOINT=$("$NTRIP_TEST" nearest caster.centipede.fr 48.86 2.35 2>/dev/null | grep "Recommended:" | awk '{print $2}' || echo "")
    if [[ -n "$MOUNTPOINT" ]]; then
        run_test "Centipede open stream ($MOUNTPOINT)" \
            connect caster.centipede.fr "$MOUNTPOINT"
    else
        skip_test "Centipede stream" "Could not find mountpoint"
    fi
}

# ============================================================================
# MAIN
# ============================================================================

print_summary() {
    section "TEST SUMMARY"
    echo -e "  ${GREEN}PASSED: $PASS${NC}"
    echo -e "  ${RED}FAILED: $FAIL${NC}"
    echo -e "  ${YELLOW}SKIPPED: $SKIP${NC}"
    echo ""
    
    if [[ $FAIL -gt 0 ]]; then
        echo -e "${RED}Some tests failed!${NC}"
        exit 1
    else
        echo -e "${GREEN}All tests passed!${NC}"
        exit 0
    fi
}

case "${1:-quick}" in
    --quick|-q)
        echo -e "${BLUE}Running QUICK tests (sourcetable only, no auth)${NC}"
        quick_tests
        ;;
    --full|-f)
        echo -e "${BLUE}Running FULL tests (all casters, no auth)${NC}"
        quick_tests
        full_tests
        ;;
    --connect|-c)
        echo -e "${BLUE}Running CONNECTION tests (requires credentials)${NC}"
        quick_tests
        connect_tests
        ;;
    --all|-a)
        echo -e "${BLUE}Running ALL tests${NC}"
        quick_tests
        full_tests
        connect_tests
        ;;
    --help|-h)
        echo "NTRIP Test Suite"
        echo ""
        echo "Usage: $0 [--quick|--full|--connect|--all]"
        echo ""
        echo "Options:"
        echo "  --quick, -q    Quick tests (sourcetable only, no auth) [default]"
        echo "  --full, -f     Full tests (all casters, protocols, no auth)"
        echo "  --connect, -c  Connection tests (requires credentials)"
        echo "  --all, -a      Run all tests"
        echo ""
        echo "Credentials:"
        echo "  Create examples/ntrip_credentials.env with:"
        echo "    export NTRIP_RTK2GO_USER=\"your.email@example.com\""
        echo "    export NTRIP_RTK2GO_PASS=\"\""
        echo "    export NTRIP_EUREF_USER=\"...\""
        echo "    export NTRIP_EUREF_PASS=\"...\""
        exit 0
        ;;
    *)
        echo "Unknown option: $1"
        echo "Use --help for usage"
        exit 1
        ;;
esac

print_summary
