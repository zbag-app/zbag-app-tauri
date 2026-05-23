# Plan: port `cef-network-smoketest.sh` to a Rust xtask, add a justfile wrapper (single PR)

## Context

`scripts/cef-network-smoketest.sh` (~740 lines of bash) is one of three guardrails that enforce NFR-002.a: "CEF MUST NOT emit any non-loopback network traffic during normal operation" (`specs/001-bagz-desktop-wallet/spec.md:391`). The other two are `scripts/check-cef-network-hardening.sh` (static source checks) and `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs` (parsed runtime arguments). The shell script has grown complex: an lsof-based sampler, retry/timeout watchdog, process-tree enumeration, bundle discovery via PlistBuddy, an EXIT-trap cleanup, and a 13-case selftest mode (5 parser fixtures, 3 live-PID fixtures, 5 sample-sockets fixtures). The complexity is justified by the security stakes, but bash is the wrong tool for this kind of state machine: testability is poor (the selftest stubs `lsof` via PATH injection), error semantics depend on `set -euo pipefail` discipline, and the three-state exit code (0=pass, 1=policy violation, 2=instrumentation failure) is enforced by hand.

The wallet is in alpha and has no public users. That lets us move faster than a typical mature-codebase migration: one PR can carry the full port, the CI cutover, the docs/spec updates, and the bash deletion together. We are not weakening the security invariant; the Rust port preserves the three exit codes, the artifact log path, the BAGZ_GRPC_URL / BAGZ_USE_SYSTEM_KEYCHAIN / HOME / XDG / TMPDIR app isolation, and the policy-vs-instrumentation distinction. The phase structure below is kept as a way to organize work inside the branch (each phase is a logical commit boundary), not as separate PRs.

## 1. Recommendation

Land everything in one PR on a single feature branch. Internally structure the branch as five phased commits (scaffold + parsers, sampler abstraction, live orchestrator, Make/CI cutover, cleanup + justfile). The PR is not done until all five phases are present, the bash script is deleted, every doc/spec reference is updated, and the new validation suite passes.

Order within the branch:

1. xtask first. It carries all the real value (testable, typed, debuggable).
2. justfile last, after xtask is wired into Make and CI. justfile delegates to `cargo xtask ...` directly (not to `make`), so it does not add a second indirection.

Out of scope for this PR: porting any other script (`cef-size-report.sh`, `check-no-telemetry.sh`, `check-cef-network-hardening.sh`, `cef-stage-slim.sh`, etc.). They stay as-is; the xtask module layout below leaves room to port them in future PRs.

## 2. Target Architecture

### Crate placement

- Directory: `xtask/` at the repo root (cargo-xtask convention).
- Package name: `bagz-xtask` (matches the `bagz-*` naming convention used elsewhere; lets `cargo run -p bagz-xtask` read cleanly).
- Workspace membership: yes, add to `[workspace] members`. Note that `default-members` does NOT change what `cargo ... --workspace` builds; it only controls bare `cargo build` with no `-p`/`--workspace`. The repo's Make/CI targets uniformly use `--workspace`, so the only reliable mechanism is to audit every `cargo {build,test,clippy} --workspace` invocation in `Makefile` and `.github/workflows/ci.yml` and add `--exclude bagz-xtask` alongside the existing `--exclude bagz-app-tauri`. The `fmt`/`fmt-check` targets use `cargo fmt --all`, which does not accept `--exclude`; let fmt cover xtask (this is desirable, it keeps formatting consistent).
- `.cargo/config.toml` alias: `xtask = "run --package bagz-xtask --"` so `cargo xtask cef-smoketest` works.

### CLI shape

```
cargo run -p bagz-xtask -- cef-smoketest --selftest
cargo run -p bagz-xtask -- cef-smoketest [--app <path>] [--duration-secs N] [--lsof-timeout-secs N] [--log-path <file>]
cargo xtask cef-smoketest [args]        # after .cargo/config.toml alias lands
```

Environment-variable parity (preserve exact names so CI overrides keep working):

| Env var | CLI flag | Default |
|---|---|---|
| `BAGZ_SMOKE_SELFTEST=1` | `--selftest` | off |
| `BAGZ_SMOKE_DURATION_SECS` | `--duration-secs` | 15 |
| `BAGZ_LSOF_TIMEOUT_SECS` | `--lsof-timeout-secs` | 3 |
| `RUNNER_TEMP` (read at runtime) | `--log-path` | `${RUNNER_TEMP:-/tmp}/bagz-cef-smoketest.log` |

`--duration-secs` and `--lsof-timeout-secs` must reject `0` to match the bash precondition at `scripts/cef-network-smoketest.sh:16-23` (which exits 2 if either is non-numeric or zero). Use `std::num::NonZeroU32` as the clap field type so both CLI-provided and env-provided zeros fail parse (which clap reports as exit code 2, matching "instrumentation failure"). Add a unit test asserting `Cli::try_parse_from(["xtask", "cef-smoketest", "--duration-secs", "0"]).unwrap_err().exit_code() == 2`.

**Intentional CLI break: no positional `APP_BUNDLE` argument.** The bash script accepts an optional positional bundle path (`APP_BUNDLE="${1:-...}"`). The Rust port exposes the same value only through `--app <path>`. This is a deliberate compatibility break because (a) the only in-tree callers are `Makefile` targets `cef-smoketest{-selftest}` and the `tauri-build-cef` / `cef-network-hardening` CI jobs, none of which pass a positional argument, and (b) clap mixing a positional with same-purpose `--app` flag adds a precedence-rule footgun for marginal benefit. Document the break in the PR description so any human who muscle-memory-types `./scripts/cef-network-smoketest.sh /path/to/MyApp.app` learns the new spelling. If a positional form is required later, prefer adding it explicitly with a precedence rule (`--app` wins on conflict, with a test) rather than letting clap auto-resolve.

Exit codes (preserve exactly):

- `0` pass
- `1` non-loopback socket observed (network policy violation)
- `2` instrumentation failure / watchdog / lifecycle issue

Clap's default parse-error exit is `2`, which conveniently matches "instrumentation failure" (a malformed CLI in CI is an instrumentation problem). Do not try to remap it. Lock it in with a unit test rather than an API setting: `assert_eq!(Cli::try_parse_from(["xtask", "cef-smoketest", "--no-such-flag"]).unwrap_err().exit_code(), 2)`. If a future clap upgrade ever shifts the default, the test fails loudly. See also "clap exit codes" in section 7.

### Module layout

```
xtask/
  Cargo.toml
  src/
    lib.rs                   # crate library: `pub mod cli; pub mod cmd;` plus
                             #   `pub fn run() -> ExitCode` and the testable
                             #   `pub fn promote_selftest_env`. Required so
                             #   integration tests under `xtask/tests/` can
                             #   reach internal modules; see note below.
    main.rs                  # thin binary entry: `fn main() -> ExitCode {
                             #   bagz_xtask::run() }`
    cli.rs                   # clap derive types (pub Cmd, pub CefSmoketestArgs)
    cmd/
      mod.rs
      cef_smoketest/
        mod.rs               # public entry: run_smoke / run_selftest
        parser.rs            # endpoint_host, is_loopback_host, classify_socket,
                             #   classify_lsof_fields
        lsof.rs              # trait LsofRunner (cross-platform) + RealLsof
                             #   (macOS-gated; Command + timeout)
        process.rs           # filter_live_pids (cross-platform, Phase 1) +
                             #   trait ProcessEnumerator (cross-platform, Phase 2) +
                             #   Pgrep (macOS-gated, Phase 2) + kill_tree (Phase 2)
        bundle.rs            # CFBundleExecutable via `plist` crate, fallback scan
                             #   (macOS-gated; only callers of plist live here)
        sampler.rs           # 1s poll loop, sentinel file management
                             #   (sample_once is cross-platform; loop is too)
        smoke.rs             # SmokeSession struct: env setup, spawn, watchdog,
                             #   Drop = cleanup (macOS-gated)
        log.rs               # timestamped log to file + stderr, copy_log
        exit.rs              # ExitCode enum: Pass=0 / Policy=1 / Instrument=2
        selftest.rs          # fixture runners (cross-platform; uses FakeLsof)
  tests/                     # Cargo integration tests; xtask/tests/ at the
                             # crate root, NOT under src/cmd
    parser_fixtures.rs       # 5 parser fixtures as #[test]
    live_pid.rs              # 3 live-pid fixtures
    sample_sockets.rs        # 5 lsof-stub fixtures via FakeLsof
```

**Why both `lib.rs` and `main.rs`.** Cargo integration tests in `xtask/tests/*.rs` are compiled as separate crates that only see the public API of the package's library target. A binary-only crate (just `main.rs`) exposes nothing to `tests/`, so `use bagz_xtask::cmd::cef_smoketest::parser::*;` would fail to compile. Putting the testable surface in `lib.rs` (with `pub mod cli; pub mod cmd; pub fn run(); pub fn promote_selftest_env(...)`) and reducing `main.rs` to a one-liner that calls `bagz_xtask::run()` is the standard cargo pattern for this case. The binary still builds; `cargo run -p bagz-xtask -- cef-smoketest ...` and `cargo xtask cef-smoketest ...` behave identically to a bin-only layout.

One subcommand-per-directory leaves room for future xtask commands (`check-cef-network-hardening`, `check-no-telemetry`, `cef-size-report` are obvious next ports) without `cef_smoketest` sprawling.

### Minimal dependencies

- `clap` (v4, `derive` + `env`). Already used in `apps/bagz-cli`.
- `anyhow` (workspace dep). Top-level error propagation in `main`.
- `thiserror` (workspace dep). Typed error per exit code.
- `tempfile` (workspace dep). `SMOKE_ROOT` + `RUN_LOG`.
- `plist` (1.x, pure Rust). `CFBundleExecutable` parsing. Avoids shelling to PlistBuddy and keeps cross-compile clean.
- `chrono` (workspace dep). ISO-8601 UTC timestamps to match `date -u '+%Y-%m-%dT%H:%M:%SZ'`.
- `ctrlc` (3.x, `features = ["termination"]`). SIGINT plus SIGTERM/SIGHUP handler that triggers explicit cleanup. The `termination` feature is required for SIGTERM/SIGHUP coverage; default `ctrlc` only handles SIGINT. Bash `trap EXIT` fires on all of these; Rust `Drop` does not.
- `tracing` + `tracing-subscriber` (workspace deps). Structured log output.

Explicitly avoid: `sysinfo` (heavy; replaced by `pgrep` shell-out), `tokio` (the orchestrator is naturally synchronous; use `std::thread` + `mpsc::recv_timeout` for the lsof watchdog), `which`, `libc` (every signal/kill path shells out to `kill` for parity with the bash and to avoid an FFI surface).

### Preserving exit-code semantics and log artifact

- `ExitCode::Policy = 1` is returned only when `classify_lsof_fields` records a non-loopback endpoint. Any error in the sampling pipeline (lsof spawn fail, watchdog fire, instrumentation can't disambiguate after retry, app exits non-zero, readiness sentinel missing) is `ExitCode::Instrument = 2`.
- The log file path resolution is `--log-path` flag > `RUNNER_TEMP` env > `/tmp`. Same as the bash. The path is printed to stdout on cleanup so CI can locate it.
- `actions/upload-artifact@v4` step in CI does not change: it reads `${{ runner.temp }}/bagz-cef-smoketest.log` and the Rust port writes there by default.

## 3. Bash-to-Rust Parity Map

| Bash function (line) | Rust module::item | Notes |
|---|---|---|
| `log` (`scripts/cef-network-smoketest.sh:25-32`) | `LogArtifact::write` | `tracing::info!` + tee to `RUN_LOG` file. ISO-8601 UTC. |
| `copy_log` (`:34-46`) + `trap EXIT` print (`:92`) | `LogArtifact::Drop` (top-level guard) | idempotent via internal `Once`; runs on ALL exit paths (selftest, non-macOS early return, missing lsof, missing bundle, signal, normal). `SmokeSession::Drop` step 4 also calls `copy_once` on the same `LogArtifact` so the two converge through one guard. See section 8 "Top-level log artifact guard". |
| `enumerate_descendants` (`:48-57`) | `process::ProcessEnumerator::descendants` | trait method returning the full transitive descendant set. `Pgrep` impl shells out to `pgrep -P <pid>` (which only returns direct children), then RECURSES on each child to gather grandchildren, great-grandchildren, etc., matching bash lines 53-56. Returning only direct children would leak grandchild PIDs past both the sampler (so their sockets would be invisible) and `kill_tree` (so they would survive cleanup). Add a `FakeProcessEnumerator` test that exercises a 3-level tree (root → child → grandchild) and asserts the returned set includes both child and grandchild PIDs. |
| `kill_tree` (`:59-71`) | `process::kill_tree` | reverse-sorted PIDs, shell out to `Command::new("kill").args(["-KILL", ...])` for uniformity with `pgrep` and to avoid pulling in `libc` |
| `cleanup` (`:73-94`) + `trap EXIT` | `smoke::SmokeSession::stop_helpers_and_join()` (called inline before post-wait checks) + `smoke::SmokeSession::drop` (idempotent fallback) + `ctrlc` handler | Exact contract in section 8 "Ctrl-C / SIGTERM cleanup mechanism": handler only sets `signal_requested`; `stop_helpers_and_join()` sets `stop_helpers` and joins sampler+watchdog so the sentinel files are guaranteed flushed before the post-wait checks read them (matching bash lines 698-701 happening BEFORE lines 711+); `Drop` then kills the app tree, reaps the child, copies log exactly once via the `LogArtifact` `Once` guard, drops `TempDir`. Run terminated by signal returns `ExitCode::Instrument`. |
| `endpoint_host` (`:97-112`) | `parser::endpoint_host` | strip `[]` for IPv6, split on rightmost `:` |
| `is_loopback_host` (`:114-120`) | `parser::is_loopback_host` | `host.starts_with("127.") \|\| host == "::1"` |
| `classify_socket` (`:136-163`) | `parser::classify_socket` | enum `SocketState::{Listen, Established}`; policy check |
| `classify_lsof_fields` (`:165-206`) | `parser::classify_lsof_fields` | parse null-terminated `p`/`c`/`P`/`TST=`/`n` field stream |
| `filter_live_pids` (`:260-272`) | `process::filter_live_pids` | free function (no trait needed); `kill -0 <pid>` per CSV entry via `Command::new("kill").args(["-0", pid]).status()`; returns the live subset. Cross-platform. Lives in `process.rs` so the live-PID fixtures in Phase 1 can `use bagz_xtask::cmd::cef_smoketest::process::filter_live_pids`. |
| selftest fixtures (`:208-258`, `:356-413`) | `selftest::run_parser_fixtures` + `#[test]` in `tests/parser_fixtures.rs` | the 5 cases are also unit tests |
| `run_filter_live_pids_fixture` (`:274-286`) | `selftest::run_live_pid_fixtures` + `#[test]` | `kill -0` check via `Command::new("kill").args(["-0", ...]).status()`; same rationale as `kill_tree` (no `libc` dep) |
| `run_sample_sockets_fixture` (`:288-354`) | `selftest::run_sample_sockets_fixtures` + `#[test]` | use `FakeLsof` impl of `LsofRunner` trait, not PATH-stubbed binary. The `retry-benign-no-matching-files` case (`:392-409`) additionally requires `FakeProcessEnumerator` (or an explicit `[live_pid, dead_pid]` vector) so the retry branch exercises `filter_live_pids` between the two `lsof` calls; see Phase 2 bullet for the seam. |
| `run_lsof_with_timeout` (`:415-454`) | `lsof::RealLsof::run` | `Command::spawn` + `std::thread` watchdog + `mpsc::recv_timeout`. Exact argv pinned in section 8 "lsof invocation": `lsof -nP -a -p <pid_csv> -iTCP -iUDP -F pcPTn0`. |
| `sample_sockets` (`:456-572`) | `sampler::sample_once` | the retry-on-stderr-non-empty branch lives here; preserve all three return paths (Ok / Policy / Instrument) |
| `bundle_executable` (`:574-604`) | `bundle::resolve_executable` | `plist::Value` parses `Info.plist`, fall back to single-executable scan of `Contents/MacOS/` |
| `sampler_loop` (`:606-624`) | `sampler::run_loop` | 1s tick; touch sentinel files `network-failure` / `instrumentation-failure` in `SMOKE_ROOT` |
| `run_smoke` (`:626-742`) | `smoke::run_smoke` | env setup (`BAGZ_GRPC_URL=https://127.0.0.1:1`, `BAGZ_HEADLESS_SMOKE=1`, `BAGZ_USE_SYSTEM_KEYCHAIN=0`, `BAGZ_SMOKE_DURATION_SECS`, `BAGZ_SMOKE_READY_FILE`, sandboxed `HOME`/`XDG_*`/`TMPDIR`), spawn app, fork hard-watchdog thread (`duration + 30s` deadline, then touches `watchdog-fired` and kills tree), fork sampler thread (1s tick), main thread waits for child to self-exit via `try_wait` polling (NOT a duration timer), then verifies in bash order: `watchdog-fired` absent, readiness sentinel present, elapsed in `[duration - 2, duration + 5]`, app status zero, instrumentation sentinel absent, network-failure sentinel absent. See section 8 "Ctrl-C / SIGTERM cleanup mechanism" steps 3-4 for the exact contract. |

The bash selftest covers 13 cases: 5 parser fixtures (`scripts/cef-network-smoketest.sh:208-258`), 3 live-PID fixtures (`:274-286`), and 5 sample-sockets fixtures (`:288-354`). All 13 must be ported as `#[test]` functions before the PR is mergeable. The 8 pure ones (parser + live-PID) are completed in Phase 1; the 5 sample-sockets fixtures land in Phase 2.

## 4. Implementation Phases (internal checkpoints in one PR)

Each phase below is an internal checkpoint on the feature branch, NOT a separate PR. The phases exist to organize commits within the branch and to make review readable; the PR only merges once all five phases are present. The bash script can remain in tree during Phase 1-3 (so contributors on the branch can still run the legacy smoketest while building Rust parity); it is deleted in Phase 5 before the PR closes.

### Phase 1: scaffold xtask + pure parser/live-PID tests (1 commit)

- Create `xtask/Cargo.toml`, `xtask/src/lib.rs`, `xtask/src/main.rs`, `xtask/src/cli.rs`, `xtask/src/cmd/{mod,cef_smoketest/{mod,parser,process,selftest,log,exit}}.rs`. `lib.rs` is the testable surface (`pub mod cli; pub mod cmd; pub fn run; pub fn promote_selftest_env`); `main.rs` is `fn main() -> ExitCode { bagz_xtask::run() }`. This is the only layout in which `xtask/tests/*.rs` can reach internal modules. `process.rs` is created in this phase with only `filter_live_pids` (a cross-platform free function); the `ProcessEnumerator` trait, `Pgrep` impl, and `kill_tree` land in Phase 2.
- Add `xtask` to workspace members. Do not touch `default-members`; it has no effect on the explicit `--workspace` invocations used in Make/CI.
- Add `.cargo/config.toml` alias.
- Port `endpoint_host`, `is_loopback_host`, `classify_socket`, `classify_lsof_fields` into `parser.rs`, and `filter_live_pids` into `process.rs`. Cover the 5 parser fixtures and 3 live-PID fixtures as `#[test]` functions in `xtask/tests/parser_fixtures.rs` and `xtask/tests/live_pid.rs` respectively (8 of the 13 selftest cases). They import via `use bagz_xtask::cmd::cef_smoketest::parser::*;` and `use bagz_xtask::cmd::cef_smoketest::process::filter_live_pids;`.
- Implement `log::LogArtifact` (top-level guard owned by `cmd::cef_smoketest::run`; `Drop` impl copies the run log to the artifact path exactly once via `Once`, then prints the artifact path on stdout). This covers selftest, non-macOS early return, and any pre-`SmokeSession` failure in later phases. See section 8 "Top-level log artifact guard". Add a unit test asserting the file is copied on `Drop` even when `copy_once` was never called explicitly.
- Wire `cargo run -p bagz-xtask -- cef-smoketest --selftest` to construct the `LogArtifact`, run those fixtures (writing PASS/FAIL lines via `LogArtifact::write` that mirror the bash log format), and let the `LogArtifact` Drop guard copy + print the artifact path as the selftest returns.
- Add `--exclude bagz-xtask` to every `--workspace` cargo invocation in `Makefile` and `ci.yml` that currently uses `--exclude bagz-app-tauri`. Confirmed Makefile sites: `build` (line 40), `build-release` (line 43), `test`, `clippy` (line 100), `clippy-strict` (line 103). Confirmed CI sites: `rust-clippy` (line 72), `rust-tests` (line 85), and the `rust-build` release job. Skip `fmt`/`fmt-check`; `cargo fmt --all` has no `--exclude` and formatting xtask is desirable.
- Add a new CI job `rust-xtask` that runs, in order, `cargo build -p bagz-xtask`, `cargo clippy -p bagz-xtask --all-targets -- -D warnings`, and `cargo test -p bagz-xtask`. The clippy step is necessary because xtask is excluded from the workspace `rust-clippy` job and the smoketest port is security-sensitive enough to want lint coverage. Model the job after `cef-args-tests` in `.github/workflows/ci.yml` (which uses `actions/checkout@v4` + `actions-rust-lang/setup-rust-toolchain@v1` with `cache: false` and `components: clippy`).

### Phase 2: lsof/process abstraction + full 13-case selftest parity (1 commit)

- Add new module `xtask/src/cmd/cef_smoketest/lsof.rs` with `trait LsofRunner` and `RealLsof` impl (Command + std::thread watchdog + `mpsc::recv_timeout`). Exact argv is pinned in section 8 "lsof invocation".
- Extend existing `xtask/src/cmd/cef_smoketest/process.rs` (created in Phase 1) with `trait ProcessEnumerator { fn descendants(&self, root: u32) -> Vec<u32>; }` (returns the transitive descendant set), `Pgrep` impl (macOS-gated; shells out to `pgrep -P <pid>` for direct children, then RECURSES on each child to gather grandchildren and beyond, matching bash `enumerate_descendants` lines 48-57), and `kill_tree` (reverse-sorted PIDs, `Command::new("kill").args(["-KILL", ...])`). Add `FakeProcessEnumerator` (cross-platform) backed by an in-memory parent→children map, plus a `#[test]` that walks a 3-level tree (root → child → grandchild) and asserts both child and grandchild appear in the returned `Vec<u32>`. Direct-children-only enumeration is a regression that lets grandchild sockets escape both sampling and cleanup; the test guards against it.
- Add new module `xtask/src/cmd/cef_smoketest/sampler.rs` with `sample_once` implementing the full retry-on-stderr-non-empty + live-PID-filter logic (calls `process::filter_live_pids` from Phase 1).
- Port all 5 `run_sample_sockets_fixture` cases as `#[test]` in `xtask/tests/sample_sockets.rs` using `FakeLsof`. The `retry-benign-no-matching-files` fixture also needs a process-enumeration seam exposing one live and one dead PID so the retry path exercises `filter_live_pids` between the first and second `lsof` call: bash creates a dead PID via `(:) &; wait $pid` (`scripts/cef-network-smoketest.sh:392-398`) and injects it through `BAGZ_FIXTURE_FAKE_DEAD_DESCENDANT` (`:319-326`, `:399`). The Rust port should plumb a `FakeProcessEnumerator` (or an explicit `Vec<u32>` of `[live_pid, dead_pid]`) into `sample_once` for this fixture, where `dead_pid` is generated the same way (spawn a short-lived child and `wait()` for it before the assertion). Without that seam, `filter_live_pids` has nothing to filter and the retry branch is uncovered.
- `--selftest` now reaches full parity with `BAGZ_SMOKE_SELFTEST=1` (13/13 fixtures).

### Phase 3: live smoke orchestrator parity (1 commit)

- Port `bundle::resolve_executable` using the `plist` crate.
- Port `smoke::run_smoke`: env setup, `tempfile::TempDir` for `SMOKE_ROOT`, `Command::spawn` for the app, hard watchdog thread (sleeps `duration + 30s` via `mpsc::Receiver::recv_timeout` so it can be woken early, then touches `watchdog-fired` sentinel and kills the app tree), sampler thread (1s tick), main thread calls `wait_for_child(...) -> WaitOutcome` which returns `Exited(status)` or `Signaled` (typed so the signal branch is not ambiguous), then calls `session.stop_helpers_and_join()` BEFORE reading any sentinel (matching bash order: lines 698-701 happen before lines 711+), then runs the post-wait checks in bash order (readiness sentinel, elapsed window `[duration.saturating_sub(2), duration + 5]` — saturating-sub matches bash's clamp at line 706-708 and is load-bearing for `duration_secs = 1` in the ignored watchdog test, where `1 - 2` would otherwise underflow `u32`), app exit status, instrumentation sentinel, network-failure sentinel).
- `SmokeSession` struct owns the `TempDir`, child handle (`std::process::Child`), `Option<JoinHandle<()>>` for both the sampler and watchdog (taken by `stop_helpers_and_join()` on the normal path or by `Drop` on the early-error/panic path), the mpsc sender that wakes the watchdog's `recv_timeout` early, a clone of the outer `Arc<AtomicBool>` named `signal_requested` (read-only here; never mutated by `SmokeSession`) plus a fresh `Arc<AtomicBool>` named `stop_helpers` (set by `stop_helpers_and_join()`), and a borrowed reference (or `Arc`) to the outer `LogArtifact` so its `Drop` can call `log_artifact.copy_once()`. The `Once`-backed copy guard lives on `LogArtifact`, not on `SmokeSession`. Public methods: `stop_helpers_and_join(&mut self)` (idempotent, no-op if `Option`s are already `None`). Its `Drop` impl performs the same cleanup as a fallback for early-error/panic paths: see the "Ctrl-C / SIGTERM cleanup mechanism" subsection in section 8 for the exact contract.
- Install `ctrlc` handler (set via `ctrlc::set_handler`) that ONLY flips `signal_requested.store(true, Ordering::SeqCst)` and returns. It must NOT call `std::process::exit` (would skip `Drop`); it must NOT do its own kill or log copy (would race with `Drop`); it must NOT touch `stop_helpers` (would conflate signal-vs-normal exit).
- After `SmokeSession` returns its `ExitCode` from the post-wait checks, the outer `cmd::cef_smoketest::run` reads `signal_requested.load(Ordering::SeqCst)` ONCE and overrides the result to `ExitCode::Instrument` if set. This is the only place that flag is read for exit-code purposes.
- Run the parity check against the bash on macOS before moving to Phase 4 (see "Test Plan" below).

### Phase 4: Makefile and CI cutover (1 commit)

- Edit `Makefile:138-143`:
  ```
  cef-smoketest-selftest:
  	@cargo run -p bagz-xtask --quiet -- cef-smoketest --selftest

  cef-smoketest:
  	@cargo run -p bagz-xtask --quiet -- cef-smoketest
  ```
- Edit `.github/workflows/ci.yml`:
  - The `cef-network-hardening` job (lines 28-34) currently has no Rust toolchain setup; it only checks out and runs bash. Replacing line 34 with a `cargo run -p bagz-xtask ...` invocation REQUIRES inserting `- uses: actions-rust-lang/setup-rust-toolchain@v1` (with `cache: false`, matching the other jobs) before the cargo step. Otherwise the cutover relies on ambient self-hosted-runner toolchain state.
  - Line 121 (the macOS `tauri-build-cef` job) already has a Rust toolchain via the preceding `make tauri-build` step; replace line 121 directly with `cargo run -p bagz-xtask -- cef-smoketest`.
- Audit `xtask`'s transitive dep tree against `cargo audit` policy.

### Phase 5: delete bash, update docs/specs, add justfile (1 commit)

- Delete `scripts/cef-network-smoketest.sh`.
- Add `justfile` at repo root with thin wrappers. Note that `just` parses everything after the colon on a recipe header line as a dependency list, so the command body must live on the next line, indented:
  ```just
  smoketest:
      cargo xtask cef-smoketest

  smoketest-selftest:
      cargo xtask cef-smoketest --selftest

  precommit:
      make pre-commit

  test:
      make test

  build:
      make build
  ```
- Update every reference to `scripts/cef-network-smoketest.sh` in tracked docs and specs (audit with `rg -n "scripts/cef-network-smoketest" -g '!plans/**'` (or `git grep -n "scripts/cef-network-smoketest" -- ':!plans'` if you prefer git pathspec) before deleting; known sites):
  - `CLAUDE.md:35` (three-guardrails block)
  - `docs/cef-network-hardening.md:41` (Layer 3 description; the line 30 quickstart block also lists the Make targets but those are still valid after the cutover)
  - `docs/cef.md:117`
  - `specs/001-bagz-desktop-wallet/spec.md:391` (NFR-002.a verification list)
  - `specs/001-bagz-desktop-wallet/traceability.md:29` (NFR-002.a traceability row)
- Update `AGENTS.md` direct-cargo command examples (lines 16, 17) to add `--exclude bagz-xtask` alongside `--exclude bagz-app-tauri`, matching the Make/CI audit from Phase 1. Specifically: `cargo build --workspace --exclude bagz-app-tauri --exclude bagz-xtask`, `cargo test --workspace --exclude bagz-app-tauri --exclude bagz-xtask`, `cargo clippy --workspace --all-targets --exclude bagz-app-tauri --exclude bagz-xtask`. Line 17's `cargo fmt --all` stays unchanged (fmt has no `--exclude`).
- `Makefile` stays canonical; CI calls `make` targets and the explicit `cargo run -p bagz-xtask -- cef-smoketest [...]` form directly (never the `cargo xtask` alias, which depends on `.cargo/config.toml` being loaded). The `cargo xtask` short form is reserved for human-facing surfaces: justfile recipes, docs, and the local validation block in section 7. This matches the success criteria in section 5 ("CI and Makefile use the explicit form so they do not depend on the alias being loaded").

## 5. Success Criteria

The PR is mergeable only when ALL of the following hold:

- `xtask/` crate exists with the full module layout from section 2 and is a workspace member.
- All 13 selftest fixtures are ported as `#[test]` (5 parser + 3 live-PID + 5 sample-sockets) and pass.
- `make cef-smoketest-selftest` and `make cef-smoketest` invoke `cargo run -p bagz-xtask --quiet -- cef-smoketest [--selftest]`. The shorter `cargo xtask cef-smoketest` form is reserved for human-facing surfaces (justfile, docs) where the `.cargo/config.toml` alias is documented; CI and Makefile use the explicit form so they do not depend on the alias being loaded. No remaining caller of the bash path.
- `.github/workflows/ci.yml` calls `cargo run -p bagz-xtask -- cef-smoketest [--selftest]` from both `cef-network-hardening` (with explicit `actions-rust-lang/setup-rust-toolchain@v1` setup) and `tauri-build-cef`. The new `rust-xtask` job is also present and green.
- `scripts/cef-network-smoketest.sh` is DELETED. No file in the repo (outside `plans/`) references the bash path; `rg -n "scripts/cef-network-smoketest" -g '!plans/**'` (or `git grep -n "scripts/cef-network-smoketest" -- ':!plans'` if you prefer git pathspec) returns nothing.
- All six doc/spec sites in Phase 5 are updated: the five smoketest references (`CLAUDE.md`, `docs/cef-network-hardening.md`, `docs/cef.md`, `specs/001-bagz-desktop-wallet/spec.md`, `specs/001-bagz-desktop-wallet/traceability.md`) point at the xtask command, and `AGENTS.md` direct-cargo examples include `--exclude bagz-xtask`.
- `justfile` at repo root exposes the wrappers listed in Phase 5.
- Exit codes preserved exactly: `0` pass, `1` policy violation, `2` instrumentation failure. Verified by unit tests on the `ExitCode` enum and the clap-parse-error test.
- Artifact log path preserved: default writes to `${RUNNER_TEMP:-/tmp}/bagz-cef-smoketest.log`. Verified by a unit test that reads `RUNNER_TEMP` and asserts the resolved path matches.
- App isolation preserved: the spawn path sets `BAGZ_GRPC_URL=https://127.0.0.1:1`, `BAGZ_HEADLESS_SMOKE=1`, `BAGZ_USE_SYSTEM_KEYCHAIN=0`, `BAGZ_SMOKE_DURATION_SECS=<duration>`, `BAGZ_SMOKE_READY_FILE=<smoke_root>/smoke-ready`, sandboxed `HOME`/`XDG_*`/`TMPDIR` (matching `scripts/cef-network-smoketest.sh:658-668`). Verified by an assertion test on the env map passed to `Command::spawn`.
- All `AGENTS.md` Done Criteria pass: `make test`, `make pre-commit`, `make tauri-build`.

## 6. PR Scope (files touched)

### Files added

- `xtask/Cargo.toml`
- `xtask/src/lib.rs`         (defines `pub mod cli`, `pub mod cmd`, `pub fn run`, `pub fn promote_selftest_env`; required so `xtask/tests/*` can reach internals)
- `xtask/src/main.rs`        (thin: `fn main() -> ExitCode { bagz_xtask::run() }`)
- `xtask/src/cli.rs`
- `xtask/src/cmd/mod.rs`
- `xtask/src/cmd/cef_smoketest/mod.rs`
- `xtask/src/cmd/cef_smoketest/parser.rs`
- `xtask/src/cmd/cef_smoketest/lsof.rs`
- `xtask/src/cmd/cef_smoketest/process.rs`
- `xtask/src/cmd/cef_smoketest/bundle.rs`
- `xtask/src/cmd/cef_smoketest/sampler.rs`
- `xtask/src/cmd/cef_smoketest/smoke.rs`
- `xtask/src/cmd/cef_smoketest/selftest.rs`
- `xtask/src/cmd/cef_smoketest/log.rs`
- `xtask/src/cmd/cef_smoketest/exit.rs`
- `xtask/tests/parser_fixtures.rs`
- `xtask/tests/live_pid.rs`
- `xtask/tests/sample_sockets.rs`
- `.cargo/config.toml` (new; `xtask` alias)
- `justfile` (new; Phase 5)

### Files modified

- `Cargo.toml` (root): add `xtask` to `[workspace] members`. Do not touch `default-members`.
- `Cargo.lock` (root): will regenerate. New direct deps (`ctrlc` with `termination`, `plist`) and any transitive additions land here. Commit the regenerated lockfile; CI runs with `--locked` in some jobs so a drift would fail the build.
- `Makefile`:
  - Audit and append `--exclude bagz-xtask` to: `build` (line 40), `build-release` (line 43), `test`, `clippy` (line 100), `clippy-strict` (line 103). Skip `fmt`/`fmt-check`.
  - Rewrite `cef-smoketest-selftest` and `cef-smoketest` (lines 138-143) to invoke `cargo run -p bagz-xtask --quiet -- cef-smoketest [--selftest]`.
- `.github/workflows/ci.yml`:
  - Append `--exclude bagz-xtask` to the workspace cargo invocations in `rust-clippy` (line 72), `rust-tests` (line 85), and the `rust-build` release job.
  - Add a new `rust-xtask` job: `actions/checkout@v4` + `actions-rust-lang/setup-rust-toolchain@v1` (with `cache: false` and `components: clippy`) + `cargo build -p bagz-xtask` + `cargo clippy -p bagz-xtask --all-targets -- -D warnings` + `cargo test -p bagz-xtask`. Model after `cef-args-tests` (lines 36-46).
  - `cef-network-hardening` job (lines 28-34): insert `- uses: actions-rust-lang/setup-rust-toolchain@v1` (with `cache: false`) before the existing run step, then replace line 34 with `cargo run -p bagz-xtask -- cef-smoketest --selftest`. Keep line 33 (`check-cef-network-hardening.sh`) unchanged; that script is not in scope.
  - `tauri-build-cef` job (line 121): replace with `cargo run -p bagz-xtask -- cef-smoketest`. No new toolchain step needed; `make tauri-build` already sets one up.
- `AGENTS.md` (lines 16, 17): append `--exclude bagz-xtask` to each `--workspace` direct-cargo example so the docs match the Make/CI exclude list. Line 17's `cargo fmt --all` is unchanged.
- `CLAUDE.md` (line 35): three-guardrails block now lists `cargo xtask cef-smoketest` instead of the bash path.
- `docs/cef-network-hardening.md` (line 41): Layer 3 description points at `cargo xtask cef-smoketest`. The quickstart Make targets at line 30 still work after the cutover, so leave the line 30 block as-is.
- `docs/cef.md` (line 117): replace the bash reference with `xtask/src/cmd/cef_smoketest/` or with the `cargo xtask cef-smoketest` command (pick whichever fits the surrounding prose).
- `specs/001-bagz-desktop-wallet/spec.md` (line 391): NFR-002.a verification list updates to `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs` and `cargo xtask cef-smoketest`.
- `specs/001-bagz-desktop-wallet/traceability.md` (line 29): same update for the NFR-002.a row.

### Files deleted

- `scripts/cef-network-smoketest.sh`

## 7. Test Plan

### Local validation (must pass before pushing the branch)

Run from a clean tree on macOS with a freshly built bundle:

```
# Static / unit signal
cargo build -p bagz-xtask
cargo clippy -p bagz-xtask --all-targets -- -D warnings
cargo test -p bagz-xtask
cargo run -p bagz-xtask --quiet -- cef-smoketest --selftest

# Workspace signal (Done Criteria)
make test
make pre-commit
make tauri-build

# Smoketest signal
make cef-smoketest-selftest                                  # now calls xtask
make tauri-build                                             # rebuild if needed
make cef-smoketest                                           # full live xtask run
```

The standalone `cargo clippy -p bagz-xtask` is necessary because Makefile workspace clippy (`make clippy` / `make clippy-strict`) excludes `bagz-xtask`, so running only the workspace targets would leave xtask un-linted locally. This mirrors the new CI `rust-xtask` job.

`make pre-commit` chains `fmt + clippy + check-telemetry + check-cef-network-hardening + cef-smoketest-selftest`. After the cutover, the final link in that chain is the Rust selftest.

### Parity check (Phase 3 → Phase 4 gate)

Done once on macOS while both bash and Rust still coexist on the branch (i.e. at the end of Phase 3, before the Phase 4 cutover commit). This is the human evidence that the Rust port is equivalent before we delete the bash. Resolve the artifact base directory the same way the script does so a locally-set `RUNNER_TEMP` does not silently mis-target the copy:

```sh
ARTIFACT_DIR="${RUNNER_TEMP:-/tmp}"
BASH_LOG="$ARTIFACT_DIR/bagz-cef-smoketest.log"
BASH_COPY="$ARTIFACT_DIR/bagz-bash-smoketest.log"
RUST_LOG="$ARTIFACT_DIR/bagz-rust-smoketest.log"

# 1) bash run writes to $BASH_LOG
./scripts/cef-network-smoketest.sh

# 2) snapshot it before the second run clobbers it
cp "$BASH_LOG" "$BASH_COPY"

# 3) Rust run, explicit log path so it does NOT overwrite $BASH_LOG
cargo run -p bagz-xtask -- cef-smoketest --log-path "$RUST_LOG"

# 4) human inspection; format differences are acceptable, classification
# verdict and sample count must match
diff "$BASH_COPY" "$RUST_LOG"
```

### Negative coverage

Do NOT attempt a real-network negative against the live app: both the bash script (`scripts/cef-network-smoketest.sh:664`) and the Rust port force-export `BAGZ_GRPC_URL=https://127.0.0.1:1` before spawning the child, so an outer-shell override has no effect, and changing that to "let the test reach a real endpoint" would weaken the isolation the smoketest depends on. Negative coverage lives in:

- `FakeLsof`-driven unit tests that inject crafted non-loopback `lsof` field streams (Phase 2). Each of the 5 sample-sockets fixtures already exercises a distinct failure mode.
- `parser::classify_lsof_fields` unit tests that assert exit code 1 for the `wildcard-listener`, `zero-listener`, and `external-connected` fixtures.
- Optionally, a future xtask-internal "fixture app" that intentionally binds to `0.0.0.0` and is invoked only by `cargo test --ignored`; out of scope for this PR.

### CI signal expected at PR merge time

All of these must be green:

- `rust-xtask` (new) — `cargo build -p bagz-xtask` + `cargo clippy -p bagz-xtask --all-targets -- -D warnings` + `cargo test -p bagz-xtask`
- `cef-network-hardening` — now runs Rust selftest with explicit toolchain setup
- `cef-args-tests` — unchanged
- `rust-clippy`, `rust-tests`, `rust-build` — unchanged behavior, only the `--exclude bagz-xtask` flag was added
- `tauri-build-cef` (macOS only) — runs Rust live smoketest post-build, uploads `${{ runner.temp }}/bagz-cef-smoketest.log` artifact

## 8. Risks and Decisions

### Preserving the security guarantee

- The Rust port must distinguish policy violation (exit 1) from instrumentation failure (exit 2). A future contributor merging "small cleanup" must not collapse these. Encode the distinction in the type system: `enum ExitCode { Pass, Policy, Instrument }` with `From` impls; never return raw integers from anywhere but `main`. Add a clippy lint (`-D clippy::as_conversions` on this crate) or a comment in `exit.rs` that explains the invariant.
- `BAGZ_USE_SYSTEM_KEYCHAIN=0` is set at `scripts/cef-network-smoketest.sh:668` and is load-bearing for not touching the real keychain during the test. Add an assertion test that the env map passed to `Command::spawn` contains it.
- `HOME`, `XDG_*`, `TMPDIR` sandboxing (lines 658-663) must survive the `TempDir` long enough for the child process to start. `SmokeSession` owns the `TempDir`, not the function scope.

### Alpha-stage cadence

The wallet is in alpha with no public users; that lets us land the whole port in one PR rather than a multi-PR rollout with a soak window. The trade-off is acceptable because (a) the bash and Rust paths can be diffed by hand on the branch (Phase 3 parity check above), (b) `git revert` of the merge commit is a trivial rollback, and (c) the three guardrails (`check-cef-network-hardening.sh`, `cef_runtime_args.rs`, the new xtask smoketest) are still all present and independent. This does NOT reduce the security invariant: every preserved exit code, env var, log path, and isolation setting is enumerated in section 5.

### macOS-only assumptions and cfg-gating policy

After Phase 1's `--exclude bagz-xtask` audit, the workspace cargo jobs (`rust-clippy`, `rust-tests`, `rust-build`) no longer compile `bagz-xtask` at all. Compilation and lint coverage for the crate comes ONLY from the new `rust-xtask` job, which runs on the same self-hosted runner pool as the rest of CI (current self-hosted runners are macOS). The cfg gating described below is forward-looking insurance, not current cross-platform CI coverage: it keeps the door open to add a Linux runner to `rust-xtask` later without rewiring source layout.

The Phase 2 plan REQUIRES FakeLsof-backed unit tests to run on whatever OS hosts `rust-xtask`. To make that work without coupling test compilation to macOS, gate platform code as narrowly as possible:

Cross-platform (compile and test everywhere — no cfg):

- `parser` (endpoint_host, is_loopback_host, classify_socket, classify_lsof_fields).
- `lsof` trait `LsofRunner` (the trait itself, not the impl).
- `process` trait `ProcessEnumerator` (the trait itself, not the impl).
- `sampler::sample_once` (the policy/instrumentation classifier; takes a `&dyn LsofRunner` so FakeLsof works everywhere).
- `selftest` (drives Fake* impls of the traits).
- `exit`, `log`.

macOS-only (`#[cfg(target_os = "macos")]` on the impl items, not their parent modules):

- `lsof::RealLsof` (uses macOS `lsof` syntax; see "lsof invocation" below for the exact argv).
- `process::Pgrep` (uses `pgrep -P` whose flags differ on BSD vs Linux; we standardize on macOS BSD pgrep).
- `bundle::resolve_executable` (reads `Contents/Info.plist` from a `.app`; only meaningful on macOS).
- `smoke::run_smoke` and `smoke::SmokeSession` (depend on the macOS-only impls above).

On non-macOS, `cmd::cef_smoketest::run` returns `ExitCode::Pass` with a log line "smoke not implemented for <target_os>" when invoked WITHOUT `--selftest`, matching the bash early-exit at `scripts/cef-network-smoketest.sh:635`. The `--selftest` path is fully functional on every OS because everything it touches (parser, FakeLsof, `sample_once`, the `selftest` runner) is cross-platform.

`plist` is pure Rust and cross-compiles; only `bundle::resolve_executable` (the runtime caller) is macOS-gated. `ctrlc` with `features = ["termination"]` is cross-platform.

### lsof invocation

`RealLsof::run` MUST spawn lsof with exactly this argv (from `scripts/cef-network-smoketest.sh:424`):

```
lsof -nP -a -p <pid_csv> -iTCP -iUDP -F pcPTn0
```

Pinned because each flag carries load:

- `-n` skips DNS reverse lookups (we never want the smoketest to trigger a name resolution).
- `-P` skips port-name conversion (keeps numeric ports for the parser).
- `-a` ANDs subsequent filters (without it, `-p PIDS` and `-iTCP -iUDP` would OR and report every system-wide TCP/UDP socket).
- `-p <pid_csv>` scopes to the app/helper PIDs.
- `-iTCP -iUDP` matches the protocols the bash classifies (line 424). Do NOT broaden to `-i` alone; that would pull in raw sockets and Unix domain sockets, neither of which the parser is built for.
- `-F pcPTn0` requests field-formatted output for `p` (pid), `c` (command), `P` (protocol), `T` (TCP state, emitted as `TST=<state>`), `n` (name); the trailing `0` requests NUL-terminated fields, which `parser::classify_lsof_fields` consumes.

Add a unit test on `RealLsof::build_command(&[1234, 5678])` that asserts the resulting `Command`'s program is `"lsof"` and the args vector equals `["-nP", "-a", "-p", "1234,5678", "-iTCP", "-iUDP", "-F", "pcPTn0"]`. This protects against silent argv drift if someone "tidies" the code later.

### CI artifact log path

The `actions/upload-artifact@v4` step in `.github/workflows/ci.yml:122-128` reads `${{ runner.temp }}/bagz-cef-smoketest.log`. The Rust port must write there by default. Tested by: a unit test reads `RUNNER_TEMP` and verifies the resolved path matches.

### Network failure vs instrumentation failure

The bash distinguishes these with three return paths in `sample_sockets`. The Rust port uses `enum SampleOutcome { Ok, Policy { evidence: ... }, Instrument { reason: ... } }`. The sampler thread accumulates outcomes and the final report counts each category. Never collapse "I couldn't tell" into "fail" without preserving the reason; CI relies on distinguishing them to triage flakes.

### Toolchain pin

`rust-toolchain.toml` pins `1.92.0`. Verify `plist` and `ctrlc` build on 1.92 before adding (both have been 1.60+ MSRV for a while; should be fine).

### justfile: delegate to Makefile or to xtask?

Recommendation: delegate to `cargo xtask` (and `make` for non-xtask targets like `test`, `build`). justfile is purely for human ergonomics; CI never reads it. Do not let justfile become a second source of truth for what `pre-commit` runs.

### Ctrl-C / SIGTERM cleanup mechanism

Bash `trap EXIT` fires uniformly on normal exit, error exit, SIGINT, SIGTERM, and SIGHUP. Rust `Drop` only fires on normal stack unwind, so SIGINT/SIGTERM/SIGHUP would skip cleanup unless we install a handler. The mechanism is split into three roles to avoid races and double-cleanup:

1. **Two independent flags, never overloaded.** This is the rule that prevents normal exits from being mis-classified as instrumentation failures:
   - `signal_requested: Arc<AtomicBool>` — ONLY the ctrlc handler is allowed to set this. `SmokeSession::Drop` MUST NOT touch it. The main function reads it after `run()` returns to decide whether to override the result to `ExitCode::Instrument`. Initial value `false`.
   - `stop_helpers: Arc<AtomicBool>` — set by `SmokeSession::Drop` step 1 to tell the sampler and watchdog threads to exit. Helpers check `signal_requested.load() || stop_helpers.load()` at the top of each iteration; either source wakes them.
   - `LogArtifact` (owned by `cmd::cef_smoketest::run` above `SmokeSession`) holds an internal `Once`-backed `log_copied` guard so the artifact is copied exactly once even if both the top-level `LogArtifact::Drop` and `SmokeSession::Drop`'s log-copy step race. See section 8 "Top-level log artifact guard".

2. **The ctrlc handler is intentionally tiny.** Installed once at the top of `cmd::cef_smoketest::run` via `ctrlc::set_handler(move || signal_requested.store(true, Ordering::SeqCst))`. It does NOTHING else: no kill, no log copy, no `std::process::exit`, no touching `stop_helpers`. Calling `exit` would skip `Drop` and leak the `TempDir`; doing a kill or log copy would race with `Drop`; touching `stop_helpers` would make signal-vs-normal indistinguishable.

3. **App exit drives the main wait loop, not duration.** This is the key behavioral parity with bash (`scripts/cef-network-smoketest.sh:670-693`). The app is told to self-exit via `BAGZ_SMOKE_DURATION_SECS` and a readiness sentinel; the main thread waits for the child to terminate, not for a duration timer:
   - **Hard watchdog thread** sleeps `duration + 30s` (matching bash line 670 `hard_timeout_secs=$((SMOKE_DURATION_SECS + 30))` and line 679 `sleep "$hard_timeout_secs"`). Use a `mpsc::Receiver::recv_timeout(Duration::from_secs(duration + 30))` so that a sender drop from `SmokeSession::stop_helpers_and_join` can wake it early. On timeout fire, touch `$SMOKE_ROOT/watchdog-fired`, then `process::kill_tree(...)` (bash lines 680-682). Then the main thread's `try_wait` polling will observe the now-dead child and break its loop with `WaitOutcome::Exited(status)`.
   - **Sampler thread** runs `sampler::run_loop` on a 1s tick, polling `signal_requested || stop_helpers` between samples.
   - **Main thread polls `child.try_wait()`** in a non-blocking loop that returns a typed `WaitOutcome`:
     ```rust
     enum WaitOutcome {
         Exited(ExitStatus),  // child terminated on its own (or hard watchdog killed it)
         Signaled,            // SIGINT/SIGTERM/SIGHUP arrived while child still alive
     }
     fn wait_for_child(child: &mut Child, signal_requested: &AtomicBool) -> io::Result<WaitOutcome> {
         loop {
             if signal_requested.load(Ordering::SeqCst) { return Ok(WaitOutcome::Signaled); }
             if let Some(status) = child.try_wait()? { return Ok(WaitOutcome::Exited(status)); }
             thread::sleep(Duration::from_millis(200));
         }
     }
     ```
     The typed return prevents the "child still alive but we have no `app_status`" bug. `try_wait` is non-blocking, so `Drop` always gets a chance to run later. No `duration_elapsed()` check in this loop. 200ms tick keeps shutdown latency low.
   - **Duration window enforcement happens AFTER the loop**, not inside it, and ONLY on the `Exited` branch. After child exit, compute `app_elapsed = now - start_ts` and compare against `[duration - 2, duration + 5]` (bash lines 705-709). Out-of-window is `ExitCode::Instrument` (matching bash lines 719-726, all `return 2`).
   - **Helpers are stopped and joined BEFORE reading sentinels** (see step 4 below). The sampler may still be flushing its last tick into `$SMOKE_ROOT/network-failure` or `$SMOKE_ROOT/instrumentation-failure` at the moment the child exits; reading those sentinels before the sampler has finished its current iteration would race. The explicit `stop_helpers_and_join()` call ensures we observe the sampler's final state.

4. **Post-wait sequence (matches bash `run_smoke` lines 691-738 exactly).** After `wait_for_child` returns, the main thread does:
   1. **Stop and join helpers via `session.stop_helpers_and_join()`.** This is a method on `SmokeSession`, separate from `Drop`. It sets `stop_helpers.store(true, ...)`, drops the watchdog mpsc sender (so its `recv_timeout` returns immediately), `take()`s both `Option<JoinHandle<()>>` fields out of `self`, and joins them. Idempotent: the second call (from `Drop`) is a no-op because the `Option`s are now `None`. Matches bash lines 698-701 (`kill "$WATCHDOG_PID"; wait "$SAMPLER_PID"`).
   2. **Match on the `WaitOutcome`:**
      - `WaitOutcome::Signaled` → skip all sentinel/window checks; the post-wait result is `ExitCode::Instrument` ("signal received during smoke"). `Drop` will still kill the now-orphaned child and reap it. Step 6 below will also flip the final result to `Instrument` redundantly, which is fine.
      - `WaitOutcome::Exited(status)` → run the bash-order check ladder below.
   3. **Bash-order check ladder** (only on `Exited`):
      ```
      if SMOKE_ROOT/watchdog-fired exists      -> ExitCode::Instrument ("app hung until watchdog fired")
      if SMOKE_ROOT/smoke-ready missing        -> ExitCode::Instrument ("exited before readiness sentinel")
      if app_elapsed < duration.saturating_sub(2)
                                               -> ExitCode::Instrument ("exited too early")
                                                  // saturating_sub clamps to 0 (matches bash lines 706-708)
      if app_elapsed > duration + 5            -> ExitCode::Instrument ("exited too late")
      if !status.success()                     -> ExitCode::Instrument ("non-zero exit")
      if SMOKE_ROOT/instrumentation-failure    -> ExitCode::Instrument ("lsof failed during sampling")
      if SMOKE_ROOT/network-failure            -> ExitCode::Policy ("non-loopback CEF socket observed")
      otherwise                                -> ExitCode::Pass
      ```
   These are the ONLY conditions that determine the result on the `Exited` branch. The `signal_requested` override (step 6 below) is layered on top of this.

5. **`SmokeSession::Drop` is idempotent fallback cleanup matching bash `cleanup()` (`scripts/cef-network-smoketest.sh:73-94`).** The main thread normally calls `stop_helpers_and_join()` explicitly before the post-wait checks (see step 4); Drop covers the early-error and panic paths where that call never ran. Store both `JoinHandle`s as `Option<JoinHandle<()>>` on `SmokeSession` so both `stop_helpers_and_join()` and `Drop` can `take()` each one without double-join:
   1. **Stop and join helper threads (no-op if already done).** Call `self.stop_helpers_and_join()` (idempotent: the `Option`s have already been `take()`d on the normal path, so this is a fast no-op). On the early-error path it actually does the work: `stop_helpers.store(true, ...)`, drops the watchdog sender, joins both threads. Matching bash lines 76-81.
   2. **Kill the app tree.** `process::kill_tree(&[child_pid, ...descendants])` (reverse-sorted PIDs). Idempotent: if the hard watchdog already killed the tree, the second kill is a no-op. Matching bash line 83.
   3. **Reap the child.** `self.child.wait()` ONCE here. The kill has been delivered, so this returns within ms. This is the only blocking `wait()` in the entire smoke path. Drops the zombie. Bash equivalent is implicit in `kill_tree` plus the trap returning.
   4. **Copy the log exactly once.** Call `self.log_artifact.copy_once()` (the `Once` guard lives on `LogArtifact`, see step 1 and section 8 "Top-level log artifact guard"). Whichever of `SmokeSession::Drop` or the outer `LogArtifact::Drop` runs first does the copy; the other no-ops. Matching bash line 86.
   5. **Drop the `TempDir` last.** `TempDir`'s own `Drop` removes the sandboxed `HOME`/`XDG_*`/`TMPDIR` tree. This runs implicitly when `SmokeSession` is dropped; do not call it manually. Matching bash lines 88-90.

   **Crucial: do NOT drop `TempDir` until after the post-wait checks complete.** The sentinel files (`watchdog-fired`, `smoke-ready`, `instrumentation-failure`, `network-failure`) live inside `SMOKE_ROOT`. The post-wait check ladder reads them. The natural lifetime works out because `SmokeSession` owns the `TempDir` and is held by the caller across the check ladder; `Drop` only fires after the caller returns its `ExitCode`.

6. **Signal-vs-normal exit override.** After `run()` returns its `ExitCode` (computed by the post-wait checks in step 4), `cmd::cef_smoketest::run` checks `signal_requested.load(Ordering::SeqCst)`. If set, override the return to `ExitCode::Instrument` because the run did not complete normally. This is the ONLY place `signal_requested` is read for exit-code purposes. Because `SmokeSession::Drop` never touches `signal_requested`, a normal completion stays a normal completion: the snapshot taken here only flips to `Instrument` when an actual signal was received.

**SIGKILL is unrecoverable in any language.** That is fine: CI does not send SIGKILL to test runners, and the bash `trap EXIT` would not have caught it either.

**Testing.**
- Cooperation flag: set `stop_helpers.store(true, ...)` and assert `sampler::run_loop` exits within one tick. Set `signal_requested.store(true, ...)` and assert the same.
- Override invariant: simulate a normal completion (`signal_requested` stays `false`, post-wait checks return `Pass`) and assert `run()` returns `Pass`, NOT `Instrument`. This is the regression test for the two-flag design.
- Hard watchdog: build a fake `Child` that never exits, run the smoke path with `duration_secs = 1` and assert that within ~31s the watchdog fires, kills the tree, the main loop observes child exit, and the result is `Instrument` with the `watchdog-fired` sentinel as the reason. Mark `#[ignore]` so it does not run in normal `cargo test` runs but is available via `cargo test --ignored`.
- Do NOT try to send a real SIGINT to the test process; that produces flaky cross-platform behavior.

### Top-level log artifact guard

The bash `trap EXIT` (line 95) ALWAYS runs `copy_log` (line 86) and prints `$SMOKE_LOG` (line 92), regardless of which code path failed: selftest, missing lsof, missing bundle, non-macOS early return, signal, normal completion. The Rust port must match this guarantee, otherwise a selftest run on CI that fails before any `SmokeSession` exists would produce no artifact log. `SmokeSession::Drop` only covers the live-smoke path.

The mechanism is a `LogArtifact` struct owned at the very top of `cmd::cef_smoketest::run`, BEFORE any other work:

- Construction creates the run log file (`tempfile::NamedTempFile` or similar) and resolves the artifact log path (`--log-path` > `RUNNER_TEMP` > `/tmp`).
- Public methods: `write(&self, line: &str)` (tee to file + stderr), `copy_once(&self)` (idempotent via `Once`), `artifact_path(&self) -> &Path`.
- `Drop` impl calls `copy_once()` then `println!("{}", self.artifact_path().display())` (matching bash line 92). The print-on-Drop is critical for CI log discovery.
- The shared `log_copied` guard described in step 4 of the Ctrl-C mechanism is actually a method on `LogArtifact`, not on `SmokeSession`; `SmokeSession::Drop`'s log-copy step calls `log_artifact.copy_once()` so both `Drop`s converge through the same `Once`.

This makes `LogArtifact` the single owner of "did the log get copied?" Selftest, non-macOS early returns, and pre-`SmokeSession` failures all get a copied artifact and the printed path because the outer `LogArtifact` Drop runs unconditionally as `run()` returns.

Verification: add a unit test that constructs a `LogArtifact` pointed at temp paths, drops it without calling `copy_once` explicitly, and asserts the artifact file exists.

### clap exit codes

Clap's default exit code for parse errors is `2`, which collides with our "instrumentation failure" semantic. This is acceptable because a malformed CLI in CI is itself an instrumentation failure (the workflow yaml is wrong). Document this in `exit.rs`. Do not try to remap clap's exit code; instead, pin the behavior with a unit test (`Cli::try_parse_from(...).unwrap_err().exit_code() == 2`) so a clap upgrade that shifts the default is caught at PR time.

## 9. Example Skeleton

### File tree (final state after the PR)

```
.cargo/config.toml                                  # new
xtask/
  Cargo.toml                                        # new
  src/
    lib.rs                                          # pub API surface for tests
    main.rs                                         # thin binary entry
    cli.rs
    cmd/
      mod.rs
      cef_smoketest/
        mod.rs
        parser.rs
        lsof.rs
        process.rs
        bundle.rs
        sampler.rs
        smoke.rs
        selftest.rs
        log.rs
        exit.rs
  tests/                                            # integration tests; reach
                                                    #   internals via `bagz_xtask::...`
    parser_fixtures.rs
    live_pid.rs
    sample_sockets.rs
justfile                                            # new
```

### `.cargo/config.toml`

```
[alias]
xtask = "run --package bagz-xtask --"
```

### `xtask/Cargo.toml` (illustrative; not full)

```
[package]
name = "bagz-xtask"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
anyhow.workspace = true
chrono.workspace = true
clap = { version = "4", features = ["derive", "env"] }
ctrlc = { version = "3", features = ["termination"] }
plist = "1"
tempfile.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

### `xtask/src/cli.rs` (illustrative)

```rust
use std::num::NonZeroU32;
use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "bagZ developer tools")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    CefSmoketest(CefSmoketestArgs),
}

#[derive(clap::Args)]
pub struct CefSmoketestArgs {
    /// Path to packaged .app bundle (default: target/release/bundle/macos/bagZ.app)
    #[arg(long, value_name = "PATH")]
    pub app: Option<PathBuf>,

    /// Run parser/fixture self-tests only; skip live app launch.
    /// `BAGZ_SMOKE_SELFTEST=1` also enables this; env handling is done in
    /// `main` (see below), not via clap's `env = ...`, because:
    /// (a) bash matches the string "1" exactly
    /// (`scripts/cef-network-smoketest.sh:744`); clap's `env` with a `bool`
    /// field accepts looser truthy variants which would diverge from bash,
    /// and (b) `env` plus a no-arg bool flag is awkward to configure
    /// without breaking the `--selftest` (no-value) CLI form.
    #[arg(long)]
    pub selftest: bool,

    /// Duration to keep app running. Must be > 0 to match the bash
    /// precondition (`scripts/cef-network-smoketest.sh:16-19`).
    /// NonZeroU32 makes clap reject `0` at parse time, returning exit
    /// code 2 (instrumentation failure), preserving bash semantics.
    #[arg(long, env = "BAGZ_SMOKE_DURATION_SECS", default_value = "15")]
    pub duration_secs: NonZeroU32,

    /// lsof watchdog timeout. Same > 0 requirement as duration_secs.
    #[arg(long, env = "BAGZ_LSOF_TIMEOUT_SECS", default_value = "3")]
    pub lsof_timeout_secs: NonZeroU32,

    /// Override the artifact log path. If unset, resolves at runtime to
    /// `${RUNNER_TEMP:-/tmp}/bagz-cef-smoketest.log`.
    /// Not wired to clap's `env = ...` because the bash source of truth is
    /// a directory env var (RUNNER_TEMP), not the file path itself.
    #[arg(long, value_name = "PATH")]
    pub log_path: Option<PathBuf>,
}
```

### `xtask/src/lib.rs` (illustrative)

```rust
//! Public crate API. `main.rs` is a one-liner; everything testable lives
//! here so `xtask/tests/*.rs` (integration test crates) can `use bagz_xtask::...`.

use clap::Parser;

pub mod cli;
pub mod cmd;

/// Binary entry point body. Keeps `main.rs` trivial.
pub fn run() -> std::process::ExitCode {
    let parsed = cli::Cli::parse();
    let exit = match parsed.cmd {
        cli::Cmd::CefSmoketest(mut args) => {
            // Strict bash parity: BAGZ_SMOKE_SELFTEST=1 enables selftest,
            // anything else is ignored. See
            // `scripts/cef-network-smoketest.sh:744`
            // (`[[ "${BAGZ_SMOKE_SELFTEST:-0}" == "1" ]]`).
            promote_selftest_env(&mut args, |k| std::env::var(k).ok());
            cmd::cef_smoketest::run(args)
        }
    };
    exit.into()
}

/// Pulled out as a free function (rather than calling `std::env::var`
/// inline) so unit tests can inject env values via a closure without
/// mutating process state. Required because clap's `env = ...` plus a
/// no-arg `bool` field is awkward to configure for strict-"1" semantics.
/// `pub` so `xtask/tests/*.rs` can call it directly.
pub fn promote_selftest_env<F>(args: &mut cli::CefSmoketestArgs, get_env: F)
where
    F: Fn(&str) -> Option<String>,
{
    if !args.selftest && get_env("BAGZ_SMOKE_SELFTEST").as_deref() == Some("1") {
        args.selftest = true;
    }
}
```

### `xtask/src/main.rs` (illustrative)

```rust
fn main() -> std::process::ExitCode {
    bagz_xtask::run()
}
```

The single-variant `match` inside `run()` is intentional: it stays correct when `Cmd` grows a second variant (e.g. when `check-cef-network-hardening` is later ported), and it avoids `clippy::irrefutable_let_patterns` under the planned `-D warnings` lint for `rust-xtask`. Do not regress this to `if let`.

Test coverage required for `promote_selftest_env` (lives in `xtask/tests/`, calls `bagz_xtask::promote_selftest_env`):

- `BAGZ_SMOKE_SELFTEST=1` with no `--selftest` flag → `args.selftest == true`.
- `BAGZ_SMOKE_SELFTEST=true` / `BAGZ_SMOKE_SELFTEST=yes` / `BAGZ_SMOKE_SELFTEST=0` / unset → `args.selftest == false` (matches bash).
- `--selftest` flag without env → `args.selftest == true`.

Each test calls `promote_selftest_env` with a closure that returns a canned value, so process env is never touched and tests run in parallel safely.

### Critical files referenced

- `scripts/cef-network-smoketest.sh` (deleted by Phase 5)
- `scripts/check-cef-network-hardening.sh` (unchanged)
- `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs` (unchanged)
- `Cargo.toml` (root): add `xtask` to members
- `Makefile` lines 40, 43, 100, 103, 138-143
- `.github/workflows/ci.yml` lines 28-34, 72, 85, 115-128
- `docs/cef-network-hardening.md` line 41
- `docs/cef.md` line 117
- `specs/001-bagz-desktop-wallet/spec.md` line 391 (NFR-002.a)
- `specs/001-bagz-desktop-wallet/traceability.md` line 29 (NFR-002.a row)
- `CLAUDE.md` line 35 (three-guardrails block)
- `AGENTS.md` lines 16, 17 (direct-cargo command examples; add `--exclude bagz-xtask`)
