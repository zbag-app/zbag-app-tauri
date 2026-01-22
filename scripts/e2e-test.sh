#!/bin/bash
#
# E2E Test Runner for zstash
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
APP_DIR="$PROJECT_ROOT/apps/zstash-app-tauri"

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

cleanup() {
    if [ -n "$TEST_BRIDGE_PID" ]; then
        log_info "Stopping test bridge (PID: $TEST_BRIDGE_PID)..."
        kill "$TEST_BRIDGE_PID" 2>/dev/null || true
        wait "$TEST_BRIDGE_PID" 2>/dev/null || true
    fi
}

trap cleanup EXIT

wait_for_test_bridge() {
    local max_attempts=30
    local attempt=1

    log_info "Waiting for test bridge to be ready on port $TEST_BRIDGE_PORT..."

    while [ $attempt -le $max_attempts ]; do
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

main() {
    cd "$PROJECT_ROOT"

    # Check if test bridge is already running
    if curl -s "http://127.0.0.1:$TEST_BRIDGE_PORT/health" > /dev/null 2>&1; then
        log_info "Test bridge already running on port $TEST_BRIDGE_PORT"
    else
        # Build the test bridge
        log_info "Building test bridge..."
        cargo build -p zstash-app-tauri --features test-bridge

        # Start the test bridge in the background
        log_info "Starting test bridge..."
        cargo run -p zstash-app-tauri --features test-bridge &
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
