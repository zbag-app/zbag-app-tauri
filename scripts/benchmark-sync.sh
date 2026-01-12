#!/usr/bin/env bash
#
# benchmark-sync.sh - Benchmark Zcash wallet sync performance
#
# Usage:
#   ./scripts/benchmark-sync.sh [--network mainnet|testnet] [--birthday YYYY-MM-DD] [--cleanup] [--runs N] [--verbose]
#
# Creates a wallet with a 1-year-old birthday (configurable) and measures sync time.
#
set -euo pipefail

# ==============================================================================
# Configuration
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# CLI binary path (prefer release build)
CLI="${ROOT_DIR}/target/release/zstash"
if [[ ! -x "$CLI" ]]; then
    CLI="${ROOT_DIR}/target/debug/zstash"
fi

# Benchmark data directory (isolated from user wallets)
BENCHMARK_DATA_DIR="${HOME}/.zstash-benchmark"

# Test mnemonic (all-zeros entropy - safe for testnet only)
TESTNET_TEST_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"

# Benchmark parameters
BIRTHDAY="2025-01-06"
NETWORK="testnet"
SERVER_URL=""
PASSWORD="benchmark-test-password"

# ==============================================================================
# Argument parsing
# ==============================================================================

CLEANUP=false
RUNS=1
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --network|-N)
            NETWORK="$2"
            shift 2
            ;;
        --birthday)
            BIRTHDAY="$2"
            shift 2
            ;;
        --server)
            SERVER_URL="$2"
            shift 2
            ;;
        --data-dir)
            BENCHMARK_DATA_DIR="$2"
            shift 2
            ;;
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
            echo "Usage: $0 [--network mainnet|testnet] [--birthday YYYY-MM-DD] [--server URL] [--data-dir DIR] [--cleanup] [--runs N] [--verbose]"
            echo ""
            echo "Benchmark Zcash wallet sync performance."
            echo ""
            echo "Options:"
            echo "  --network, -N  Network to benchmark (mainnet or testnet, default: testnet)"
            echo "  --birthday     Birthday date (YYYY-MM-DD, default: $BIRTHDAY)"
            echo "  --server URL   Set default lightwalletd server URL for this benchmark data-dir"
            echo "  --data-dir DIR Benchmark data directory (default: $BENCHMARK_DATA_DIR)"
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
    local wallet_dir="${BENCHMARK_DATA_DIR}/wallets/${NETWORK}/${wallet_id}"
    if [[ -d "$wallet_dir" ]]; then
        log "Cleaning up wallet directory..."
        # Print command for user to run (per CLAUDE.md convention)
        echo "To remove: rm -rf \"$wallet_dir\""
    fi
}

network_to_title() {
    case "$1" in
        mainnet) echo "Mainnet" ;;
        testnet) echo "Testnet" ;;
        *) echo "" ;;
    esac
}

validate_network() {
    case "$NETWORK" in
        mainnet|testnet) ;;
        *)
            die "Invalid network: $NETWORK (expected: mainnet or testnet)"
            ;;
    esac
}

validate_birthday() {
    if ! [[ "$BIRTHDAY" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
        die "Invalid birthday: $BIRTHDAY (expected: YYYY-MM-DD)"
    fi
}

resolve_default_server_url() {
    local network_title
    network_title="$(network_to_title "$NETWORK")"
    if [[ -z "$network_title" ]]; then
        echo ""
        return 0
    fi

    local servers_json
    servers_json="$("$CLI" --json server list --data-dir "$BENCHMARK_DATA_DIR" 2>/dev/null || true)"
    if [[ -z "$servers_json" ]]; then
        echo ""
        return 0
    fi

    echo "$servers_json" | jq -r \
        --arg net "$network_title" \
        '.[] | select(.network == $net and .is_default == true) | .grpc_url' \
        | head -n 1
}

set_default_server_url() {
    local url="$1"
    local network_title
    network_title="$(network_to_title "$NETWORK")"

    if [[ -z "$network_title" ]]; then
        die "Invalid network: $NETWORK"
    fi

    # Ensure app DB exists and servers are seeded.
    mkdir -p "$BENCHMARK_DATA_DIR"

    local servers_json matching_id matching_network
    servers_json="$("$CLI" --json server list --data-dir "$BENCHMARK_DATA_DIR" 2>/dev/null || true)"

    matching_id="$(echo "$servers_json" | jq -r --arg url "$url" '.[] | select(.grpc_url == $url) | .id' | head -n 1)"
    matching_network="$(echo "$servers_json" | jq -r --arg url "$url" '.[] | select(.grpc_url == $url) | .network' | head -n 1)"

    if [[ -n "$matching_id" && "$matching_id" != "null" ]]; then
        if [[ -n "$matching_network" && "$matching_network" != "$network_title" ]]; then
            die "Server URL '$url' is for $matching_network, but --network is $network_title"
        fi

        "$CLI" --json server set-default "$matching_id" --data-dir "$BENCHMARK_DATA_DIR" >/dev/null
        return 0
    fi

    # Add the server (will probe and determine network), then set default.
    local add_out server_id server_net
    add_out="$("$CLI" --json server add --name "benchmark" --url "$url" --data-dir "$BENCHMARK_DATA_DIR" 2>/dev/null)" || die "Failed to add server '$url'"
    server_id="$(echo "$add_out" | jq -r '.server.id' || true)"
    server_net="$(echo "$add_out" | jq -r '.server.network' || true)"

    if [[ -z "$server_id" || "$server_id" == "null" ]]; then
        die "Failed to parse server ID after adding '$url'"
    fi
    if [[ -n "$server_net" && "$server_net" != "null" && "$server_net" != "$network_title" ]]; then
        die "Server URL '$url' is for $server_net, but --network is $network_title"
    fi

    "$CLI" --json server set-default "$server_id" --data-dir "$BENCHMARK_DATA_DIR" >/dev/null
}

generate_seed_phrase() {
    (
        set -euo pipefail
        local tmp_dir out seed
        tmp_dir="$(mktemp -d -t zstash-seedgen.XXXXXX)"
        trap 'rm -rf "$tmp_dir"' EXIT

        # wallet create outputs the seed phrase; keep it in-memory only and never print it.
        out="$("$CLI" wallet create \
            --json \
            --name "seedgen-$(date +%Y%m%d-%H%M%S)" \
            --network "$NETWORK" \
            --password "$PASSWORD" \
            --data-dir "$tmp_dir" 2>/dev/null)" || die "Failed to generate seed phrase"

        seed="$(echo "$out" | jq -r '.seed_phrase | join(" ")' || true)"
        if [[ -z "$seed" || "$seed" == "null" ]]; then
            die "Failed to parse generated seed phrase"
        fi

        echo "$seed"
    )
}

analyze_eta_accuracy() {
    local progress_log_file="$1"
    local total_duration="$2"

    # Aggregate accuracy across all samples with numeric ETA.
    local stats
    stats="$(awk -v total="$total_duration" '
        function abs(x){return x<0?-x:x}
        BEGIN{n=0; sum_abs=0; max_abs=0}
        {
            # Expect tokens like: [HH:MM:SS]  12% | ... | ETA ... (123s)
            t=$1;
            pct=$2;
            if (t !~ /^\[[0-9][0-9]:[0-9][0-9]:[0-9][0-9]\]$/) next;
            if (pct !~ /^[0-9]+%$/) next;
            if (!match($0, /\([0-9]+s\)/)) next;

            # elapsed seconds
            gsub(/^\[/, "", t);
            gsub(/\]$/, "", t);
            split(t, parts, ":");
            elapsed=(parts[1]+0)*3600+(parts[2]+0)*60+(parts[3]+0);

            # eta seconds (strip surrounding parentheses and trailing s)
            eta_str=substr($0, RSTART+1, RLENGTH-2);
            sub(/s$/, "", eta_str);
            eta=eta_str+0;

            remaining=total-elapsed;
            if (remaining < 0) remaining=0;
            err=eta-remaining;
            ae=abs(err);
            n++;
            sum_abs+=ae;
            if (ae > max_abs) max_abs=ae;
        }
        END{
            if (n>0) {
                mean=sum_abs/n;
                printf("%d %.1f %d", n, mean, max_abs);
            }
        }' "$progress_log_file")"

    if [[ -z "$stats" ]]; then
        echo "  ETA samples:      0 (no numeric ETA lines found)"
        return 0
    fi

    local n mean max_abs
    n="$(echo "$stats" | awk '{print $1}')"
    mean="$(echo "$stats" | awk '{print $2}')"
    max_abs="$(echo "$stats" | awk '{print $3}')"

    echo "  ETA samples:      $n"
    echo "  ETA mean |err|:   ${mean}s"
    echo "  ETA max  |err|:   ${max_abs}s"

    # Snapshot accuracy at key percent marks (first sample at/after each bucket).
    for bucket in 25 50 75; do
        local snap
        snap="$(awk -v total="$total_duration" -v bucket="$bucket" '
            function abs(x){return x<0?-x:x}
            {
                t=$1;
                pct=$2;
                if (t !~ /^\[[0-9][0-9]:[0-9][0-9]:[0-9][0-9]\]$/) next;
                if (pct !~ /^[0-9]+%$/) next;
                if (!match($0, /\([0-9]+s\)/)) next;

                pctv=pct;
                sub(/%$/, "", pctv);
                pctv=pctv+0;
                if (pctv < bucket) next;

                gsub(/^\[/, "", t);
                gsub(/\]$/, "", t);
                split(t, parts, ":");
                elapsed=(parts[1]+0)*3600+(parts[2]+0)*60+(parts[3]+0);

                eta_str=substr($0, RSTART+1, RLENGTH-2);
                sub(/s$/, "", eta_str);
                eta=eta_str+0;

                remaining=total-elapsed;
                if (remaining < 0) remaining=0;
                err=eta-remaining;
                ae=abs(err);
                printf("%d %d %d %d %d", pctv, eta, remaining, err, ae);
                exit;
            }' "$progress_log_file")"

        if [[ -n "$snap" ]]; then
            local pct eta rem err ae
            pct="$(echo "$snap" | awk '{print $1}')"
            eta="$(echo "$snap" | awk '{print $2}')"
            rem="$(echo "$snap" | awk '{print $3}')"
            err="$(echo "$snap" | awk '{print $4}')"
            ae="$(echo "$snap" | awk '{print $5}')"
            printf "  ETA @%3s%%:       pred %'ds, actual %'ds, err %'ds (|err| %'ds)\n" "$pct" "$eta" "$rem" "$err" "$ae"
        fi
    done
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
    die "zstash CLI binary not found. Run 'make cli' first."
fi

if ! command -v jq >/dev/null 2>&1; then
    die "jq is required for JSON parsing. Install with: brew install jq"
fi

if ! command -v bc >/dev/null 2>&1; then
    die "bc is required for calculations. Install with: brew install bc"
fi

validate_network
validate_birthday

# Ensure benchmark data directory exists
mkdir -p "$BENCHMARK_DATA_DIR"

# Optional server override (sets benchmark data-dir default server for the selected network).
if [[ -n "${SERVER_URL}" ]]; then
    log "Setting default server for $NETWORK: $SERVER_URL"
    set_default_server_url "$SERVER_URL"
fi

# ==============================================================================
# Main benchmark function
# ==============================================================================

run_benchmark() {
    local run_num="$1"
    local wallet_name="bench-$(date +%Y%m%d-%H%M%S)-${run_num}"
    local wallet_id=""
    local birthday_height=""
    local progress_log_file=""
    local seed_phrase=""

    echo ""
    log "=== Benchmark Run $run_num ==="
    log "Wallet name: $wallet_name"
    log "Birthday: $BIRTHDAY"

    # Step 1: Restore wallet
    log "Restoring wallet..."

    if [[ "$NETWORK" == "testnet" ]]; then
        seed_phrase="$TESTNET_TEST_MNEMONIC"
    else
        seed_phrase="$(generate_seed_phrase)"
    fi

    local restore_output
    restore_output=$("$CLI" wallet restore \
        --json \
        --name "$wallet_name" \
        --network "$NETWORK" \
        --birthday "$BIRTHDAY" \
        --password "$PASSWORD" \
        --seed "$seed_phrase" \
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

    progress_log_file="$(mktemp -t zstash-sync-progress.XXXXXX)"
    trap 'rm -f "$progress_log_file"' RETURN

    # Sync command blocks until complete. Always enable progress logging so we can
    # parse the actual chain tip (testnet block times are not stable enough for
    # genesis+75s estimates).
    if [[ "$VERBOSE" == "true" ]]; then
        "$CLI" sync "$wallet_id" \
            --password "$PASSWORD" \
            --data-dir "$BENCHMARK_DATA_DIR" \
            --progress-log 2>&1 | tee "$progress_log_file" || die "Sync failed"
    else
        "$CLI" sync "$wallet_id" \
            --password "$PASSWORD" \
            --data-dir "$BENCHMARK_DATA_DIR" \
            --progress-log >"$progress_log_file" 2>&1 || die "Sync failed"
    fi

    end_time=$(date +%s)
    sync_duration=$((end_time - start_time))

    # Step 3: Parse actual chain tip from progress log (right-hand side of "current / tip").
    local last_progress_line height_pair height_pair_clean end_height
    last_progress_line="$(grep -E "\\|[[:space:]]*[0-9,]+[[:space:]]*/[[:space:]]*[0-9,]+[[:space:]]*\\|" "$progress_log_file" | tail -n 1 || true)"
    height_pair="$(echo "$last_progress_line" | awk -F'|' '{print $3}' || true)"
    height_pair_clean="$(echo "$height_pair" | tr -cd '0-9/,' | tr -d ',' || true)"
    end_height="$(echo "$height_pair_clean" | cut -d/ -f2 || true)"

    if [[ -z "$end_height" ]]; then
        die "Failed to parse chain tip height from progress log"
    fi

    # Calculate blocks synced
    local blocks_synced=0
    if [[ "$birthday_height" != "unknown" ]]; then
        blocks_synced=$((end_height - birthday_height))
    fi

    # Calculate speed (blocks per second)
    local blocks_per_second="N/A"
    if [[ $sync_duration -gt 0 && $blocks_synced -gt 0 ]]; then
        blocks_per_second=$(echo "scale=2; $blocks_synced / $sync_duration" | bc)
    fi

    # Step 4: Output results
    local server_used
    server_used="$(resolve_default_server_url)"
    if [[ -z "$server_used" ]]; then
        server_used="unknown"
    fi

    echo ""
    echo "================================================================================"
    echo "                    Zcash Wallet Sync Benchmark Results"
    echo "================================================================================"
    echo ""
    echo "Configuration:"
    echo "  Network:          $NETWORK"
    echo "  Birthday:         $BIRTHDAY"
    echo "  Server:           $server_used"
    echo "  Tor:              disabled"
    echo ""
    echo "Results:"
    if [[ "$birthday_height" != "unknown" ]]; then
        printf "  Start Height:     %'d\n" "$birthday_height"
        printf "  End Height:       %'d\n" "$end_height"
        printf "  Blocks Synced:    %'d\n" "$blocks_synced"
    else
        echo "  Start Height:     unknown"
        printf "  End Height:       %'d\n" "$end_height"
    fi
    echo "  Total Time:       $(format_duration "$sync_duration") ($sync_duration seconds)"
    echo "  Avg Speed:        $blocks_per_second blocks/second"
    echo ""
    echo "ETA Accuracy:"
    analyze_eta_accuracy "$progress_log_file" "$sync_duration"
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
echo "zSTASH Sync Benchmark"
echo "===================="
log "Using CLI: $CLI"
log "Data directory: $BENCHMARK_DATA_DIR"
log "Runs: $RUNS"
log "Network: $NETWORK"
if [[ -n "${SERVER_URL}" ]]; then
    log "Server: $SERVER_URL"
fi

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
