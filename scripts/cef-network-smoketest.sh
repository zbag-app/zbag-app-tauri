#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="${1:-$ROOT/target/release/bundle/macos/bagZ.app}"
SMOKE_DURATION_SECS="${BAGZ_SMOKE_DURATION_SECS:-15}"
LSOF_TIMEOUT_SECS="${BAGZ_LSOF_TIMEOUT_SECS:-3}"
SMOKE_LOG="${RUNNER_TEMP:-/tmp}/bagz-cef-smoketest.log"
SMOKE_ROOT=""
RUN_LOG=""
APP_PID=""
WATCHDOG_PID=""
SAMPLER_PID=""
COPIED=0

if ! [[ "$SMOKE_DURATION_SECS" =~ ^[0-9]+$ ]] || [[ "$SMOKE_DURATION_SECS" -eq 0 ]]; then
  echo "error: BAGZ_SMOKE_DURATION_SECS must be a positive integer" >&2
  exit 2
fi
if ! [[ "$LSOF_TIMEOUT_SECS" =~ ^[0-9]+$ ]] || [[ "$LSOF_TIMEOUT_SECS" -eq 0 ]]; then
  echo "error: BAGZ_LSOF_TIMEOUT_SECS must be a positive integer" >&2
  exit 2
fi

log() {
  local line
  line="$(date -u '+%Y-%m-%dT%H:%M:%SZ') $*"
  if [[ -n "$RUN_LOG" ]]; then
    printf '%s\n' "$line" >>"$RUN_LOG"
  fi
  printf '%s\n' "$line" >&2
}

copy_log() {
  if [[ "$COPIED" -eq 1 ]]; then
    return 0
  fi

  mkdir -p "$(dirname "$SMOKE_LOG")"
  if [[ -n "$RUN_LOG" && -f "$RUN_LOG" ]]; then
    cp "$RUN_LOG" "$SMOKE_LOG"
  else
    : >"$SMOKE_LOG"
  fi
  COPIED=1
}

enumerate_descendants() {
  local parent="$1"
  local kids
  kids="$(pgrep -P "$parent" 2>/dev/null || true)"
  local kid
  for kid in $kids; do
    printf '%s\n' "$kid"
    enumerate_descendants "$kid"
  done
}

kill_tree() {
  local root_pid="${1:-${APP_PID:-}}"
  if [[ -z "$root_pid" ]]; then
    return 0
  fi

  local pids
  pids="$({ enumerate_descendants "$root_pid"; printf '%s\n' "$root_pid"; } | sort -ur || true)"
  local pid
  for pid in $pids; do
    kill -KILL "$pid" 2>/dev/null || true
  done
}

cleanup() {
  local status=$?

  if [[ -n "${WATCHDOG_PID:-}" ]]; then
    kill "$WATCHDOG_PID" 2>/dev/null || true
  fi
  if [[ -n "${SAMPLER_PID:-}" ]]; then
    kill "$SAMPLER_PID" 2>/dev/null || true
  fi
  if [[ -n "${APP_PID:-}" ]]; then
    kill_tree "$APP_PID"
  fi

  copy_log

  if [[ -n "$SMOKE_ROOT" ]]; then
    rm -rf "$SMOKE_ROOT"
  fi

  printf '%s\n' "$SMOKE_LOG"
  return "$status"
}
trap cleanup EXIT

endpoint_host() {
  local endpoint="$1"
  endpoint="${endpoint%% *}"

  if [[ "$endpoint" == \[*\]:* ]]; then
    endpoint="${endpoint#\[}"
    printf '%s\n' "${endpoint%%\]*}"
    return 0
  fi

  if [[ "$endpoint" == *:* ]]; then
    printf '%s\n' "${endpoint%:*}"
  else
    printf '%s\n' "$endpoint"
  fi
}

is_loopback_host() {
  local host="$1"
  host="${host#[}"
  host="${host%]}"

  [[ "$host" == 127.* || "$host" == "::1" ]]
}

record_socket_failure() {
  local sample="$1"
  local reason="$2"
  local pid="$3"
  local cmd="$4"
  local proto="$5"
  local state="$6"
  local name="$7"

  if [[ "${SUPPRESS_CLASSIFIER_FAILURE_LOG:-0}" != "1" ]]; then
    log "FAIL: $reason sample=$sample pid=$pid cmd=$cmd proto=$proto state=$state name=$name"
  fi
}

classify_socket() {
  local sample="$1"
  local pid="$2"
  local cmd="$3"
  local proto="$4"
  local state="$5"
  local name="$6"

  if [[ "$name" == *"->"* ]]; then
    local remote="${name##*->}"
    local remote_host
    remote_host="$(endpoint_host "$remote")"
    if is_loopback_host "$remote_host"; then
      return 0
    fi
    record_socket_failure "$sample" "non-loopback remote" "$pid" "$cmd" "$proto" "$state" "$name"
    return 1
  fi

  local bind_host
  bind_host="$(endpoint_host "$name")"
  if is_loopback_host "$bind_host"; then
    return 0
  fi

  record_socket_failure "$sample" "non-loopback bind" "$pid" "$cmd" "$proto" "$state" "$name"
  return 1
}

classify_lsof_fields() {
  local sample="$1"
  local pid=""
  local cmd=""
  local proto=""
  local state=""
  local name=""
  local failed=0
  local field

  while IFS= read -r field || [[ -n "$field" ]]; do
    [[ -z "$field" ]] && continue

    case "$field" in
      p*)
        pid="${field#p}"
        ;;
      c*)
        cmd="${field#c}"
        ;;
      P*)
        proto="${field#P}"
        ;;
      TST=*)
        state="${field#TST=}"
        ;;
      T*)
        ;;
      n*)
        name="${field#n}"
        if ! classify_socket "$sample" "$pid" "$cmd" "$proto" "$state" "$name"; then
          failed=1
        fi
        proto=""
        state=""
        name=""
        ;;
    esac
  done

  return "$failed"
}

fixture_stream() {
  case "$1" in
    loopback-listener)
      printf 'p1234\0cbagZ\0PTCP\0TST=LISTEN\0n127.0.0.1:7777\0'
      ;;
    wildcard-listener)
      printf 'p1234\0cbagZ\0PTCP\0TST=LISTEN\0n*:7777\0'
      ;;
    zero-listener)
      printf 'p1234\0cbagZ\0PTCP\0TST=LISTEN\0n0.0.0.0:7777\0'
      ;;
    external-connected)
      printf 'p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->142.250.190.78:443\0'
      ;;
    loopback-connected)
      printf 'p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->127.0.0.1:7777\0'
      ;;
    *)
      echo "unknown fixture: $1" >&2
      return 2
      ;;
  esac | tr '\0' '\n'
}

run_fixture() {
  local name="$1"
  local expected="$2"
  local status

  set +e
  if [[ "$expected" == "fail" ]]; then
    SUPPRESS_CLASSIFIER_FAILURE_LOG=1
    fixture_stream "$name" | classify_lsof_fields "selftest:$name"
    status=$?
    unset SUPPRESS_CLASSIFIER_FAILURE_LOG
  else
    fixture_stream "$name" | classify_lsof_fields "selftest:$name"
    status=$?
  fi
  set -e

  case "$expected:$status" in
    pass:0|fail:1)
      log "PASS: parser fixture $name"
      ;;
    *)
      log "FAIL: parser fixture $name expected $expected, got status $status"
      return 1
      ;;
  esac
}

filter_live_pids() {
  local csv="$1"
  local live=()
  local pid
  local pids
  IFS=',' read -ra pids <<<"$csv"
  for pid in "${pids[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
      live+=("$pid")
    fi
  done
  (IFS=,; printf '%s\n' "${live[*]}")
}

run_filter_live_pids_fixture() {
  local label="$1"
  local expected="$2"
  local input="$3"
  local got
  got="$(filter_live_pids "$input")"
  if [[ "$got" == "$expected" ]]; then
    log "PASS: filter_live_pids fixture $label"
  else
    log "FAIL: filter_live_pids fixture $label expected=$expected got=$got"
    return 1
  fi
}

run_sample_sockets_fixture() {
  local label="$1"
  local expected_rc="$2"
  local stub_script="$3"

  local fixture_timeout=""
  if [[ -n "${BAGZ_LSOF_TIMEOUT_SECS_FIXTURE:-}" ]]; then
    if ! [[ "$BAGZ_LSOF_TIMEOUT_SECS_FIXTURE" =~ ^[0-9]+$ ]] \
       || [[ "$BAGZ_LSOF_TIMEOUT_SECS_FIXTURE" -eq 0 ]]; then
      log "FAIL: fixture $label has invalid BAGZ_LSOF_TIMEOUT_SECS_FIXTURE=$BAGZ_LSOF_TIMEOUT_SECS_FIXTURE"
      return 1
    fi
    fixture_timeout="$BAGZ_LSOF_TIMEOUT_SECS_FIXTURE"
  fi

  local stub_dir
  stub_dir="$(mktemp -d "${TMPDIR:-/tmp}/bagz-stub.XXXXXX")"
  printf '%s\n' "$stub_script" >"$stub_dir/lsof"
  chmod +x "$stub_dir/lsof"

  local prior_path="$PATH"
  local prior_app_pid="${APP_PID:-}"
  local prior_smoke_root="${SMOKE_ROOT:-}"
  local prior_lsof_timeout="$LSOF_TIMEOUT_SECS"
  PATH="$stub_dir:$PATH"
  APP_PID=$$
  SMOKE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bagz-smoke-fixture.XXXXXX")"
  if [[ -n "$fixture_timeout" ]]; then
    LSOF_TIMEOUT_SECS="$fixture_timeout"
  fi

  local prior_enumerate_def=""
  if [[ -n "${BAGZ_FIXTURE_FAKE_DEAD_DESCENDANT:-}" ]]; then
    prior_enumerate_def="$(declare -f enumerate_descendants)"
    enumerate_descendants() {
      printf '%s\n' "$1"
      printf '%s\n' "$BAGZ_FIXTURE_FAKE_DEAD_DESCENDANT"
    }
  fi

  export BAGZ_FIXTURE_STATE_DIR="$SMOKE_ROOT"

  local got_rc
  if sample_sockets "fixture-$label"; then
    got_rc=0
  else
    got_rc=$?
  fi

  unset BAGZ_FIXTURE_STATE_DIR
  if [[ -n "$prior_enumerate_def" ]]; then
    unset -f enumerate_descendants
    eval "$prior_enumerate_def"
  fi
  rm -rf "$SMOKE_ROOT" "$stub_dir"
  PATH="$prior_path"
  APP_PID="$prior_app_pid"
  SMOKE_ROOT="$prior_smoke_root"
  LSOF_TIMEOUT_SECS="$prior_lsof_timeout"

  if [[ "$got_rc" -eq "$expected_rc" ]]; then
    log "PASS: sample_sockets fixture $label (rc=$got_rc)"
    return 0
  fi
  log "FAIL: sample_sockets fixture $label expected=$expected_rc got=$got_rc"
  return 1
}

run_selftest() {
  RUN_LOG="$(mktemp "${TMPDIR:-/tmp}/bagz-cef-smoketest-selftest.XXXXXX")"
  log "running CEF network smoke parser self-test"

  run_fixture loopback-listener pass
  run_fixture wildcard-listener fail
  run_fixture zero-listener fail
  run_fixture external-connected fail
  run_fixture loopback-connected pass

  sleep 0.01 &
  local dead_pid=$!
  wait "$dead_pid" 2>/dev/null || true
  run_filter_live_pids_fixture all-live "$$" "$$"
  run_filter_live_pids_fixture mixed "$$" "$$,$dead_pid"
  run_filter_live_pids_fixture all-dead "" "$dead_pid"

  run_sample_sockets_fixture evidence-with-nonzero-status 1 '#!/usr/bin/env bash
printf "p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->142.250.190.78:443\0"
echo "lsof: synthetic stderr about a dead PID" >&2
exit 1'

  run_sample_sockets_fixture clean-with-nonzero-status-no-race 2 '#!/usr/bin/env bash
printf "p1234\0cbagZ\0PTCP\0TST=LISTEN\0n127.0.0.1:7777\0"
echo "lsof: synthetic generic error" >&2
exit 1'

  BAGZ_LSOF_TIMEOUT_SECS_FIXTURE=1 \
    run_sample_sockets_fixture timeout-no-evidence 2 '#!/usr/bin/env bash
exec sleep 30'

  BAGZ_LSOF_TIMEOUT_SECS_FIXTURE=1 \
    run_sample_sockets_fixture timeout-with-evidence 1 '#!/usr/bin/env bash
printf "p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->142.250.190.78:443\0"
exec sleep 30'

  local dead_descendant_pid
  dead_descendant_pid="$(
    (:) &
    pid=$!
    wait "$pid" 2>/dev/null || true
    printf '%s' "$pid"
  )"
  BAGZ_FIXTURE_FAKE_DEAD_DESCENDANT="$dead_descendant_pid" \
    run_sample_sockets_fixture retry-benign-no-matching-files 0 '#!/usr/bin/env bash
counter_file="$BAGZ_FIXTURE_STATE_DIR/lsof-calls"
count=$(cat "$counter_file" 2>/dev/null || echo 0)
echo $((count + 1)) >"$counter_file"
if [[ "$count" -eq 0 ]]; then
  printf "p1234\0cbagZ\0PTCP\0TST=LISTEN\0n127.0.0.1:7777\0"
  echo "lsof: synthetic stderr forcing race retry" >&2
  exit 1
else
  exit 1
fi'

  log "PASS: CEF network smoke parser self-test"
}

run_lsof_with_timeout() {
  local timeout_secs="$1"
  local pids="$2"
  local raw="$3"
  local stderr_path="$4"
  local timeout_sentinel="$5"

  rm -f "$timeout_sentinel"

  lsof -nP -a -p "$pids" -iTCP -iUDP -F pcPTn0 >"$raw" 2>"$stderr_path" &
  local lsof_pid=$!

  (
    waited=0
    while [[ "$waited" -lt "$timeout_secs" ]]; do
      sleep 1
      waited=$((waited + 1))
      if ! kill -0 "$lsof_pid" 2>/dev/null; then
        exit 0
      fi
    done
    touch "$timeout_sentinel"
    kill -TERM "$lsof_pid" 2>/dev/null || true
    sleep 1
    kill -KILL "$lsof_pid" 2>/dev/null || true
  ) &
  local killer_pid=$!

  local status
  if wait "$lsof_pid"; then
    status=0
  else
    status=$?
  fi

  kill "$killer_pid" 2>/dev/null || true
  wait "$killer_pid" 2>/dev/null || true

  return "$status"
}

sample_sockets() {
  local sample="$1"
  local pid_csv
  pid_csv="$({ enumerate_descendants "$APP_PID"; printf '%s\n' "$APP_PID"; } | awk 'NF' | sort -u | paste -sd, -)"

  if [[ -z "$pid_csv" ]]; then
    return 0
  fi

  local raw_file="$SMOKE_ROOT/lsof.raw.$sample"
  local stderr_file="$SMOKE_ROOT/lsof.stderr.$sample"
  local timeout_sentinel="$SMOKE_ROOT/lsof.timedout.$sample"
  local status

  if run_lsof_with_timeout "$LSOF_TIMEOUT_SECS" \
       "$pid_csv" "$raw_file" "$stderr_file" "$timeout_sentinel"; then
    status=0
  else
    status=$?
  fi

  local classify_rc=0
  if [[ -s "$raw_file" ]]; then
    local output
    output="$(tr '\0' '\n' <"$raw_file")"
    if [[ -n "$output" ]]; then
      if classify_lsof_fields "$sample" <<<"$output"; then
        classify_rc=0
      else
        classify_rc=$?
      fi
    fi
  fi
  if [[ "$classify_rc" -eq 1 ]]; then
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 1
  fi

  if [[ -f "$timeout_sentinel" ]]; then
    log "ERROR: lsof timed out sample=$sample timeout=${LSOF_TIMEOUT_SECS}s"
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 2
  fi
  if [[ "$status" -eq 0 ]]; then
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 0
  fi
  if [[ ! -s "$stderr_file" ]]; then
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 0
  fi

  local live_csv
  live_csv="$(filter_live_pids "$pid_csv")"
  if [[ -z "$live_csv" ]]; then
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 0
  fi
  if [[ "$live_csv" == "$pid_csv" ]]; then
    log "ERROR: lsof instrumentation failed sample=$sample status=$status stderr=$(tr '\n' ' ' <"$stderr_file")"
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 2
  fi

  local retry_raw="$SMOKE_ROOT/lsof.retry.raw.$sample"
  local retry_stderr="$SMOKE_ROOT/lsof.retry.stderr.$sample"
  local retry_timeout_sentinel="$SMOKE_ROOT/lsof.retry.timedout.$sample"
  local retry_status

  if run_lsof_with_timeout "$LSOF_TIMEOUT_SECS" \
       "$live_csv" "$retry_raw" "$retry_stderr" "$retry_timeout_sentinel"; then
    retry_status=0
  else
    retry_status=$?
  fi

  local retry_classify_rc=0
  if [[ -s "$retry_raw" ]]; then
    local retry_output
    retry_output="$(tr '\0' '\n' <"$retry_raw")"
    if [[ -n "$retry_output" ]]; then
      if classify_lsof_fields "$sample" <<<"$retry_output"; then
        retry_classify_rc=0
      else
        retry_classify_rc=$?
      fi
    fi
  fi

  local retry_timed_out=0
  if [[ -f "$retry_timeout_sentinel" ]]; then
    retry_timed_out=1
  fi
  local retry_stderr_empty=1
  if [[ -s "$retry_stderr" ]]; then
    retry_stderr_empty=0
  fi

  rm -f "$raw_file" "$stderr_file" "$timeout_sentinel" \
        "$retry_raw" "$retry_stderr" "$retry_timeout_sentinel"

  if [[ "$retry_classify_rc" -eq 1 ]]; then
    return 1
  fi
  if [[ "$retry_timed_out" -eq 1 ]]; then
    log "ERROR: lsof timed out (retry) sample=$sample"
    return 2
  fi
  if [[ "$retry_status" -eq 0 ]]; then
    return 0
  fi
  if [[ "$retry_stderr_empty" -eq 1 ]]; then
    return 0
  fi
  log "ERROR: lsof instrumentation failed (retry) sample=$sample status=$retry_status"
  return 2
}

bundle_executable() {
  local bundle="$1"
  local macos_dir="$bundle/Contents/MacOS"
  local plist="$bundle/Contents/Info.plist"
  local executable_name=""

  if [[ -f "$plist" && -x /usr/libexec/PlistBuddy ]]; then
    executable_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$plist" 2>/dev/null || true)"
    if [[ -n "$executable_name" && -x "$macos_dir/$executable_name" ]]; then
      printf '%s\n' "$macos_dir/$executable_name"
      return 0
    fi
  fi

  local candidate
  local found=""
  local count=0
  for candidate in "$macos_dir"/*; do
    if [[ -f "$candidate" && -x "$candidate" ]]; then
      found="$candidate"
      count=$((count + 1))
    fi
  done

  if [[ "$count" -eq 1 ]]; then
    printf '%s\n' "$found"
    return 0
  fi

  return 1
}

sampler_loop() {
  local sample=0
  local rc

  while kill -0 "$APP_PID" 2>/dev/null; do
    sample=$((sample + 1))
    if sample_sockets "sample-$sample"; then
      rc=0
    else
      rc=$?
    fi
    if [[ "$rc" -eq 1 ]]; then
      touch "$SMOKE_ROOT/network-failure"
    elif [[ "$rc" -eq 2 ]]; then
      touch "$SMOKE_ROOT/instrumentation-failure"
    fi
    sleep 1
  done
}

run_smoke() {
  local os_name
  os_name="$(uname -s)"

  SMOKE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bagz-cef-smoketest.XXXXXX")"
  RUN_LOG="$SMOKE_ROOT/run.log"
  : >"$RUN_LOG"

  if [[ "$os_name" != "Darwin" ]]; then
    log "smoke not implemented for $os_name"
    return 0
  fi

  if ! command -v lsof >/dev/null 2>&1; then
    log "error: lsof is required on Darwin"
    return 2
  fi

  local app_exe
  if ! app_exe="$(bundle_executable "$APP_BUNDLE")"; then
    log "error: bundled app executable not found in: $APP_BUNDLE/Contents/MacOS"
    return 2
  fi

  mkdir -p \
    "$SMOKE_ROOT/home" \
    "$SMOKE_ROOT/cache" \
    "$SMOKE_ROOT/config" \
    "$SMOKE_ROOT/data" \
    "$SMOKE_ROOT/state" \
    "$SMOKE_ROOT/tmp"

  export HOME="$SMOKE_ROOT/home"
  export XDG_CACHE_HOME="$SMOKE_ROOT/cache"
  export XDG_CONFIG_HOME="$SMOKE_ROOT/config"
  export XDG_DATA_HOME="$SMOKE_ROOT/data"
  export XDG_STATE_HOME="$SMOKE_ROOT/state"
  export TMPDIR="$SMOKE_ROOT/tmp"
  export BAGZ_GRPC_URL="https://127.0.0.1:1"
  export BAGZ_HEADLESS_SMOKE=1
  export BAGZ_SMOKE_DURATION_SECS="$SMOKE_DURATION_SECS"
  export BAGZ_SMOKE_READY_FILE="$SMOKE_ROOT/smoke-ready"
  export BAGZ_USE_SYSTEM_KEYCHAIN=0

  local hard_timeout_secs=$((SMOKE_DURATION_SECS + 30))
  local start_ts
  start_ts="$(date +%s)"

  log "starting CEF network smoke: app=$app_exe duration=${SMOKE_DURATION_SECS}s timeout=${hard_timeout_secs}s"
  "$app_exe" >>"$RUN_LOG" 2>&1 &
  APP_PID=$!

  (
    sleep "$hard_timeout_secs"
    log "WATCHDOG: hard timeout after ${hard_timeout_secs}s"
    touch "$SMOKE_ROOT/watchdog-fired"
    kill_tree "$APP_PID"
  ) &
  WATCHDOG_PID=$!

  sampler_loop &
  SAMPLER_PID=$!

  local app_status
  set +e
  wait "$APP_PID"
  app_status=$?
  set -e

  local app_elapsed
  app_elapsed=$(($(date +%s) - start_ts))

  kill "$WATCHDOG_PID" 2>/dev/null || true
  WATCHDOG_PID=""
  wait "$SAMPLER_PID" 2>/dev/null || true
  SAMPLER_PID=""

  local elapsed
  elapsed=$(($(date +%s) - start_ts))
  local lower_bound=$((SMOKE_DURATION_SECS - 2))
  if [[ "$lower_bound" -lt 0 ]]; then
    lower_bound=0
  fi
  local upper_bound=$((SMOKE_DURATION_SECS + 5))

  if [[ -f "$SMOKE_ROOT/watchdog-fired" ]]; then
    log "FAIL: app hung until watchdog fired after ${hard_timeout_secs}s"
    return 2
  fi
  if [[ ! -f "$SMOKE_ROOT/smoke-ready" ]]; then
    log "FAIL: app exited before CEF smoke setup wrote readiness sentinel status=$app_status elapsed=${elapsed}s"
    return 2
  fi
  if (( app_elapsed < lower_bound )); then
    log "FAIL: app exited too early status=$app_status app_elapsed=${app_elapsed}s total_elapsed=${elapsed}s expected_min=${lower_bound}s"
    return 2
  fi
  if (( app_elapsed > upper_bound )); then
    log "FAIL: app exited too late status=$app_status app_elapsed=${app_elapsed}s total_elapsed=${elapsed}s expected_max=${upper_bound}s"
    return 2
  fi
  if (( app_status != 0 )); then
    log "FAIL: app exited non-zero status=$app_status elapsed=${elapsed}s"
    return 2
  fi
  if [[ -f "$SMOKE_ROOT/instrumentation-failure" ]]; then
    log "FAIL: lsof failed during sampling"
    return 2
  fi
  if [[ -f "$SMOKE_ROOT/network-failure" ]]; then
    log "FAIL: non-loopback CEF socket observed"
    return 1
  fi

  log "PASS: CEF network smoke observed no non-loopback sockets"
  return 0
}

if [[ "${BAGZ_SMOKE_SELFTEST:-0}" == "1" ]]; then
  run_selftest
else
  run_smoke
fi
