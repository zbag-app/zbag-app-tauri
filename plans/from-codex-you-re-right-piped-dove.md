# CEF Network Hardening: Lock Down and Detect

## Context

bagZ's move to Tauri's experimental CEF runtime (default branch `cef`, pinned at Tauri rev `6fd733b` per `Cargo.toml:93-95`, `cef` crate `148.0.0+147.0.10`, CEF binary `chromium-147.0.7727.118`) introduced default Chromium browser behavior. A Little Snitch capture showed the bagZ process tree opening connections to:

- Google update / Omaha (`gvt1.com`, `clients.l.google.com`, `clients2.google.com`)
- Google services / Privacy Sandbox (`googleapis.com`, `accounts.google.com`, `ogads-pa.clients6.google.com`)
- Chromium Secure DNS probes (`chrome.cloudflare-dns.com`, `doh.opendns.com`)
- Other defaults (`youtube.com` via media-engagement preconnect, `gstatic`)

None of these are reached by our Rust code. They are all Chromium internal services that survived the earlier password-manager-only CEF hardening at `apps/bagz-app-tauri/src-tauri/src/lib.rs:41-54` (pre-fix). This violates NFR-002 in `specs/001-bagz-desktop-wallet/spec.md:390` (no remote telemetry) and silently bypasses the project's fail-closed Tor principle, since CEF's native network stack does not honor Arti.

The team previously documented (`docs/cef-password-hardening.md:57`) that aggressive `--disable-features=...` bundles destabilized CEF startup. Upstream has since improved: Tauri PR #15252 (null pointer on shutdown, merged 2026-04-16) and PR #15279 (re-entrancy guard for user-event callbacks, merged 2026-04-28) are both ancestors of our pinned rev `6fd733b` (committed 2026-05-14). The strict path is now viable.

Codex landed the strict path in uncommitted edits on the current branch (`overnight/bagz-rebrand-refresh-20260515`):

- 19 plain switches plus `--disable-features` (15 features), `--dns-over-https-mode=off`, empty DoH templates, `--webrtc-ip-handling-policy=disable_non_proxied_udp`, and `--host-resolver-rules=MAP * 0.0.0.0 EXCLUDE localhost,127.0.0.1,::1,*.localhost,ipc.localhost,tauri.localhost` (`apps/bagz-app-tauri/src-tauri/src/lib.rs:33-120`).
- Per-launch temp cache with legacy and stale-temp cache purge (`apps/bagz-app-tauri/src-tauri/src/lib.rs:122-216`).
- Browser-preferences hardening for Safe Browsing, search suggestions, spellcheck, translate, sign-in, network prediction, Privacy Sandbox, autofill (`apps/bagz-app-tauri/src-tauri/src/lib.rs:232-374`).
- Static check script `scripts/check-cef-network-hardening.sh` wired into `make pre-commit` and `make check` (`Makefile:111,113,121-122`).
- Doc `docs/cef-network-hardening.md` (new) plus updates to `docs/cef-password-hardening.md` and `docs/cef.md`.

Codex reports its packaged-app smoke observed zero TCP/UDP sockets across the app and helper PIDs. This plan accepts that strict path as the foundation. It does NOT add more Chromium feature names (per user decision), but documents the optional Tier 2/Tier 3 additions in a reference doc for future use.

What this plan adds on top of Codex's work:

1. Stronger detection so the regression cannot return silently: three layers (static, structured cargo test, runtime smoke test).
2. A constitution amendment (NFR-002.a) and traceability entry naming the three artifacts.
3. A Tier reference doc capturing optional Tier 2/Tier 3 additions and a staged peel-back ordering, in case CEF/Chromium upgrades require re-tuning. Tier 2 and Tier 3 stay documentation-only unless the Layer 3 smoke observes a leak.
4. A follow-up note that swap-related UI copy should warn users about IP exposure to `1click.chaindefuser.com` when Tor is off (no code change in this plan; UI doc only).

Codex review incorporated across eight rounds:

Round 1:
- Layer 1 static check uses Rust structural anchors (`const CEF_...`, `fn cef_runtime_args`, `fn enforce_cef_browser_policy`), not line numbers, since line numbers drift.
- Layer 2 cargo integration test is the strongest layer (operates on parsed args, not text shape); flagged mandatory.
- Layer 3 smoke isolates `HOME`/`XDG_*`/`TMPDIR` and forces `BAGZ_GRPC_URL=https://127.0.0.1:1` so the test exercises CEF cold-start, not real wallet networking on the runner.
- Layer 3 smoke peer policy is loopback-only (`127.0.0.0/8`, `::1`); `0.0.0.0` is NOT a safe peer and any listening socket bound to `0.0.0.0` or a non-loopback address also fails. Runs as a step inside the existing `tauri-build-cef` CI job (no separate job, no `.app` artifact round trip).

Round 2:
- CI runner is `self-hosted` (`.github/workflows/ci.yml:81`), NOT GitHub-hosted `macos-latest`. The smoke script detects platform at runtime (`uname -s`); the Darwin path uses `lsof`. Non-Darwin hosts log "smoke not implemented for this OS" and exit 0 (Linux/Windows are explicit follow-ups, not partial implementations).
- `cef-smoketest` Makefile target is dependency-free (no `cef-smoketest: tauri-build`) to avoid a double-rebuild when CI runs both. CI calls the script directly after the existing build step.
- Layer 1 static check ALSO scans the `cef_runtime_args` extracted region for bare switch names (no `--` prefix) to catch builder-style calls like `cef_switch_value("enable-features", ...)` that the `--`-prefixed forbidden list would miss.
- Layer 3 `lsof` parsing uses field set `-F pcPTn0` (not `L`) to distinguish listeners from connected peers via `TST=LISTEN` / `TST=ESTABLISHED` state in the `T` field; addresses are read from the `n` field. Validates both remote peers AND local listen addresses (a non-loopback listener is a regression signal even if no traffic flowed).

Round 3:
- `lsof` field set corrected: `L` is process login name, not local listen address. Addresses live in the `n` field, state in the `T` field (`TST=LISTEN`, `TST=ESTABLISHED`, ...). Final field set: `-F pcPTn0`.
- `lsof` invocation uses `-a` to AND selectors: `lsof -nP -a -p "$pid_csv" -iTCP -iUDP -F pcPTn0`. Without `-a`, `-p` and `-i` OR together and can return sockets from other processes.
- `mktemp -d` invocation uses an explicit template path (`mktemp -d "${TMPDIR:-/tmp}/bagz-cef-smoketest.XXXXXX"`) for portability; BSD's `-t prefix` and GNU's `-t TEMPLATE` are incompatible.

Round 4:
- Smoke log is copied to a stable path (`${RUNNER_TEMP:-/tmp}/bagz-cef-smoketest.log`) before the `trap EXIT` cleanup deletes `SMOKE_ROOT`. CI uploads the stable path via `${{ runner.temp }}/bagz-cef-smoketest.log` (NOT a relative path).
- The `lsof -F pcPTn0` parser has its own self-test mode (`BAGZ_SMOKE_SELFTEST=1`) with four synthetic-record fixtures (loopback listener PASS, wildcard listener FAIL, external connected peer FAIL, loopback connected peer PASS). Self-test runs in `make pre-commit` since it requires no packaged bundle.

Round 5:
- Layer 1 region extraction uses explicit marker comments (`// CEF_HARDENING_SWITCHES_BEGIN/END`, `// CEF_HARDENING_VALUED_ARGS_BEGIN/END`, `// CEF_HARDENING_PREFS_BEGIN/END`) added to lib.rs. `awk '/fn .../, /^}/'` is broken for Rust functions because nested `}` (e.g., the macOS keychain `if`) closes the awk range early. The two `concat!(...)` consts (`CEF_DISABLED_FEATURES`, `CEF_HOST_RESOLVER_RULES`) keep `awk '/.../, /);/'` because the macro body has no nested `);`.
- Smoke `lsof` invocation is wrapped with `|| true` so its non-zero exit on "no matching sockets" (which is the success state we want) does not kill the script under `set -e`. Empty output is treated as PASS.
- Removed stale `cef-runtime-smoketest` from the required-gates list in verification step 7; the smoke runs inside `tauri-build-cef`.
- CI artifact upload uses the absolute log path `${{ runner.temp }}/bagz-cef-smoketest.log`, not a relative filename.

Round 6:
- Layer 2 forbidden-switch tests normalize keys via `trim_start_matches('-')` before comparing to `enable-features`, `proxy-server`, `proxy-pac-url`, `remote-debugging-port`, `remote-debugging-pipe`. The previous draft compared against `--enable-features` literally and would miss a refactor that changed the prefix handling.
- Smoke script gains a hard-timeout watchdog: `HARD_TIMEOUT_SECS = BAGZ_SMOKE_DURATION_SECS + 30`. Catches pre-`.setup()` crashes and indefinite hangs; kills app + helper PIDs; copies the log to the stable path; exits with code 2 to distinguish a hang (2) from a phone-home regression (1) from a clean run (0).
- Linux and Windows are explicit follow-ups, NOT partial implementations. Script logs "smoke not implemented for this OS" and exits 0 if `uname -s` is not `Darwin`. CI workflow step is also OS-guarded.
- `tauri-build-cef` `timeout-minutes` increased from 30 to 40 to absorb the new smoke step (up to ~45s hard cap) plus artifact upload variability.
- Cosmetic: prior "two rounds, eight amendments" wording replaced with "six rounds".

Round 7:
- Watchdog uses a sentinel file (`$SMOKE_ROOT/watchdog-fired`) so the main script can deterministically detect that the watchdog fired and return exit code 2. The previous draft only wrote a log line, which the main script could not act on after `wait`.
- Process tree teardown uses an explicit recursive descendant enumerator instead of `pkill -P`. `pkill -P` only kills direct children; CEF helper processes can spawn grandchildren that would survive. Reuses the same `enumerate_descendants` helper used by the socket sampler.
- Removed stale Linux `ss` parsing references throughout the smoke section. Smoke is Darwin-only; non-Darwin hosts log "smoke not implemented" and exit 0. The CI step is OS-guarded.
- CI artifact upload condition is explicitly `if: always()` AND path `${{ runner.temp }}/bagz-cef-smoketest.log` (exact match the script writes to). No relative paths.

Round 8:
- Early-exit detection: watchdog alone catches hangs, not early crashes. If the app crashed before `.setup()` and exited quickly, the previous design could spuriously PASS (no watchdog sentinel, no sockets observed). Fix: the Rust `.setup()` branch writes a `smoke-ready` sentinel via env var `BAGZ_SMOKE_READY_FILE`; the script requires the sentinel to exist AND the elapsed time within `[BAGZ_SMOKE_DURATION_SECS - 2, BAGZ_SMOKE_DURATION_SECS + 5]` (asymmetric tolerance: 2s lower for jitter, 5s upper because teardown is sometimes slow) AND the app exit status to be zero. Any other classification yields exit code 2.
- Final cleanup of stale Darwin-vs-Linux/lsof-vs-ss summary wording. The implementation is Darwin-only for runtime smoke; the static (Layer 1) and Cargo (Layer 2) layers and the parser self-test are OS-independent.

Non-goals: Tor-mandatory gating of CoinGecko / 1click / lightwalletd (deferred; user said the existing Tor toggle and UI labels are acceptable). Adding more Chromium feature names beyond the current 15 (deferred to Tier 2 doc only, promoted only on smoke-observed leak).

## Implementation steps

Numbered with parallelization annotations. After approval, each becomes a TaskCreate entry; non-trivial steps execute via `general-purpose` subagents.

1. **Review and commit Codex's CEF hardening** (Critical path; blocks 2-8)
   - Inspect `git status` + `git diff` for the uncommitted set: `Makefile`, `apps/bagz-app-tauri/src-tauri/src/lib.rs`, `docs/cef-password-hardening.md`, `docs/cef.md` (modified) and `docs/cef-network-hardening.md`, `scripts/check-cef-network-hardening.sh` (new).
   - Run `make check-cef-network-hardening && make check && make tauri-build`.
   - Manual smoke: launch the built `.app`, watch Little Snitch, confirm only `zec.rocks` (or configured lightwalletd) appears in the bagZ tree.
   - Commit as a single landing commit (subject: `fix(cef): disable Chromium phone-home, enforce offline renderer`).

2. **Layer 1: Restructure the static check** (Parallelizable with 3, 4, 6, 7)
   - File: `scripts/check-cef-network-hardening.sh`.
   - Today the script greps the whole file with `rg -Fq`; a literal inside a comment satisfies the gate. Per Codex review, do NOT hardcode line numbers (they drift). Use anchor-based region extraction by structural names.
   - File (also touched): `apps/bagz-app-tauri/src-tauri/src/lib.rs`.
     - **Marker comments (Codex correction R8):** `awk '/fn .../, /^}/'` extraction breaks on Rust functions because nested `}` closes (e.g., the macOS keychain `if` inside `cef_runtime_args`, the `for ... { ... }` loop, the various nested blocks in `enforce_cef_browser_policy`) terminate the awk range early. A brace-depth-aware extractor in shell is fragile; a Rust/Python parser is overkill for one file. Use explicit marker comments around each hardening region instead. Add to lib.rs:
       - `// CEF_HARDENING_SWITCHES_BEGIN` immediately above the `for switch in [...]` loop inside `cef_runtime_args`, and `// CEF_HARDENING_SWITCHES_END` immediately after the loop.
       - `// CEF_HARDENING_VALUED_ARGS_BEGIN` and `// CEF_HARDENING_VALUED_ARGS_END` around the `args.push(cef_switch_value(...))` block (`--disable-features`, `--dns-over-https-mode`, `--dns-over-https-templates`, `--host-resolver-rules`, `--webrtc-ip-handling-policy`).
       - `// CEF_HARDENING_PREFS_BEGIN` and `// CEF_HARDENING_PREFS_END` around the body of `enforce_cef_browser_policy` (or specifically the pref-write block within it).
     - The `CEF_DISABLED_FEATURES` and `CEF_HOST_RESOLVER_RULES` consts are `concat!(...)` macros bounded by `);`; for those, `awk '/const NAME.*concat!/, /);/'` is robust because there are no nested `);` inside the macro body.
   - Replace the single `REQUIRED_LITERALS` array with four region-anchored arrays. Extract each region via `awk`:
     - `REQUIRED_SWITCHES`: extract the block `awk '/CEF_HARDENING_SWITCHES_BEGIN/,/CEF_HARDENING_SWITCHES_END/'` and grep within for each entry of the expected switch list.
     - `REQUIRED_VALUED_ARGS`: extract `awk '/CEF_HARDENING_VALUED_ARGS_BEGIN/,/CEF_HARDENING_VALUED_ARGS_END/'` and grep within for `disable-features`, `dns-over-https-mode`, `host-resolver-rules`, `webrtc-ip-handling-policy`.
     - `REQUIRED_DISABLED_FEATURES`: extract `awk '/const CEF_DISABLED_FEATURES.*concat!/,/);/'` and grep within for each expected feature name.
     - `REQUIRED_HOST_RESOLVER_EXCLUDES`: extract `awk '/const CEF_HOST_RESOLVER_RULES.*concat!/,/);/'` and grep within.
     - `REQUIRED_PREFS`: extract `awk '/CEF_HARDENING_PREFS_BEGIN/,/CEF_HARDENING_PREFS_END/'` and grep within for each expected pref key (`safebrowsing`, `dns_over_https`, `network_prediction_options`, `search`, `signin`, `spellcheck`, `translate`, `autofill`, `credentials_enable_service`).
     - `FORBIDDEN_LITERALS` (must NOT exist anywhere in `apps/bagz-app-tauri/src-tauri/`): `--enable-features=`, `--proxy-server=`, `--proxy-pac-url=`, `dns-over-https-mode=automatic`, `dns-over-https-mode=secure`, raw hostnames `googleapis.com`, `gvt1.com`, `clients2.google.com`, `cloudflare-dns.com`, `doh.opendns.com`, `youtube.com`, `gstatic.com`.
     - `FORBIDDEN_IN_RUNTIME_ARGS` (Codex correction 3): the `cef_runtime_args` extracted region must NOT contain any of the bare switch names `enable-features`, `proxy-server`, `proxy-pac-url`, `remote-debugging-port`, `remote-debugging-pipe`. This catches builder-style invocations like `cef_switch_value("enable-features", ...)` that the `--`-prefixed `FORBIDDEN_LITERALS` would miss. Layer 2 also catches this; Layer 1 has it as a fast pre-commit signal.
   - Anchor patterns are intentionally tied to Rust structural names (`const CEF_...`, `fn cef_runtime_args`, `fn enforce_cef_browser_policy`). Renaming these in `lib.rs` requires updating the script in the same commit, which is acceptable churn and far less fragile than line numbers.
   - Acceptance test: replacing `cef_runtime_args` body with `Vec::new()` while leaving all comments intact MUST fail the gate. Also, moving the const declarations elsewhere in the file but keeping their names MUST still pass.
   - Layer 2 below is the structural enforcer; Layer 1's job is fast pre-commit feedback. Keep Layer 1 broad and rely on Layer 2 for semantic guarantees.

3. **Layer 2: Cargo integration test on `cef_runtime_args()`** (Parallelizable with 2, 4, 6, 7). MANDATORY, strongest layer.
   - Per Codex review: this layer is the strongest because it asserts on the parsed `Vec<(String, Option<String>)>` data structure that the runtime actually receives, not on text shape. Any future regression that mutates the policy semantically (a renamed switch, an additional `--enable-features`, a host-resolver EXCLUDE leak) is caught here even if Layer 1's text scan happens to pass.
   - File: `apps/bagz-app-tauri/src-tauri/src/lib.rs`.
     - Make `cef_runtime_args` (lib.rs:73), `CEF_DISABLED_FEATURES` (lib.rs:33), `CEF_HOST_RESOLVER_RULES` (lib.rs:52) `pub` under the same `cfg` gate.
     - Optionally extract to a new module `apps/bagz-app-tauri/src-tauri/src/cef_args.rs` (cleaner) and re-export.
   - File (new): `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs`.
     - `#![cfg(all(feature = "cef-runtime", not(feature = "test-bridge")))]`.
     - Tests operate on the parsed `Vec<(String, Option<String>)>`, not strings:
       - `required_switches_present`: every entry in `EXPECTED_SWITCHES` exists with value `None`.
       - `disabled_features_exact_set`: comma-split `--disable-features` value, assert set-equality with `EXPECTED_DISABLED_FEATURES` (mirrors the current 15-name list).
       - `host_resolver_rules_exact`: parse `--host-resolver-rules`, assert starts with `MAP * 0.0.0.0` and EXCLUDE list is exactly `{localhost, 127.0.0.1, ::1, *.localhost, ipc.localhost, tauri.localhost}`.
       - `no_enable_features_switch`: no key normalized via `trim_start_matches('-')` equals `enable-features`. Codex correction R11: do not match against the `--`-prefixed form only; a caller using `cef_switch_value("enable-features", ...)` would produce `--enable-features` while a future refactor that changes the helper to emit a different prefix would slip past. Normalize, then compare.
       - `dns_over_https_off`: `--dns-over-https-mode = Some("off")` AND `--dns-over-https-templates = Some("")`.
       - `webrtc_non_proxied_udp_disabled`: `--webrtc-ip-handling-policy = Some("disable_non_proxied_udp")`.
       - `no_proxy_or_remote_debugging`: reject any key whose normalized form (`key.trim_start_matches('-')`) matches `proxy-server`, `proxy-pac-url`, `remote-debugging-port`, or `remote-debugging-pipe`. Same R11 rationale.
   - File: `Makefile`.
     - Add `check-cef-args: ; cargo test -p bagz-app-tauri --features cef-runtime --test cef_runtime_args`.
     - Add `check-cef-args` to `pre-commit` and `check` deps.

4. **Layer 3: Runtime lsof smoke test on packaged app (Darwin only)** (Parallelizable with 2, 3, 6, 7). Amended per Codex review for isolation, stricter peer policy, CI co-location, watchdog with sentinel, recursive process-tree kill.
   - File: `apps/bagz-app-tauri/src-tauri/src/lib.rs`.
     - Inside the existing `.setup(|app| { ... })` block (around the existing Tor `start_if_enabled` call), branch on `std::env::var("BAGZ_HEADLESS_SMOKE").is_ok()`:
       - Parse `BAGZ_SMOKE_DURATION_SECS` (default 15).
       - **Smoke-ready sentinel (Codex correction R19):** if `BAGZ_SMOKE_READY_FILE` is set, write a single byte to that path (`std::fs::write(path, "1")`). Best-effort; ignore I/O errors but log via `tracing::warn!`. This proves `.setup()` actually ran; the script uses it to distinguish a clean smoke from a pre-`setup()` crash.
       - Spawn an async task that sleeps `BAGZ_SMOKE_DURATION_SECS` then calls `app_handle.exit(0)`.
     - Rationale: this is CI-only behavior, opens no IPC surface, and the `test-bridge` `compile_error!` at lib.rs:5-6 is unaffected.
   - File (new): `scripts/cef-network-smoketest.sh`.
     - Usage: `./scripts/cef-network-smoketest.sh [path-to-bundled-app]` (default `target/release/bundle/macos/bagZ.app`).
     - **State isolation (Codex amendment 1):** the script MUST run the app under a freshly-created temp directory tree so it cannot inherit a real wallet, real Tor config, or a real lightwalletd endpoint from the runner. The script:
       - Creates `SMOKE_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/bagz-cef-smoketest.XXXXXX")`. (Codex correction R3: BSD `mktemp -d -t prefix` is not portable; GNU mktemp interprets `-t` differently. Using an explicit template path works on both macOS and Linux.)
       - Exports `HOME="$SMOKE_ROOT/home"`, `XDG_CACHE_HOME="$SMOKE_ROOT/cache"`, `XDG_CONFIG_HOME="$SMOKE_ROOT/config"`, `XDG_DATA_HOME="$SMOKE_ROOT/data"`, `XDG_STATE_HOME="$SMOKE_ROOT/state"`, `TMPDIR="$SMOKE_ROOT/tmp"`, and creates each directory.
       - Exports `BAGZ_GRPC_URL=https://127.0.0.1:1` so even if the app tries to reach lightwalletd at startup, it targets a guaranteed-dead local address. Reference: `CLAUDE.md` documents this env override.
       - Exports `BAGZ_HEADLESS_SMOKE=1`, `BAGZ_SMOKE_DURATION_SECS=15`, and `BAGZ_SMOKE_READY_FILE=$SMOKE_ROOT/smoke-ready` (Codex correction R19; the Rust `.setup()` branch writes this file once it runs, so the script can confirm `.setup()` was reached before the app exited).
       - Exports `BAGZ_USE_SYSTEM_KEYCHAIN=0` (default) so the macOS keychain branch in `cef_runtime_args` stays at `--use-mock-keychain`.
       - Tears down `SMOKE_ROOT` on `trap EXIT`.
     - macOS path: spawn `<app>/Contents/MacOS/bagZ` in the background. Capture `APP_PID`. Enumerate the full descendant PID tree (CEF spawns helper processes that own their own sockets; grandchildren exist too). `pgrep -P $PID` alone returns ONLY direct children, so implement an explicit recursive walker in shell:
       - `enumerate_descendants() { local parent="$1"; local kids; kids=$(pgrep -P "$parent" 2>/dev/null || true); for k in $kids; do echo "$k"; enumerate_descendants "$k"; done; }`
       - Call as `ALL_PIDS=$(enumerate_descendants "$APP_PID" | sort -u; echo "$APP_PID")` and refresh each sample (CEF may spawn helpers lazily).
     - Sample once per second for ~13s.
     - **Linux: unsupported in this plan (Codex correction R13).** The macOS `lsof` path is the only fully specified runner. Linux gets no partial `ss` parsing in this implementation; that creates false confidence. When/if a Linux runner is added, this plan must be amended with an explicit ss-based classifier, the same listener-vs-connected logic, and the same loopback rule. Until then, the script exits 0 with a log line "smoke not implemented for $(uname -s)" on any non-Darwin host, AND the CI job step is OS-guarded so it only runs on `runner.os == 'macOS'`. Track as a follow-up issue when a Linux build job is added.
     - Windows: skip in this plan (follow-up; same posture as Linux above).
     - **`lsof` parsing (Codex correction 4 + R1):** `-F pn` alone does not distinguish listeners from connected peers. Use field set `-F pcPTn0`. Field meanings per `man lsof`: `p`=pid, `c`=command, `P`=protocol (TCP/UDP), `T`=TCP/TPI state (e.g., `TST=LISTEN`, `TST=ESTABLISHED`, `TST=SYN_SENT`), `n`=name (the address(es): for listeners `*:PORT` or `127.0.0.1:PORT`, for connected sockets `LOCAL->REMOTE`). Do NOT use `L` (that is the process login name, NOT the local listen address). Use the `0` field code so records are null-separated and addresses containing `:` do not corrupt the parser.
     - **`lsof` invocation (Codex correction R2 + R9):** lsof selectors default to OR, so `lsof -p PID -i` can return unrelated network sockets. Use `-a` to AND the selectors. Build a CSV of pid + helper pids and invoke once per sample:
       - `output=$(lsof -nP -a -p "$pid_csv" -iTCP -iUDP -F pcPTn0 2>/dev/null || true)`
       - `-n` skips DNS reverse lookups (deterministic output), `-P` skips port name resolution.
       - **Tolerate no-match exit (Codex correction R7):** `lsof` exits non-zero when no matching sockets exist. Under `set -e`, a successful run with zero sockets would kill the script. Wrap with `|| true` and treat empty `$output` as PASS (zero sockets is the goal). Linux/Windows are unsupported (see above), so this is Darwin-only.
     - Parse each null-separated record and classify by `T` field:
       - `TST=LISTEN` → listener; the `n` field is the bind address. Allowed iff bound to `127.0.0.0/8` or `[::1]`. Bind to `*`, `0.0.0.0`, `[::]`, or any non-loopback IP fails the run.
       - `TST=ESTABLISHED|SYN_SENT|FIN_WAIT_*|TIME_WAIT|CLOSE_WAIT|...` → connected; the `n` field is `LOCAL->REMOTE`. Allowed iff REMOTE is in `{127.0.0.0/8, ::1}`. Any other remote fails.
       - UDP (no `T` state, protocol `P=UDP`): if the `n` field shows a remote (`A->B`), apply the connected rule. Bare bound UDP sockets (`*:PORT`) are treated as listeners and follow the listener rule.
     - **Strict peer rule (Codex amendment 2):** the smoke exits 1 on any record violating the above. Do NOT allow `0.0.0.0` or `[::]` as a listener address. Do NOT allow `*.localhost` literal at the socket layer (DNS-only EXCLUDE; sockets see resolved IPs).
     - On any failure, dump the full offending lsof record (all parsed fields) plus the sample timestamp to the log, then exit 1.
     - **Parser fixture tests (Codex correction R5):** the `lsof -F pcPTn0` parser is the sharpest part of the implementation and must have its own unit tests. Add to the smoke script (or extract into a small helper script invoked from `make check`) a self-test mode `BAGZ_SMOKE_SELFTEST=1` that runs against fixed sample inputs and asserts the classifier verdict. Minimum three fixtures:
       - Loopback listener: synthetic record `p1234\0cbagZ\0PTCP\0TST=LISTEN\0n127.0.0.1:7777\0` MUST classify as listener and PASS.
       - Wildcard listener: synthetic record `p1234\0cbagZ\0PTCP\0TST=LISTEN\0n*:7777\0` MUST classify as listener and FAIL with reason "non-loopback bind". Same fixture with `n0.0.0.0:7777` MUST also FAIL.
       - External connected peer: synthetic record `p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->142.250.190.78:443\0` MUST classify as connected and FAIL with reason "non-loopback remote".
       - Loopback connected peer: synthetic record `p1234\0cbagZ\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->127.0.0.1:7777\0` MUST PASS.
     - Run `BAGZ_SMOKE_SELFTEST=1 ./scripts/cef-network-smoketest.sh` as a step inside `pre-commit` (it has no runtime dependencies and is fast). Wire via a new `cef-smoketest-selftest` Makefile target so the parser is exercised on every commit, separately from the full packaged-app run.
     - **Stable log path (Codex correction R4):** the log MUST NOT live inside `SMOKE_ROOT`, because the `trap EXIT` cleanup deletes that directory before CI uploads the artifact. Write the live log to `$SMOKE_ROOT/run.log` during execution for I/O locality, then on completion (pass OR fail) copy the final log to a stable path outside `SMOKE_ROOT`: `SMOKE_LOG="${RUNNER_TEMP:-/tmp}/bagz-cef-smoketest.log"`. The trap can safely delete `SMOKE_ROOT` afterwards. CI uploads `$SMOKE_LOG`. The script prints `$SMOKE_LOG` to stdout as its last line so callers can find it.
     - **Watchdog (Codex correction R12 + R16 sentinel + R17 recursive kill):** the app might crash before `.setup()` runs (so the `BAGZ_HEADLESS_SMOKE` timer never starts) or hang indefinitely. The script MUST enforce an outer hard timeout. Implementation:
       - `HARD_TIMEOUT_SECS=$(( BAGZ_SMOKE_DURATION_SECS + 30 ))` (smoke duration plus 30s buffer for startup and teardown; for the default 15s smoke this gives 45s total).
       - Define a kill helper that uses the same recursive PID enumerator as the sampler (do NOT use `pkill -P` alone; it misses grandchildren):
         - `kill_tree() { local pids; pids=$( { enumerate_descendants "$APP_PID"; echo "$APP_PID"; } | sort -u); for p in $pids; do kill -KILL "$p" 2>/dev/null || true; done; }`
       - Start a background watchdog that drops a sentinel file on fire so the main script can detect it after `wait`:
         - `( sleep "$HARD_TIMEOUT_SECS"; echo "WATCHDOG: hard timeout after ${HARD_TIMEOUT_SECS}s" >> "$SMOKE_ROOT/run.log"; touch "$SMOKE_ROOT/watchdog-fired"; kill_tree ) &`
         - Record `WATCHDOG_PID=$!`.
       - Main script records start time (`START_TS=$(date +%s)`), then `wait "$APP_PID"` and captures status (`set +e; wait "$APP_PID"; APP_STATUS=$?; set -e`). After wait:
         - `kill "$WATCHDOG_PID" 2>/dev/null || true` (suppress watchdog if app finished naturally).
         - `ELAPSED=$(( $(date +%s) - START_TS ))`.
         - **Lifecycle detection (Codex correction R20 + R21 late-exit):** classify in order. The bounds are asymmetric because app teardown can run a few seconds slower than ideal, while early exits indicate a real crash:
           - `[[ -f "$SMOKE_ROOT/watchdog-fired" ]]` → exit code 2 (hang; watchdog killed the app after `HARD_TIMEOUT_SECS`).
           - `[[ ! -f "$SMOKE_ROOT/smoke-ready" ]]` → exit code 2 (app crashed before `.setup()` ran; the Rust sentinel was never written).
           - `(( ELAPSED < BAGZ_SMOKE_DURATION_SECS - 2 ))` → exit code 2 (app crashed AFTER `.setup()` but well before its scheduled exit; 2-second lower tolerance for scheduler jitter). Log `ELAPSED` and `APP_STATUS`.
           - `(( ELAPSED > BAGZ_SMOKE_DURATION_SECS + 5 ))` → exit code 2 (app exited cleanly but much later than the scheduled timer; signals a broken smoke timer or a setup branch that did not schedule the exit. 5-second upper tolerance is wider than the lower tolerance because teardown can be a little slower. This still fires before the hard watchdog at `BAGZ_SMOKE_DURATION_SECS + 30`, so it gives a more informative diagnosis than letting the watchdog catch it).
           - `(( APP_STATUS != 0 ))` → exit code 2 (app exited near the expected time but with non-zero status; treated as crash for safety).
         - Else if any sample observed a non-loopback peer/listener → exit code 1 (phone-home regression).
         - Else → exit code 0 (clean: `.setup()` ran, app ran within `[BAGZ_SMOKE_DURATION_SECS - 2, BAGZ_SMOKE_DURATION_SECS + 5]` seconds, exited with status 0, no non-loopback sockets).
       - The `trap EXIT` handler always (a) kills the watchdog if still running, (b) calls `kill_tree` to clean up any surviving descendant PIDs (recursive, not `pkill -P`), (c) copies `$SMOKE_ROOT/run.log` to `$SMOKE_LOG`, (d) removes `$SMOKE_ROOT`. Use a flag (`COPIED=1`) to ensure the copy step only happens once even if EXIT fires recursively.
   - File: `Makefile`.
     - **No dependency on `tauri-build` (Codex correction 2):** Makefile targets are PHONY in this repo, and adding `cef-smoketest: tauri-build` causes a double-rebuild when CI runs both. Make the target dependency-free, just invoke the script:
       - `cef-smoketest: ; ./scripts/cef-network-smoketest.sh`
     - Document in the Makefile comment that the target requires a prebuilt bundle at `target/release/bundle/macos/bagZ.app`; the caller is responsible for running `make tauri-build` first.
     - NOT added to `pre-commit` (requires a built bundle). CI-only.
   - File: `.github/workflows/ci.yml`.
     - **Co-locate with the build job (Codex amendment 3):** do NOT create a separate `cef-runtime-smoketest` job that downloads the `.app` artifact. The `.app` bundle can lose executable bits, codesign state, or arch metadata after an artifact upload/download round trip. Instead, append a step to the existing `tauri-build-cef` job (currently at `.github/workflows/ci.yml:79-`, `runs-on: self-hosted` per line 81):
       - After the existing build step (which runs `make tauri-build`), add a new step that calls `./scripts/cef-network-smoketest.sh` directly (NOT `make cef-smoketest`, so the no-dep target above is enforced for callers who DO use the Makefile but the CI invocation stays explicit and audit-friendly).
       - **Artifact upload (Codex correction R10 + R18 exact match):** the upload step MUST use `if: always()` AND `path: ${{ runner.temp }}/bagz-cef-smoketest.log` (exact, absolute). Do NOT use a relative path; the working directory at upload time is not guaranteed and the script writes to `$RUNNER_TEMP` (workflow expression `${{ runner.temp }}`).
     - **Runner assumption (Codex correction 1):** the existing job runs on `self-hosted`, NOT `macos-latest`. Implementation must NOT introduce assumptions about a GitHub-hosted macOS image, signing certificates, or default tool versions; the script detects whether the runner is macOS (`uname -s` = `Darwin`) and only then runs the smoke; on Linux/Windows it logs "smoke not implemented for this OS" and exits 0 (per Codex correction R13, Linux/Windows are explicit follow-ups, not partial implementations).
     - **Timeout headroom (Codex correction R14):** the existing `tauri-build-cef` job has `timeout-minutes: 30` (`.github/workflows/ci.yml:83`). Adding the smoke step (up to `HARD_TIMEOUT_SECS = BAGZ_SMOKE_DURATION_SECS + 30 = 45s` plus artifact upload) plus normal CI variability can make marginal runs flaky. Increase to `timeout-minutes: 40` in the same edit.

5. **Layer 5: Constitution + traceability + memory** (Depends on 1; parallelizable with 6, 7)
   - File: `specs/001-bagz-desktop-wallet/spec.md` (NFR-002 around line 390).
     - Append NFR-002.a: "CEF MUST NOT emit any non-loopback network traffic during normal operation. Enforced by (1) deny-all `--host-resolver-rules`, (2) disabled background services and DoH, (3) ephemeral per-launch cache and browser-prefs hardening, (4) a CI runtime smoketest asserting zero non-loopback sockets for 15s after cold start. Verified by: `scripts/check-cef-network-hardening.sh`, `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs`, `scripts/cef-network-smoketest.sh`."
   - File: `specs/001-bagz-desktop-wallet/traceability.md`.
     - Add row mapping NFR-002.a to the three artifacts above.
   - File: `.specify/memory/constitution.md`.
     - Update the Non-Negotiable Checklist to reference NFR-002.a.
   - File: `CLAUDE.md` (project-level).
     - Add a "CEF network hardening: do not regress" section pointing at the three artifacts. Warn that editing `cef_runtime_args` / `CEF_DISABLED_FEATURES` / `CEF_HOST_RESOLVER_RULES` requires updating the matching `EXPECTED_*` constants in the integration test.

6. **Tier reference doc** (Parallelizable with 2, 3, 4, 7)
   - File (new): `docs/cef-network-hardening-tiers.md`.
     - **Promotion gate (Codex amendment 4):** Tier 2 features and Tier 3 (`--user-data-dir`) stay documentation-only. They are NOT applied to `cef_runtime_args` until and unless the Layer 3 smoke (step 4) observes a real leak that the current Tier 1 set fails to block. This avoids preemptive feature-flag churn that could destabilize CEF startup without evidence of a need. The doc must state this gate explicitly at the top.
     - Section "Tier 1 (current, shipped)": the 19 switches, 15 features, host-resolver deny-all, prefs hardening shipped today. List each verbatim with rationale.
     - Section "Tier 2 (documented, NOT shipped)": Privacy Sandbox v3 + media + prefetch names from the audit: `BrowsingTopics`, `Fledge`, `InterestGroupStorage`, `AttributionReporting`, `PrivateAggregationApi`, `SharedStorageAPI`, `FencedFrames`, `NetworkTimeServiceQuerying`, `NetworkQualityEstimator`, `Reporting`, `NetworkErrorLogging`, `Prerender2`, `Preconnect`, `LoadingPredictorUseLocalPredictions`, `PushMessaging`, `BackgroundSync`, `BackgroundFetch`, `WidevineCdm`, `DialMediaRouteProvider`, `CastMediaRouteProvider`, `MediaRemoting`, `HttpsUpgrades`, `WebBluetooth`, `WebUsb`, `WebHID`. Note: Chromium silently ignores unknown feature names so an over-broad list is safe but obscures auditability.
     - Section "Tier 3 (documented, NOT shipped): explicit `--user-data-dir` pin": rationale (some Chromium subsystems consult `--user-data-dir` separately from `root_cache_path`, so an explicit `--user-data-dir=<cef_runtime_cache_path>` provides belt-and-suspenders). Apply only if smoke evidence shows a non-loopback peer despite Tier 1.
     - Section "Staged peel-back if startup breaks": ordered list (Tier 0 current → 1 → 2 → 3 minimum-viable) from the audit, with a per-tier validation procedure (rebuild, launch, Little Snitch check).
     - Section "Upstream stability fixes referenced": Tauri PRs #15252 (2026-04-16) and #15279 (2026-04-28), both ancestors of pinned rev `6fd733b` (2026-05-14).
     - Section "Validation procedure": A/B by removing one tier at a time and watching Little Snitch.

7. **Swap UI IP-leak documentation follow-up note** (Parallelizable with 2, 3, 4, 6)
   - File (new): `docs/swap-ip-leak-followup.md`.
     - Document: the swap subsystem (`crates/bagz-network/src/near_intents.rs:10` → `https://1click.chaindefuser.com`) reveals the user's IP to a third party when Tor is off. The existing Tor toggle (`commands::tor::bagz_set_tor_enabled`) and Tor-state surface are correct, but swap-specific UI screens should add an explicit copy block such as: "Requesting a swap quote contacts a third-party service (1Click). Without Tor, your IP address is visible to that service." Same applies to swap initiation and status polling.
     - Mark as follow-up; no code change in this plan. Owner: frontend.

8. **Final validation pass** (Depends on 1-7)
   - `make pre-commit` (static + cargo args test + telemetry guard + fmt + clippy) → green.
   - `make check` (above + full test suite) → green.
   - `make tauri-build` → packaged `.app`.
   - `make cef-smoketest` → packaged app boots in headless smoke mode for 15s, `lsof` (Darwin only) observes zero non-loopback peers across app + helper PIDs.
   - Manual Little Snitch run: launch, create wallet, sync, view balance. Confirm bagZ tree contains only the configured lightwalletd host (plus `api.coingecko.com` if fiat enabled, `1click.chaindefuser.com` if swap UI opened). NO Google / Cloudflare / OpenDNS / gvt1 / googleapis / youtube hosts.
   - Regression rehearsal: temporarily remove `--host-resolver-rules` from `cef_runtime_args` and confirm all three layers fail (Layer 1 reports missing literal, Layer 2 reports missing key, Layer 3 reports non-loopback sockets). Restore; all three pass.
   - Open PR; required gates: `telemetry-guard`, `cef-network-hardening` (Layer 1), `cef-args-tests` (Layer 2), `rust-clippy`, `rust-tests`, `tauri-build-cef` (which now also runs the Layer 3 smoke as an internal step). There is NO separate `cef-runtime-smoketest` gate; a smoke failure surfaces as a `tauri-build-cef` failure with the uploaded `${{ runner.temp }}/bagz-cef-smoketest.log` artifact.

## Critical files

| File | Steps | Action |
|---|---|---|
| `apps/bagz-app-tauri/src-tauri/src/lib.rs` | 1, 2, 3, 4 | Accept Codex edits (1); add marker comments `CEF_HARDENING_*_BEGIN/END` around the three regions (2); make `cef_runtime_args` + 2 consts `pub` or extract to `cef_args.rs` (3); add `BAGZ_HEADLESS_SMOKE` branch in `setup` (4) |
| `apps/bagz-app-tauri/src-tauri/src/cef_args.rs` (new, optional) | 3 | Extraction module if not exposing in lib.rs directly |
| `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs` (new) | 3 | Cargo integration test |
| `scripts/check-cef-network-hardening.sh` | 2 | Restructure to four arrays + forbidden list + region-anchored matching |
| `scripts/cef-network-smoketest.sh` (new) | 4 | Packaged-app runtime smoke |
| `Makefile` | 3, 4 | New `check-cef-args` target (wired into pre-commit/check); new dependency-free `cef-smoketest` target that runs the script only (caller must build first; avoids double-build); new `cef-smoketest-selftest` target that runs the parser fixture self-test (wired into pre-commit) |
| `.github/workflows/ci.yml` | 4 | Append a step to the existing `tauri-build-cef` job (line 79+, `runs-on: self-hosted` per line 81) that calls `./scripts/cef-network-smoketest.sh` directly after the build step (OS-guarded to Darwin); upload `${{ runner.temp }}/bagz-cef-smoketest.log` on `if: always()`; increase `timeout-minutes` from 30 to 40. Do NOT assume `macos-latest`. |
| `specs/001-bagz-desktop-wallet/spec.md` | 5 | NFR-002.a sub-clause |
| `specs/001-bagz-desktop-wallet/traceability.md` | 5 | NFR-002.a → three artifacts |
| `.specify/memory/constitution.md` | 5 | Non-Negotiable Checklist update |
| `CLAUDE.md` | 5 | "Do not regress" section |
| `docs/cef-network-hardening-tiers.md` (new) | 6 | Tier 1/2/3 reference + peel-back |
| `docs/swap-ip-leak-followup.md` (new) | 7 | UI copy follow-up for swap |

## Reuse existing utilities

- `cef_switch` / `cef_switch_value` helpers at `apps/bagz-app-tauri/src-tauri/src/lib.rs:63-70`. The cargo test compares parsed args; no change needed beyond `pub` exposure.
- Existing `scripts/check-no-telemetry.sh:1-48` exit-code convention (0 pass, 1 fail, 2 tool error). New scripts in steps 2 and 4 follow the same shape.
- Existing `lsof` usage pattern at `scripts/e2e-test.sh:127`. The new smoke script reuses the same invocation style.
- Existing `tauri-build-cef` CI job. Per Codex amendment 3, the runtime smoke test runs as an additional step inside this same job, immediately after `make tauri-build`. No new job, no artifact upload/download round trip.
- Existing `make pre-commit` / `make check` aggregation pattern at `Makefile:111,113`. The new gates extend these without changing the pattern.

## Verification

1. `make pre-commit` → Layer 1 (static) + Layer 2 (cargo args test) + telemetry guard + format + clippy all green. Sub-second to seconds.
2. `make check` → all of the above plus full test suite. Minutes.
3. `make tauri-build` → packaged `.app` produced.
4. `make cef-smoketest` (new) → launches `target/release/bundle/macos/bagZ.app/Contents/MacOS/bagZ` under an isolated `HOME`/`XDG_*`/`TMPDIR` and with `BAGZ_GRPC_URL=https://127.0.0.1:1 BAGZ_HEADLESS_SMOKE=1 BAGZ_SMOKE_DURATION_SECS=15`, samples sockets every second for ~13s across app + helper PIDs, asserts every observed peer is loopback (`127.0.0.0/8`, `::1`). Any other peer, any listening socket bound to `0.0.0.0` or a non-loopback address, fails the run. Exit 0 on pass.
5. Manual: Little Snitch (or equivalent network monitor) shows only the configured lightwalletd host plus the two opt-in Rust-side hosts (`api.coingecko.com`, `1click.chaindefuser.com`) when their UIs are used. NO Chromium phone-home hosts.
6. Regression rehearsal (proves the layers actually catch regressions, not just pass on the happy path):
   - Remove `--host-resolver-rules` from `cef_runtime_args` → Layer 1 fails (missing literal), Layer 2 fails (missing key), Layer 3 fails (non-loopback sockets observed). Restore → all green.
   - Add `--enable-features=BrowsingTopics` to `cef_runtime_args` → Layer 1 fails (forbidden literal), Layer 2 fails (no_enable_features_switch). Remove → green.
   - Replace one feature name in `CEF_DISABLED_FEATURES` (e.g., `Translate` → `Translater`) → Layer 1 fails (missing literal), Layer 2 fails (set inequality). Restore → green.
7. CI: open a PR and confirm required gates are: `telemetry-guard`, `cef-network-hardening` (Layer 1 static), `cef-args-tests` (Layer 2 cargo), `rust-clippy`, `rust-tests`, `tauri-build-cef`. The Layer 3 smoke runs as a step inside `tauri-build-cef`, so its failure surfaces as a `tauri-build-cef` failure with the uploaded log artifact attached.
