# Plan: isolate reqwest 0.13 + CEF TLS hang, report upstream

## Context

While doing the dependency refresh in the previous session, we bumped `reqwest` in the workspace from `0.12 + rustls-tls` to `0.13 + rustls`. The app compiled, built, bundled, and launched, but every outbound HTTPS request hung: sync stuck indefinitely, the Settings > Network test button never returned, and the UI locked while pending. Reverting to `reqwest 0.12 + rustls-tls` restored networking (commit `8c2813d` on branch `cef`).

`reqwest 0.13`'s `rustls` feature changed two primitives at the same time vs `0.12`:
- crypto provider: ring → `aws-lc-rs` (a BoringSSL fork)
- trust store: `webpki-roots` → `rustls-platform-verifier` (OS trust store, `SecTrustEvaluateWithError` on macOS)

Either one could plausibly break TLS inside a CEF-hosted process. CEF statically links Chromium's BoringSSL into the main browser process, which is the natural collision surface for `aws-lc-rs`. The platform verifier, separately, has known sandbox-related pitfalls on macOS.

We want to isolate the root cause, build a minimal standalone reproduction, and file a concrete upstream report. `seanmonstar/reqwest` maintainers respond well to repros; we want one ready before we open anything.

## Goals

1. Determine which of the two 0.13 changes (aws-lc-rs vs platform-verifier) is the cause in a CEF host.
2. Produce a minimal Tauri+CEF reproduction project (outside zSTASH's workspace) that an upstream maintainer can clone and run.
3. Comment on `tauri-apps/tauri#13878` (near-dup, macOS reqwest hang in production builds) with our isolation data + repro link.
4. File a focused issue on the correct downstream repo (`aws/aws-lc-rs`, `rustls/rustls-platform-verifier`, or `seanmonstar/reqwest`) based on isolation result.

## Non-goals

- Changing zSTASH's shipped reqwest version. Branch `cef` stays on `0.12` regardless of what we find.
- Fixing CEF itself or patching BoringSSL.
- Opening a PR upfront. We only open a PR if a maintainer explicitly asks, and only as a doc-only change (see Phase D).
- A full Tauri+CEF example app with frontend framework; repro is plain HTML.

## Prior-art signals (from Phase 1 research)

- `tauri-apps/tauri#13878` (open): macOS production build with reqwest hangs all HTTPS. Near-dup; our data strengthens it.
- `seanmonstar/reqwest#2924` (open): `rustls-no-provider` init requirement not documented.
- `seanmonstar/reqwest#2423` (open): debate about letting rustls pick default provider. Contentious; do not wade in.
- `seanmonstar/reqwest#3009` (closed): `aws-lc-sys` linker collisions on Linux. Different layer, not the same bug.
- No confirmed prior reproductions of aws-lc-rs + CEF's BoringSSL in-process, nor of rustls-platform-verifier hanging inside a CEF main process on macOS.
- `reqwest 0.13` changelog confirms the `rustls-tls` → `rustls` rename and both primitive changes.

## Execution plan

Each step inside a phase is serial unless annotated otherwise. Each full `make tauri-build` is ~4 minutes.

### Phase A: Isolate (serial)

A0. **Stack dump first, may obsolete A1/A2.** Relaunch the current reqwest 0.13 + `rustls` build (checkout the pre-revert state from the reflog or cherry-pick the Cargo.toml change onto a scratch branch). Reproduce the hang. While it is hung, `sample <pid> 5` on the main Rust thread and the tokio worker. If the stuck frame is inside `SecTrustEvaluateWithError`, platform-verifier is confirmed and we can skip A1. If the stuck frame is inside `aws_lc_sys` or `SSL_CTX_*` symbols, aws-lc-rs is confirmed and we can skip A2.

A1. **Variant "ring + platform-verifier"** (run first unless A0 already answered it; aws-lc-rs collision is the more novel hypothesis, worth isolating first).
   - Edit `Cargo.toml` line 63 to:
     ```toml
     reqwest = { version = "0.13", default-features = false, features = ["rustls-no-provider", "json"] }
     rustls = { version = "0.23", features = ["ring"] }
     rustls-platform-verifier = "0.3"
     ```
   - Add `rustls::crypto::ring::default_provider().install_default().ok();` at the top of `HttpClient::new_with_transport` in `crates/zstash-network/src/http_client.rs`.
   - `make tauri-build`, launch, click Settings > Network test, observe within 5s.
   - **Result interpretation:** pass → aws-lc-rs is the culprit. Proceed to Phase B with this variant as the baseline for the "fixed" side. Fail → crypto provider is not the issue; platform-verifier is suspect; proceed to A2.
   - **Rollback:** `git checkout -- Cargo.toml crates/zstash-network/src/http_client.rs` after test.

A2. **Variant "aws-lc-rs + webpki-roots"** (only if A1 did not conclude).
   - `reqwest = { version = "0.13", default-features = false, features = ["rustls-no-provider", "json"] }`
   - Install aws-lc-rs provider, build a custom `ClientConfig` using `webpki-roots::TLS_SERVER_ROOTS` instead of the platform verifier. Plumb it through `reqwest::Client::builder().use_preconfigured_tls(...)`.
   - Rebuild, launch, test.
   - **Result interpretation:** pass → platform-verifier is the culprit. Fail → both are broken; see escape hatch.
   - **Rollback:** same as A1.

**Escape hatches:**
- Both variants hang → the collision is not isolated to either changed primitive; suspect BoringSSL symbol interposition at `dlopen` time regardless of provider. Attach both stack dumps to the upstream filing and file against CEF as well as reqwest.
- Both variants pass → build non-determinism; re-run each variant 3x, diff resolved lockfiles against the failing 0.13-rustls lockfile, narrow further.

### Phase B: Minimal standalone repro (after A)

B1. Create `/tmp/tauri-cef-tls-repro/` via `cargo new`. Standalone crate, NOT inside zSTASH's workspace.

B2. Dependencies: `tauri = { git = "...", rev = "562bc592...", default-features = false, features = ["compression", "common-controls-v6", "dynamic-acl", "cef"] }`, and the reqwest variant that reproduces. No React. No zstash crates. Plain HTML UI.

B3. Single Tauri command:
   ```rust
   #[tauri::command]
   async fn fetch() -> Result<u16, String> {
       let client = reqwest::Client::new();
       client.get("https://www.google.com").send().await
           .map(|r| r.status().as_u16())
           .map_err(|e| e.to_string())
   }
   ```

B4. `index.html` with one button that invokes `fetch` and prints the result. Target: < 200 LOC Rust + < 50 LOC HTML.

B5. `cargo tauri build`, launch, click button, confirm hang. If it does not repro minimally, add pieces back one at a time (tauri plugins, second async runtime, etc.) until it does. Each narrowing step is evidence.

B6. Persist the project as a tarball in the zSTASH plans/ directory for attachment. Only push to a public GitHub repo if the upstream issue asks for one.

### Phase C: File upstream

C1. Comment on `tauri-apps/tauri#13878` with: our isolation result from Phase A, the minimal repro from Phase B (attach tarball or link), the 0.12 → 0.13 diff as the cause, and "reverting reqwest restores network". This is the anchor filing; keep it as the single source of truth.

C2. **If A identified aws-lc-rs:** open one issue on `aws/aws-lc-rs` titled "aws-lc-rs and CEF's bundled BoringSSL coexist poorly in-process (macOS arm64)". Include stack dump, repro link, and a pointer to the tauri#13878 comment. Do NOT file on `seanmonstar/reqwest` for this variant; the root cause is upstream of reqwest.

C3. **If A identified platform-verifier:** open one issue on `rustls/rustls-platform-verifier` titled "SecTrustEvaluateWithError hangs inside Tauri CEF-hosted macOS process". Include stack dump + repro link. Do NOT file on reqwest; reqwest is a consumer here.

C4. **If escape hatch fired (both broken):** file at CEF + cross-link. Lower priority, lower confidence.

### Phase D: PR (gated)

D1. Do not open a PR preemptively. Only if a maintainer replies asking for one.

D2. If asked, scope is doc-only. Candidates:
   - `seanmonstar/reqwest`: README note about provider init requirements in embedded-Chromium hosts.
   - `tauri-apps/tauri`: a feat/cef guide note warning downstream about aws-lc-rs + BoringSSL.

D3. Do not touch `seanmonstar/reqwest#2423`'s default-provider debate. Behavior-change PRs there are contentious.

## Critical files

- `/Users/bioharz/git/zcash/bagz/bagz-cef-git/Cargo.toml` (line 63, reqwest feature toggle for A1/A2)
- `/Users/bioharz/git/zcash/bagz/bagz-cef-git/crates/zstash-network/src/http_client.rs` (HttpClient init, add `install_default()` call)
- `/Users/bioharz/git/zcash/bagz/bagz-cef-git/Cargo.lock` (inspect after each variant, revert via checkout)
- (new, throwaway) `/tmp/tauri-cef-tls-repro/` (minimal standalone repro)
- `/Users/bioharz/git/zcash/bagz/bagz-cef-git/plans/` (store repro tarball here if attached to issue)

## Reusable patterns

- `make tauri-build` is the canonical entry point; every isolation variant uses it.
- `docs/cef.md` already documents the Frameworks/ audit recipe; we reuse this check in verification.
- Our prior upstream filing (`tauri-apps/tauri#15287`) is a reference for tone: concrete repro + source pointer.

## Verification

Per isolation variant:
1. `cargo audit` stays at 0 errors.
2. `make tauri-build` exits 0.
3. `target/release/bundle/macos/zSTASH.app/Contents/Frameworks/` contains `Chromium Embedded Framework.framework` + 5 helper apps.
4. Launch via `open -na ...zSTASH.app` produces main + helper processes in `pgrep -fa zstash-app-tauri`.
5. Settings > Network test returns HTTP 200 within 5 seconds. Hang > 30s = FAIL.

Per repro project:
1. `cargo tauri build` in `/tmp/tauri-cef-tls-repro/` exits 0.
2. Launched app's fetch button reproduces the hang on our machine.
3. README (under 40 lines) describes: rust version, build command, expected vs observed behavior.

## Rollback

All isolation changes are confined to `Cargo.toml` + `crates/zstash-network/src/http_client.rs`. After each variant (pass or fail), `git checkout -- Cargo.toml Cargo.lock crates/zstash-network/src/http_client.rs` restores the known-good commit `8c2813d` state. Branch `cef`'s final state after this plan is unchanged: reqwest 0.12 + rustls-tls, full security-patched Cargo.lock. The minimal repro lives outside the workspace; nothing in the zSTASH tree depends on it.
