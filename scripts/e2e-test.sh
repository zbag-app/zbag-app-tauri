#!/bin/bash
#
# E2E Test Runner for zbag
#
# This script orchestrates the full E2E test flow:
# 1. Build the test bridge (Rust backend with HTTP server)
# 2. Start the test bridge server
# 3. Wait for test bridge to be ready
# 4. Run Playwright tests
# 5. Cleanup
#
# Usage:
#   ./scripts/e2e-test.sh          # Run all E2E tests
#   ./scripts/e2e-test.sh --headed # Run with visible browser
#   ./scripts/e2e-test.sh --ui     # Open Playwright UI
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/apps/zbag-app-tauri"

TEST_BRIDGE_PORT=19816
TEST_BRIDGE_PID=""
PLAYWRIGHT_ARGS=("$@")

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
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

is_blank() {
    [[ -z "${1//[[:space:]]/}" ]]
}

# Use isolated test home directory to avoid polluting real data directory
# Track whether we created the directory (to avoid deleting user-provided directories)
if [ -n "${ZBAG_TEST_HOME+x}" ] && is_blank "$ZBAG_TEST_HOME"; then
    log_error "ZBAG_TEST_HOME is set but empty or whitespace; please unset it or provide a valid path."
    exit 1
fi

if [ "${VITE_TEST_BRIDGE:-}" = "true" ] && is_blank "${ZBAG_TEST_HOME:-}"; then
    log_error "VITE_TEST_BRIDGE=true requires ZBAG_TEST_HOME to be set to a non-empty path."
    exit 1
fi

if is_blank "${ZBAG_TEST_HOME:-}"; then
    export ZBAG_TEST_HOME="$(mktemp -d /tmp/zbag-e2e.XXXXXX)"
    TEST_HOME_CREATED="1"
else
    TEST_HOME_CREATED=""
fi
echo "Using test home: $ZBAG_TEST_HOME"

cleanup() {
    if [ -n "$TEST_BRIDGE_PID" ]; then
        log_info "Stopping test bridge (PID: $TEST_BRIDGE_PID)..."
        kill "$TEST_BRIDGE_PID" 2>/dev/null || true
        wait "$TEST_BRIDGE_PID" 2>/dev/null || true
    fi
    # Clean up test home directory (only if we created it)
    if [ -n "$TEST_HOME_CREATED" ] && [ -n "$ZBAG_TEST_HOME" ] && [ -d "$ZBAG_TEST_HOME" ]; then
        log_info "Cleaning up test home: $ZBAG_TEST_HOME"
        rm -rf "$ZBAG_TEST_HOME"
    fi
}

trap cleanup EXIT

wait_for_test_bridge() {
    local max_attempts=30
    local attempt=1

    log_info "Waiting for test bridge to be ready on port $TEST_BRIDGE_PORT..."

    while [ $attempt -le $max_attempts ]; do
        # Check if the process is still alive
        if ! kill -0 "$TEST_BRIDGE_PID" 2>/dev/null; then
            echo ""
            log_error "Test bridge process died unexpectedly"
            return 1
        fi
        if curl -s "http://127.0.0.1:$TEST_BRIDGE_PORT/health" > /dev/null 2>&1; then
            log_info "Test bridge is ready!"
            return 0
        fi
        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done

    echo ""
    log_error "Test bridge failed to start within ${max_attempts}s"
    return 1
}

# Validate health response is from our test bridge
validate_test_bridge() {
    local response
    response=$(curl -s "http://127.0.0.1:$TEST_BRIDGE_PORT/health" 2>/dev/null) || return 1

    # Check for expected fields in response
    if echo "$response" | grep -q '"status":"ok"' && echo "$response" | grep -q '"version"'; then
        return 0
    fi
    return 1
}

# Kill any process holding our port
kill_port_holder() {
    local pids
    pids=$(lsof -ti :$TEST_BRIDGE_PORT 2>/dev/null) || true
    if [ -n "$pids" ]; then
        log_warn "Killing stale process(es) on port $TEST_BRIDGE_PORT: $pids"
        echo "$pids" | xargs kill 2>/dev/null || true
        sleep 1
    fi
}

main() {
    cd "$PROJECT_ROOT"

    # Check if a valid test bridge is already running
    if validate_test_bridge; then
        log_info "Test bridge already running on port $TEST_BRIDGE_PORT"
    else
        # Check if port is occupied by something else
        if curl -s "http://127.0.0.1:$TEST_BRIDGE_PORT" > /dev/null 2>&1; then
            log_warn "Port $TEST_BRIDGE_PORT occupied by unknown process"
            kill_port_holder
        fi

        # Build the test bridge
        log_info "Building test bridge..."
        cargo build -p zbag-app-tauri --features test-bridge

        # Start the test bridge in the background
        log_info "Starting test bridge..."
        cargo run -p zbag-app-tauri --features test-bridge &
        TEST_BRIDGE_PID=$!

        # Wait for the test bridge to be ready
        if ! wait_for_test_bridge; then
            log_error "Failed to start test bridge"
            exit 1
        fi
    fi

    # Run Playwright tests
    cd "$APP_DIR"

    log_info "Running Playwright tests..."
    bunx playwright test "${PLAYWRIGHT_ARGS[@]}"

    log_info "E2E tests completed successfully!"
}

main
