#!/usr/bin/env bash
#
# benchmark-sync.sh - Benchmark Zcash wallet sync performance
#
# Usage:
#   ./scripts/benchmark-sync.sh [--cleanup] [--runs N] [--verbose]
#
# Creates a testnet wallet with a 1-year-old birthday and measures sync time.
#
set -euo pipefail

# ==============================================================================
# Configuration
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# CLI binary path (prefer release build)
CLI="${ROOT_DIR}/target/release/zkore"
if [[ ! -x "$CLI" ]]; then
    CLI="${ROOT_DIR}/target/debug/zkore"
fi

# Benchmark data directory (isolated from user wallets)
BENCHMARK_DATA_DIR="${HOME}/.zkore-benchmark"

# Test mnemonic (all-zeros entropy - safe for testnet only)
TEST_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"

# Benchmark parameters
BIRTHDAY="2025-01-06"
NETWORK="testnet"
PASSWORD="benchmark-test-password"

# ==============================================================================
# Argument parsing
# ==============================================================================

CLEANUP=false
RUNS=1
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --cleanup)
            CLEANUP=true
            shift
            ;;
        --runs)
            RUNS="$2"
            shift 2
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--cleanup] [--runs N] [--verbose]"
            echo ""
            echo "Benchmark Zcash wallet sync performance on testnet."
            echo ""
            echo "Options:"
            echo "  --cleanup    Remove benchmark wallet after completion"
            echo "  --runs N     Number of benchmark runs (default: 1)"
            echo "  --verbose    Show detailed progress output"
            echo "  --help       Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# ==============================================================================
# Helper functions
# ==============================================================================

log() {
    echo "[$(date +%H:%M:%S)] $*"
}

log_verbose() {
    if [[ "$VERBOSE" == "true" ]]; then
        log "$@"
    fi
}

die() {
    echo "ERROR: $*" >&2
    exit 1
}

cleanup_wallet() {
    local wallet_id="$1"
    local wallet_dir="${BENCHMARK_DATA_DIR}/wallets/testnet/${wallet_id}"
    if [[ -d "$wallet_dir" ]]; then
        log "Cleaning up wallet directory..."
        # Print command for user to run (per CLAUDE.md convention)
        echo "To remove: rm -rf \"$wallet_dir\""
    fi
}

format_duration() {
    local seconds="$1"
    local hours=$((seconds / 3600))
    local minutes=$(((seconds % 3600) / 60))
    local secs=$((seconds % 60))

    if [[ $hours -gt 0 ]]; then
        printf "%dh %dm %ds" "$hours" "$minutes" "$secs"
    elif [[ $minutes -gt 0 ]]; then
        printf "%dm %ds" "$minutes" "$secs"
    else
        printf "%ds" "$secs"
    fi
}

# ==============================================================================
# Prerequisite checks
# ==============================================================================

if [[ ! -x "$CLI" ]]; then
    die "zkore CLI binary not found. Run 'make cli' first."
fi

if ! command -v jq >/dev/null 2>&1; then
    die "jq is required for JSON parsing. Install with: brew install jq"
fi

if ! command -v bc >/dev/null 2>&1; then
    die "bc is required for calculations. Install with: brew install bc"
fi

# Ensure benchmark data directory exists
mkdir -p "$BENCHMARK_DATA_DIR"

# ==============================================================================
# Main benchmark function
# ==============================================================================

run_benchmark() {
    local run_num="$1"
    local wallet_name="bench-$(date +%Y%m%d-%H%M%S)-${run_num}"
    local wallet_id=""
    local birthday_height=""

    echo ""
    log "=== Benchmark Run $run_num ==="
    log "Wallet name: $wallet_name"
    log "Birthday: $BIRTHDAY"

    # Step 1: Restore wallet
    log "Restoring wallet..."
    local restore_output
    restore_output=$("$CLI" wallet restore \
        --json \
        --name "$wallet_name" \
        --network "$NETWORK" \
        --birthday "$BIRTHDAY" \
        --password "$PASSWORD" \
        --seed "$TEST_MNEMONIC" \
        --data-dir "$BENCHMARK_DATA_DIR" 2>&1) || die "Wallet restore failed: $restore_output"

    # Parse wallet ID and birthday height from JSON output
    wallet_id=$(echo "$restore_output" | jq -r '.wallet.id')
    birthday_height=$(echo "$restore_output" | jq -r '.birthday_height')

    log_verbose "Wallet ID: $wallet_id"
    log_verbose "Birthday height: $birthday_height"

    if [[ -z "$wallet_id" || "$wallet_id" == "null" ]]; then
        die "Failed to parse wallet ID from restore output"
    fi

    if [[ -z "$birthday_height" || "$birthday_height" == "null" ]]; then
        birthday_height="unknown"
    fi

    # Step 2: Run sync and measure time
    log "Starting sync from height $birthday_height..."
    log "This may take a while..."
    echo ""

    local start_time end_time sync_duration
    start_time=$(date +%s)

    # Sync command blocks until complete
    if [[ "$VERBOSE" == "true" ]]; then
        "$CLI" sync "$wallet_id" \
            --password "$PASSWORD" \
            --data-dir "$BENCHMARK_DATA_DIR" \
            --progress-log 2>&1 || die "Sync failed"
    else
        "$CLI" sync "$wallet_id" \
            --password "$PASSWORD" \
            --data-dir "$BENCHMARK_DATA_DIR" 2>&1 || die "Sync failed"
    fi

    end_time=$(date +%s)
    sync_duration=$((end_time - start_time))

    # Step 3: Estimate chain tip (using block time formula)
    # Testnet genesis: 2016-10-28 (1477612800)
    local current_height_estimate
    current_height_estimate=$(( ($(date +%s) - 1477612800) * 1000 / 75000 ))

    # Calculate blocks synced
    local blocks_synced=0
    if [[ "$birthday_height" != "unknown" ]]; then
        blocks_synced=$((current_height_estimate - birthday_height))
    fi

    # Calculate speed (blocks per second)
    local blocks_per_second="N/A"
    if [[ $sync_duration -gt 0 && $blocks_synced -gt 0 ]]; then
        blocks_per_second=$(echo "scale=2; $blocks_synced / $sync_duration" | bc)
    fi

    # Step 4: Output results
    echo ""
    echo "================================================================================"
    echo "                    Zcash Wallet Sync Benchmark Results"
    echo "================================================================================"
    echo ""
    echo "Configuration:"
    echo "  Network:          $NETWORK"
    echo "  Birthday:         $BIRTHDAY"
    echo "  Server:           https://lwd.testnet.zec.pro"
    echo "  Tor:              disabled"
    echo ""
    echo "Results:"
    if [[ "$birthday_height" != "unknown" ]]; then
        printf "  Start Height:     %'d\n" "$birthday_height"
        printf "  End Height:       %'d (estimated)\n" "$current_height_estimate"
        printf "  Blocks Synced:    %'d\n" "$blocks_synced"
    else
        echo "  Start Height:     unknown"
        printf "  End Height:       %'d (estimated)\n" "$current_height_estimate"
    fi
    echo "  Total Time:       $(format_duration "$sync_duration") ($sync_duration seconds)"
    echo "  Avg Speed:        $blocks_per_second blocks/second"
    echo ""
    echo "Wallet Info:"
    echo "  Wallet ID:        ${wallet_id:0:8}"
    echo "  Data Directory:   $BENCHMARK_DATA_DIR"
    echo ""
    echo "================================================================================"

    # Step 5: Cleanup if requested
    if [[ "$CLEANUP" == "true" ]]; then
        cleanup_wallet "$wallet_id"
    fi
}

# ==============================================================================
# Execute benchmark
# ==============================================================================

echo ""
echo "Zkore Sync Benchmark"
echo "===================="
log "Using CLI: $CLI"
log "Data directory: $BENCHMARK_DATA_DIR"
log "Runs: $RUNS"

for ((i=1; i<=RUNS; i++)); do
    run_benchmark "$i"

    # Brief pause between runs if multiple
    if [[ $i -lt $RUNS ]]; then
        log "Waiting 5 seconds before next run..."
        sleep 5
    fi
done

echo ""
log "Benchmark complete."
