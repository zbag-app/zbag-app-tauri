# CEF Smoke: classify-first ordering + per-call lsof timeout

## Context

Codex review of commit `35790bd` ("close lsof, duplicate-switch, and live-cache gaps") surfaced two remaining gaps in `scripts/cef-network-smoketest.sh`:

1. **Classification ordering inverts the security posture.** `sample_sockets` (lines 303-354) treats lsof's exit status as the primary signal. When lsof exits non-zero with stderr (e.g., during CEF subprocess churn), the code enters race-retry and either overwrites `$raw_file` (retry with live subset) or removes it (whole tree exited). Any non-loopback socket that lsof captured in stdout *before* erroring is discarded without classification. A real leak observed on a process that died mid-scan is silently dropped → false PASS for a security control.
2. **Sampler wait is unbounded.** After `wait "$APP_PID"`, the watchdog is killed (line 480) and `wait "$SAMPLER_PID"` runs without a bound (line 482). An ordinary slow or stuck-but-killable `lsof` blocks the sampler indefinitely; only the outer CI/make timeout terminates the run.

   Scope note: this fix addresses ordinary slow/stuck `lsof` (the realistic case). D-state kernel wedges where SIGKILL is ineffective remain out of scope. The script's `hard_timeout_secs` watchdog (`SMOKE_DURATION_SECS + 30`) only bounds the app process; after `wait "$APP_PID"` returns, the post-app `wait "$SAMPLER_PID"` is NOT covered by that watchdog and can still block on a kernel-wedged child. The outer CI/make timeout remains the backstop for that pathological case, since a parent-side `wait` cannot interrupt an uninterruptible kernel sleep.

Both fixes land in `scripts/cef-network-smoketest.sh`. No production code change, no new dependencies.

## Fix A: classify the original lsof output before any retry/forgive

**File:** `scripts/cef-network-smoketest.sh` — rewrite `sample_sockets` (currently lines 303-354).

Principle: a non-loopback hit in lsof stdout is real evidence regardless of lsof's exit status. Classify whatever stdout was captured FIRST. Only fall through to race-retry/forgive when the first-pass classification was clean.

Errexit discipline (mandatory throughout): inside `sample_sockets`, every non-zero-capable call MUST go through `if helper; then rc=0; else rc=$?; fi`. Do NOT use `set +e ... cmd ... rc=$?; set -e` blocks: a function that toggles `set -e` and returns non-zero will tear the caller down before `rc=$?` runs, because `set -e` is a global shell flag, not a lexical scope. The errexit-exempt context (`if`/`while`/`||`/`&&`/`!`) at the call site is the ONLY reliable mitigation. This rule applies to:
- `run_lsof_with_timeout` (both first-pass and retry).
- `classify_lsof_fields` (both first-pass and retry).
- `sample_sockets` itself, when called by `sampler_loop` (already if-wrapped) or the new selftest harness (must be if-wrapped, see Selftest fixtures below).

`run_lsof_with_timeout` is written to preserve the caller's errexit state (never re-enables `set -e` internally), but the if/else discipline at every call site is the load-bearing guarantee, not the helper's internal state hygiene.

New flow:

```bash
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

  # SECURITY-FIRST: classify whatever stdout was captured before reasoning
  # about exit status. A hit observed pre-error is real evidence.
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

  # First pass was clean. Decide whether we trust the sample.
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
    # Non-zero with no stderr is benign (e.g., "no matching open files").
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 0
  fi

  # Non-zero with stderr → could be PID churn or real instrumentation failure.
  local live_csv
  live_csv="$(filter_live_pids "$pid_csv")"
  if [[ -z "$live_csv" ]]; then
    # Whole sampled tree gone; first-pass classification was the best we'll
    # get and it was clean.
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 0
  fi
  if [[ "$live_csv" == "$pid_csv" ]]; then
    # No PIDs died → not a race. Real instrumentation failure.
    log "ERROR: lsof instrumentation failed sample=$sample status=$status stderr=$(tr '\n' ' ' <"$stderr_file")"
    rm -f "$raw_file" "$stderr_file" "$timeout_sentinel"
    return 2
  fi

  # Race: retry with live subset, into a fresh file so we don't clobber
  # the first-pass raw (already classified above, so safe to drop now).
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

  # Classify the retry output too: it can surface ongoing leaks.
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

  # IMPORTANT: capture retry timeout state AND retry-stderr-empty state
  # BEFORE cleanup; otherwise both checks are dead code.
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
    # Mirror the first-pass benign branch (above): nonzero with no stderr
    # is "no matching open files". After PID churn, the live subset can
    # legitimately have no TCP/UDP sockets, so lsof exits nonzero with
    # empty stderr; that is NOT instrumentation failure.
    return 0
  fi
  log "ERROR: lsof instrumentation failed (retry) sample=$sample status=$retry_status"
  return 2
}
```

Key shape changes vs current:
- Classification of first-pass stdout is unconditional and runs before any exit-status reasoning.
- Race retry writes to fresh `retry_raw`/`retry_stderr` files; the first-pass `raw_file` is never overwritten, only removed after it's been classified.
- Retry output is also classified for ongoing leaks.
- Timeout sentinels distinguish "lsof exited non-zero on its own" from "we killed lsof", and both `retry_timed_out` and `retry_stderr_empty` are captured into local variables BEFORE cleanup so the post-cleanup checks are meaningful.
- The retry path applies the same benign-vs-failure rule as the first pass: nonzero exit with empty stderr is "no matching open files" (the live subset just has no TCP/UDP sockets after PID churn), NOT instrumentation failure.
- Every non-zero-capable call (`run_lsof_with_timeout`, `classify_lsof_fields`) uses if/else capture; no `set +e ... set -e` blocks inside `sample_sockets`.

## Fix B: `run_lsof_with_timeout` helper

**Same file.** Add a helper, inserted just above `sample_sockets`:

```bash
run_lsof_with_timeout() {
  local timeout_secs="$1"
  local pids="$2"
  local raw="$3"
  local stderr_path="$4"
  local timeout_sentinel="$5"

  rm -f "$timeout_sentinel"

  lsof -nP -a -p "$pids" -iTCP -iUDP -F pcPTn0 >"$raw" 2>"$stderr_path" &
  local lsof_pid=$!

  # Killer subshell polls every 1s instead of sleeping monolithically for
  # $timeout_secs. On a fast lsof exit it self-terminates within ~1s, so
  # killing it from the parent (below) does not orphan a long-running
  # `sleep "$timeout_secs"` child. Worst-case lingering sleep child is ~1s
  # regardless of the configured timeout.
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

  # Best-effort: signal the killer in case it is still polling. On a fast
  # lsof exit it has likely already self-terminated; the kill then no-ops.
  kill "$killer_pid" 2>/dev/null || true
  wait "$killer_pid" 2>/dev/null || true

  return "$status"
}
```

Default timeout: 3 seconds (lsof normally returns in ms; 3s is conservative, well below the smoke run's wall-clock budget). Env override: `BAGZ_LSOF_TIMEOUT_SECS`, validated at script entry (see Fix C).

This bounds every `lsof` invocation against ordinary slow or stuck-but-killable hangs, which closes both the in-flight sampler-hang case (Concern 2) and the post-app-exit `wait "$SAMPLER_PID"` case: the sampler can no longer be stuck inside a killable `lsof` for longer than `timeout_secs + 1s`. Caveat: if `lsof` is wedged in an uninterruptible kernel sleep (D-state on Linux; the macOS analogue), `wait "$lsof_pid"` itself blocks because SIGKILL cannot reap a kernel-wedged process. D-state wedges are out of scope (see Context); the outer CI/make timeout is the backstop for that pathological case, since a parent-side polling design would not help (the wait would still be blocked on the same kernel-held child).

Errexit hygiene: the helper uses `if wait …; then …; else status=$?; fi` rather than `set +e; wait; status=$?; set -e`. Toggling `set -e` inside a helper is dangerous: the flag is global, so leaving it `on` at return means a non-zero return from the helper triggers errexit at the caller's call site (unless that call site is errexit-exempt). Preserving the caller's errexit state by never touching the flag here is the simple, reliable invariant. Callers MUST still use `if helper; then …; else rc=$?; fi` — see the Errexit discipline note in Fix A.

## Fix C: validate `BAGZ_LSOF_TIMEOUT_SECS` at script entry

**Same file.** With the polling killer in Fix B, `$timeout_secs` is consumed as the integer upper bound of `while [[ "$waited" -lt "$timeout_secs" ]]; do sleep 1; … done` (Fix B line 186), not as the argument to a monolithic `sleep "$timeout_secs"`. The validation is still mandatory: if `BAGZ_LSOF_TIMEOUT_SECS` is non-numeric, empty, negative, or zero, the integer comparison either fails to iterate at all (immediate SIGKILL on lsof regardless of real runtime, breaking timeout semantics) or errors in `set -e` strict mode, depending on bash's lenient-vs-strict arithmetic parsing. Either way the killer no longer represents "kill lsof if it has not finished within $timeout_secs seconds" — the helper's whole guarantee evaporates. Fail-fast at script entry on the validated value is the simple invariant.

Mirror the existing `SMOKE_DURATION_SECS` validation pattern in `scripts/cef-network-smoketest.sh` (currently at lines 6 and 15) by adding, near the top of the script alongside that block:

```bash
LSOF_TIMEOUT_SECS="${BAGZ_LSOF_TIMEOUT_SECS:-3}"
if ! [[ "$LSOF_TIMEOUT_SECS" =~ ^[0-9]+$ ]] || [[ "$LSOF_TIMEOUT_SECS" -eq 0 ]]; then
  echo "error: BAGZ_LSOF_TIMEOUT_SECS must be a positive integer" >&2
  exit 2
fi
```

The pseudocode in Fix A already consumes `$LSOF_TIMEOUT_SECS` (the validated value), not the raw env var, in both the first-pass and retry call sites and in the timeout log message.

The script's top-level validation runs once at startup, so `BAGZ_LSOF_TIMEOUT_SECS=1 run_sample_sockets_fixture …` in a fixture line cannot override the per-fixture timeout: `LSOF_TIMEOUT_SECS` is already locked in. The fixture helper itself must therefore rebind (and restore) `LSOF_TIMEOUT_SECS` around the `sample_sockets` call when a fixture-specific override is requested. To avoid collision with a real `BAGZ_LSOF_TIMEOUT_SECS` set in the user's environment, the fixture override uses a distinct name: `BAGZ_LSOF_TIMEOUT_SECS_FIXTURE`. The complete fixture helper body, including override save/validate/apply/restore, lives in the Selftest fixtures section below (single canonical copy, no duplication).

## Selftest fixtures

**Same file.** Add a stub-driven harness for `sample_sockets`, since the new flow has more branches than `classify_lsof_fields` alone exercises.

New helper near `run_filter_live_pids_fixture` (currently lines 269-281):

```bash
run_sample_sockets_fixture() {
  local label="$1"
  local expected_rc="$2"
  local stub_script="$3"

  # Validate the fixture timeout override BEFORE mutating any state, so a
  # rejected override does not leak temp dirs or rebound globals.
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

  # Optional: inject a fake dead descendant PID into enumerate_descendants
  # so sample_sockets sees PID churn (filter_live_pids drops the fake PID,
  # triggering the retry path). Pass a PID that does not correspond to a
  # running process. The fixture below uses a freshly-reaped child PID;
  # there is a small kernel-PID-reuse race window, but selftest is low-churn.
  local prior_enumerate_def=""
  if [[ -n "${BAGZ_FIXTURE_FAKE_DEAD_DESCENDANT:-}" ]]; then
    prior_enumerate_def="$(declare -f enumerate_descendants)"
    enumerate_descendants() {
      printf '%s\n' "$1"
      printf '%s\n' "$BAGZ_FIXTURE_FAKE_DEAD_DESCENDANT"
    }
  fi

  # Stateful-stub state directory (for fixtures whose lsof stub needs to
  # behave differently on first vs subsequent calls).
  export BAGZ_FIXTURE_STATE_DIR="$SMOKE_ROOT"

  # Errexit-exempt call site: sample_sockets and any helper it calls may
  # legitimately return non-zero. Bare `cmd; rc=$?` under `set -e` (even
  # bracketed by set +e/set -e) is fragile because nested helpers can re-
  # enable errexit. The if/else form is unconditional and safe.
  local got_rc
  if sample_sockets "fixture-$label"; then
    got_rc=0
  else
    got_rc=$?
  fi

  # Restore ALL prior state BEFORE the verdict return, so a FAIL path does
  # not leak temp dirs, mutated PATH, APP_PID, SMOKE_ROOT, LSOF_TIMEOUT_SECS,
  # enumerate_descendants, or BAGZ_FIXTURE_STATE_DIR into subsequent fixtures.
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
```

Add these fixtures inside `run_selftest`, after the existing `filter_live_pids` block:

```bash
# Concern 1: evidence in stdout + non-zero status with stderr MUST classify
# as network-failure, not be discarded by the race-retry path.
run_sample_sockets_fixture evidence-with-nonzero-status 1 '#!/usr/bin/env bash
printf "p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->142.250.190.78:443\0"
echo "lsof: synthetic stderr about a dead PID" >&2
exit 1'

# Clean stdout + non-zero status with stderr, no PID churn (live set
# unchanged because the fixture uses APP_PID=$$, which is always reachable)
# → instrumentation-failure (no false PASS).
run_sample_sockets_fixture clean-with-nonzero-status-no-race 2 '#!/usr/bin/env bash
printf "p1234\0cbagZ\0PTCP\0TST=LISTEN\0n127.0.0.1:7777\0"
echo "lsof: synthetic generic error" >&2
exit 1'

# Concern 2: hung lsof MUST timeout to instrumentation-failure within the
# bound, not hang the sampler. Use `exec sleep` so the stub shell *becomes*
# the sleep process, and the helper's SIGKILL of the stub PID also reaps
# the sleep (otherwise the killed shell could leave its child sleep alive).
BAGZ_LSOF_TIMEOUT_SECS_FIXTURE=1 \
run_sample_sockets_fixture timeout-no-evidence 2 '#!/usr/bin/env bash
exec sleep 30'

# Bonus: hung lsof that emitted evidence before hanging MUST still classify
# as network-failure (security-first). `printf` first, then `exec sleep` to
# avoid orphaning the sleep child when the helper kills the stub PID.
BAGZ_LSOF_TIMEOUT_SECS_FIXTURE=1 \
run_sample_sockets_fixture timeout-with-evidence 1 '#!/usr/bin/env bash
printf "p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->142.250.190.78:443\0"
exec sleep 30'

# Retry path benign: first pass has stdout, nonzero exit, AND stderr
# (forces race detection via filter_live_pids dropping the injected fake
# dead PID); retry on the live subset exits nonzero with EMPTY stderr
# (benign "no matching open files" because the live subset has no TCP/UDP
# sockets after PID churn). MUST pass (rc=0), not fail as instrumentation.
# Fake-dead-PID generation: spawn a `true`, wait for it to reap, then
# reuse its PID. Small kernel-PID-reuse race window is acceptable in
# selftest because no other process churn happens between this line and
# `filter_live_pids` inside `sample_sockets`.
__bagz_fixture_dead_pid_helper=$(
  (:) &
  pid=$!
  wait "$pid" 2>/dev/null || true
  printf '%s' "$pid"
)
BAGZ_FIXTURE_FAKE_DEAD_DESCENDANT="$__bagz_fixture_dead_pid_helper" \
run_sample_sockets_fixture retry-benign-no-matching-files 0 '#!/usr/bin/env bash
counter_file="$BAGZ_FIXTURE_STATE_DIR/lsof-calls"
count=$(cat "$counter_file" 2>/dev/null || echo 0)
echo $((count + 1)) > "$counter_file"
if [[ "$count" -eq 0 ]]; then
  printf "p1234\0cbagZ\0PTCP\0TST=LISTEN\0n127.0.0.1:7777\0"
  echo "lsof: synthetic stderr forcing race retry" >&2
  exit 1
else
  exit 1
fi'
unset __bagz_fixture_dead_pid_helper
```

Each timeout fixture is wall-clock bounded by `BAGZ_LSOF_TIMEOUT_SECS_FIXTURE=1` so selftest stays fast (the two timeout fixtures take ~1-2s each). The `exec sleep` form ensures the stub does not orphan a long-running sleep when the helper kills the stub PID.

## Files modified

| File | Change |
|---|---|
| `scripts/cef-network-smoketest.sh` | Add top-level `LSOF_TIMEOUT_SECS` validation block, add `run_lsof_with_timeout` helper with polling killer subshell, rewrite `sample_sockets` to classify-first (consuming validated `$LSOF_TIMEOUT_SECS`, capturing `retry_timed_out` and `retry_stderr_empty` before cleanup, applying benign empty-stderr rule to retry too), add `run_sample_sockets_fixture` helper with per-fixture `BAGZ_LSOF_TIMEOUT_SECS_FIXTURE` re-validation and optional `enumerate_descendants` injection, add five new selftest fixtures |

No other files touched.

## Verification

1. `BAGZ_SMOKE_SELFTEST=1 ./scripts/cef-network-smoketest.sh` — all existing fixtures pass AND the five new `sample_sockets` fixtures pass. Total selftest runtime should stay under ~5s.
2. `make pre-commit` — fmt/clippy/static check/cargo args test/parser selftest all green.
3. `make test` — full workspace Rust tests untouched, all green.
4. `make tauri-build` — bundle rebuild on clean tree.
5. `make cef-smoketest` — baseline against the freshly packaged app, PASS expected. The new per-call timeout SHOULD NOT trigger in normal runs (lsof returns in ms).
6. Regression rehearsals (each MUST be reverted before commit):
   - **6a Concern 1, full smoke:** stub at `/tmp/bagz-lsof-stub-evidence/lsof` that emits `printf "p$$\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->8.8.8.8:443\0"` to stdout, "synthetic error" to stderr, exits 1. Run `PATH=/tmp/bagz-lsof-stub-evidence:$PATH make cef-smoketest`. Expect exit code 1 with "non-loopback CEF socket observed" log line (NOT exit 2). `rm -rf /tmp/bagz-lsof-stub-evidence` after.
   - **6b Concern 2, full smoke:** stub at `/tmp/bagz-lsof-stub-hang/lsof` that does `exec sleep 60` (uses `exec` so the helper's SIGKILL of the stub PID reaps the sleep instead of orphaning it). Run `BAGZ_LSOF_TIMEOUT_SECS=2 PATH=/tmp/bagz-lsof-stub-hang:$PATH make cef-smoketest`. Expect exit code 2 with "lsof timed out" log line, total runtime well under the outer CI/make timeout. `rm -rf /tmp/bagz-lsof-stub-hang` after.
   - **6c Concern 2, validation:** confirm fail-fast on bad input. `BAGZ_LSOF_TIMEOUT_SECS=oops ./scripts/cef-network-smoketest.sh </dev/null` and `BAGZ_LSOF_TIMEOUT_SECS=0 ./scripts/cef-network-smoketest.sh </dev/null` both MUST exit non-zero with the "BAGZ_LSOF_TIMEOUT_SECS must be a positive integer" error on stderr, before any CEF launch.
7. `git diff --stat -- scripts/cef-network-smoketest.sh` empty after rehearsal cleanup.

## Commit shape

Single commit, one file: `scripts/cef-network-smoketest.sh`.

Message: `fix(cef): classify lsof output before retry and bound per-call timeout`.

`git diff --cached --name-only | sort` MUST output EXACTLY:

```
scripts/cef-network-smoketest.sh
```
