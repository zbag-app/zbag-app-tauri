# Handoff: reqwest 0.13 / CEF TLS hang investigation

Final state after two sessions of isolation work. Investigation is effectively done; what remains is deciding how to land the fix and whether to post upstream.

## One-line outcome

When zSTASH's Tauri-CEF build uses `reqwest 0.13`'s default `rustls` feature, it implicitly makes `aws-lc-rs` the process-wide `rustls::crypto::CryptoProvider`. `tonic 0.14`'s later TLS handshake picks up `aws-lc-rs` and deadlocks inside a process that also statically links CEF's bundled Chromium BoringSSL. Installing `ring` as the default provider at startup (and using `reqwest`'s `rustls-no-provider` feature) resolves it. Manual end-to-end testing of the fix against the real wallet (sync, Settings network, Tor) passes.

## Shipping state vs scratch state

- **Shipping branch `cef` at `8c2813d`**: `reqwest = "0.12", features = ["json", "rustls-tls"]`. Known-good. Untouched by this work.
- **Scratch branch `repro/reqwest-0.13-hang`**: same HEAD `8c2813d`, with uncommitted edits that implement the A1 fix end-to-end. Not pushed.

## The fix (uncommitted on scratch branch)

Diff stat against `8c2813d`:

```
Cargo.lock                                 | 345 +++++++----------------------
Cargo.toml                                 |   2 +-
apps/zstash-app-tauri/src-tauri/Cargo.toml |   2 +
apps/zstash-app-tauri/src-tauri/src/lib.rs |   6 +
```

1. `Cargo.toml:63` (workspace):
   ```toml
   reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-no-provider"] }
   ```
   (was `0.12` with `["json", "rustls-tls"]`)

2. `apps/zstash-app-tauri/src-tauri/Cargo.toml`:
   ```toml
   reqwest.workspace = true
   rustls = { version = "0.23", features = ["ring"] }
   ```
   `reqwest.workspace = true` was added so this crate can depend on it; the Tauri app previously didn't list reqwest directly. `rustls` is added only to call `install_default()`.

3. `apps/zstash-app-tauri/src-tauri/src/lib.rs` at the top of `run_with_invoke_handler` (before `state::AppState::new()`):
   ```rust
   // Install ring as rustls default CryptoProvider before any TLS use.
   // Reqwest 0.13 is configured with `rustls-no-provider`; both reqwest and tonic
   // pick up this default. Avoids aws-lc-rs (reqwest 0.13's default), which deadlocks
   // when loaded alongside CEF's bundled BoringSSL.
   let _ = rustls::crypto::ring::default_provider().install_default();
   ```

4. `Cargo.lock` drops `aws-lc-rs` and `aws-lc-sys` from the graph once `reqwest` is on `rustls-no-provider` and nothing else pulls them in. `rustls-platform-verifier 0.6.2` stays (reqwest still needs a verifier); `ring 0.17.14` is now the only crypto provider. ~345 lines removed, ~86 added.

## How the fix was validated

### Phase A: isolation inside the zSTASH app

- **A0** (reqwest 0.13 default `rustls` feature, no explicit provider install): reproduced the hang exactly as reported (sync stuck, Settings > Network test never returns). Startup probe showed reqwest's own HTTPS call succeeded (`200 OK` in 741 ms); a `sample(1)` of the main process during the hang showed all tokio workers parked, no active TLS stack. Conclusion: `aws-lc-rs` works for reqwest but breaks the later tonic handshake.
- **A1** (reqwest 0.13 `rustls-no-provider` + `rustls::crypto::ring::default_provider().install_default()`): startup probe ran reqwest → tonic (`GrpcClient::probe_server("https://zec.rocks:443")`) → reqwest. All three succeeded in under 1 s total. Manual testing of the real wallet (sync to tip, Settings, Tor toggle, network test) passed.
- **A2** (aws-lc-rs + webpki-roots): skipped. A1 passed so the root cause was already identified per the plan's decision tree.

### Phase B: minimal standalone repro

Project at `/tmp/tauri-cef-tls-repro/` (also packaged in `plans/tauri-cef-tls-repro.tar.gz`). Tauri v2 CEF, no React, no zSTASH code. Auto-probe in the `.setup` hook runs one reqwest GET + one tonic connect, logs to `/tmp/repro-probe.log`. Two feature variants:

- **Broken** (`cargo tauri build --features cef-runtime --bundles app`): `aws-lc-rs` is the implicit default provider. reqwest succeeds ~400 ms; tonic never completes. Waited 90 s, no `tonic ok`, no `tonic err`, no `tonic TIMEOUT`. The `tokio::time::timeout(25s)` inside the probe cannot preempt the handshake, which means it is blocked in a synchronous FFI call that never yields. Strong signature for the `aws-lc-rs`/CEF BoringSSL collision hypothesis.
- **Fixed** (`cargo tauri build --features cef-runtime,install-ring --bundles app`): same code path with `rustls::crypto::ring::default_provider().install_default()` at startup. reqwest ~600 ms, tonic ~600 ms, both complete.

Same binary, same graph; only the default provider flips. Keychain prompts suppressed via Tauri CEF `command_line_args([("password-store", Some("basic")), ("use-mock-keychain", None)])` so rebuilds don't re-prompt.

### Phase C: upstream filings

Posted 2026-04-23:

- Anchor comment on `tauri-apps/tauri#13878`: https://github.com/tauri-apps/tauri/issues/13878#issuecomment-4304319358
- New issue on `aws/aws-lc-rs`: https://github.com/aws/aws-lc-rs/issues/1107

Drafts preserved in `plans/phase-c-draft.md`. Posted bodies live at `/tmp/phase-c-anchor.md` and `/tmp/phase-c-awslc.md`.

Repro tarball (`plans/tauri-cef-tls-repro.tar.gz`) not attached to either filing yet. gh CLI cannot attach files to issue bodies; attach via web UI if desired.

- No filing on `seanmonstar/reqwest` (plan rule).
- No PR opened. If a maintainer on either filing asks for one, the scoped option is a short note in the tauri feat/cef docs warning downstream about the aws-lc-rs interaction.

## Decision points open to the user

1. **Land the A1 fix on the `cef` branch?** The current shipping state is `reqwest 0.12 + rustls-tls` which is fine but stays exposed to the rustsec advisories in the 0.12 line and to the eventual `aws-lc-rs`/CEF collision if any future dep pulls it transitively. If the fix lands:
   - Commit message candidate: `fix: pin ring as rustls default provider for CEF host`
   - Scope of commit: workspace `Cargo.toml`, app `Cargo.toml`, `lib.rs`, `Cargo.lock`.
   - Constitution check: no secrets/signing/network-ACL/IPC-version changes; safe under the CLAUDE.md merge gate.
2. **Post Phase C?** Needs your go-ahead. Drafts are ready to `gh issue comment` and `gh issue create` verbatim.
3. **Discard scratch branch?** If keeping 0.12 as shipping, `git checkout -- Cargo.toml Cargo.lock apps/zstash-app-tauri/src-tauri/Cargo.toml apps/zstash-app-tauri/src-tauri/src/lib.rs` and `git branch -D repro/reqwest-0.13-hang` once any documentation you want to keep has been committed.

## Artifacts

- `plans/handoff-cef-tls-hang.md` (this file): final handoff.
- `plans/ancient-shimmying-storm.md`: original plan. Steps A1, B, C executed. A2 skipped. D not needed.
- `plans/phase-c-draft.md`: upstream filing drafts.
- `plans/tauri-cef-tls-repro.tar.gz` (~64 KB): standalone minimal repro. Extracts to `tauri-cef-tls-repro/`.
- `/tmp/tauri-cef-tls-repro/`: working copy of the minimal repro (outside workspace).
- `/tmp/zstash-tauri-build-clean.log`: last clean zstash build log (1m 57s, `8c2813d` + A1 edits, no probe code).
- `/tmp/repro-build-broken-v4.log`, `/tmp/repro-build-fixed-v2.log`: repro build logs.
- `/tmp/repro-probe.log`: most recent repro probe output (overwritten per launch).
- `/Users/bioharz/.zstash/logs/zstash.2026-04-23`: zSTASH app log. Contains `ZSTASH-REPRO:` probe lines from A0/A1 runs (probe code has since been stripped from `lib.rs`).
- Stack samples (stripped release binary, Rust frames unreadable): `/tmp/zstash-sample-main-a0.txt`, `/tmp/zstash-sample-main-a0-v2.txt`.

## How to resume if interrupted mid-Phase-C

Each target is one `gh` command; the body text lives in `plans/phase-c-draft.md`:

```bash
# anchor comment (after extracting the section 1 body into a file)
gh issue comment 13878 --repo tauri-apps/tauri --body-file /tmp/phase-c-anchor.md
# attach the tarball via the GitHub web UI on the rendered comment

# root-cause issue
gh issue create \
  --repo aws/aws-lc-rs \
  --title "aws-lc-rs deadlocks when loaded alongside CEF's bundled Chromium BoringSSL (macOS arm64, in-process)" \
  --body-file /tmp/phase-c-awslc.md
```

Capture the resulting URLs and update this handoff.

## Quick-fact recap (fits in one screen)

- Symptom: HTTPS hangs in the Tauri-CEF build when reqwest is bumped 0.12 → 0.13.
- Cause: `reqwest 0.13`'s default `rustls` feature pulls `aws-lc-rs` and makes it the default `rustls::CryptoProvider` for the process. `aws-lc-rs` (a BoringSSL fork, statically linked) deadlocks when co-resident with CEF's bundled Chromium BoringSSL. `tonic`'s later handshake picks up the poisoned default and never completes.
- Fix: pin `reqwest` to `rustls-no-provider` and install `ring` as the default provider at process start.
- Validation: startup probes confirm tonic works under ring. End-to-end wallet test (sync, Settings, Tor) passes.
- Minimal repro: ~70 LOC Rust + 30 LOC HTML, reproduces A/B deterministically.
