# Phase C: upstream filing drafts

Drafts only. Do **not** post without user approval.

Built from Phase A isolation + Phase B minimal repro. Two targets, one comment + one new issue.

## Artifacts to attach

- `plans/tauri-cef-tls-repro.tar.gz` (64 KB): minimal standalone project
- Probe outputs (broken vs fixed) quoted inline below

## 1. Anchor: comment on `tauri-apps/tauri#13878`

Target: https://github.com/tauri-apps/tauri/issues/13878

### Title

(reply comment, no title)

### Body

```markdown
Data point that looks like the same root cause, isolated against the CEF feature.

## Environment

- tauri + tauri-build pinned to `tauri-apps/tauri@562bc592b337de417aa48e72034c7816cfb4c142` (feat/cef snapshot, 2026-04)
- CEF 146.4.1+146.0.9 via `cef-runtime` feature
- macOS arm64, Rust 1.92, release build
- App: a Zcash wallet that uses both `reqwest 0.13` for outbound HTTPS and `tonic 0.14` for lightwalletd gRPC

## Isolation

Bumping `reqwest` from `0.12 rustls-tls` to `0.13 rustls` causes every outbound HTTPS call inside the CEF-hosted process to hang. Reverting to 0.12 restores networking.

`reqwest 0.13`'s default `rustls` feature changes two primitives at once vs `0.12`:
- crypto provider: `ring` → `aws-lc-rs` (a BoringSSL fork)
- trust store: `webpki-roots` → `rustls-platform-verifier`

Phase A1 in our project isolated the crypto provider as the cause:

- **Broken**: `reqwest 0.13` with default `rustls` feature. No explicit `CryptoProvider` install. Observation: `reqwest` HTTPS calls succeed (`200 OK` in < 1 s). A subsequent `tonic` gRPC handshake never completes. `tokio::time::timeout(25s)` cannot preempt it (the blocking FFI call doesn't yield), so the task hangs indefinitely.
- **Fixed**: `reqwest 0.13` with `default-features = false, features = ["json", "rustls-no-provider"]`, plus `rustls::crypto::ring::default_provider().install_default()` called before any TLS use. Observation: both `reqwest` and `tonic` complete in < 500 ms.

Both builds ship the same `aws-lc-rs`, `rustls-platform-verifier`, and `ring` crates in the resolved `Cargo.lock`. The only difference is which `rustls::crypto::CryptoProvider` is registered as the process default. That strongly points at `aws-lc-rs` (BoringSSL fork) deadlocking when loaded into the same address space as CEF's bundled Chromium BoringSSL.

## Minimal repro

Standalone project (no React, no wallet code), 2 files of Rust + 1 of HTML. See attached tarball `tauri-cef-tls-repro.tar.gz`.

Build:

```
cd src-tauri
cargo tauri build --features cef-runtime --bundles app          # broken
cargo tauri build --features cef-runtime,install-ring --bundles app   # fixed
```

Each build launches, auto-runs two probes on startup, and appends to `/tmp/repro-probe.log`:

**Broken (no explicit provider install):**
```
[repro] startup variant=broken (no explicit provider install)
[repro] probe reqwest start
[repro] probe reqwest => reqwest ok status=200 elapsed_ms=444
[repro] probe tonic start
                                            <- hangs here, no further output after 90 s
```

**Fixed (ring installed as default before any TLS):**
```
[repro] startup variant=fixed (ring installed as default)
[repro] probe reqwest start
[repro] probe reqwest => reqwest ok status=200 elapsed_ms=326
[repro] probe tonic start
[repro] probe tonic => tonic ok elapsed_ms=352
[repro] probes done
```

## Workaround for downstream

If you use `reqwest 0.13 rustls` under Tauri CEF today:

```toml
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-no-provider"] }
rustls = { version = "0.23", default-features = false, features = ["ring"] }
```

And at startup, before any TLS:

```rust
let _ = rustls::crypto::ring::default_provider().install_default();
```

Filing a separate issue on `aws/aws-lc-rs` with the stack angle. Linking back here as the anchor. Happy to open a PR against the CEF feature guide doc if useful, to warn downstream about this trap.
```

## 2. Root-cause issue: new issue on `aws/aws-lc-rs`

Target: https://github.com/aws/aws-lc-rs/issues/new

### Title

```
aws-lc-rs deadlocks when loaded alongside CEF's bundled Chromium BoringSSL (macOS arm64, in-process)
```

### Body

```markdown
## Summary

Loading `aws-lc-rs` into a macOS process that also statically links CEF's Chromium-bundled BoringSSL causes the first `rustls` handshake that uses `aws-lc-rs` as its `CryptoProvider` to hang indefinitely. The hang blocks a tokio worker in a way that `tokio::time::timeout` cannot preempt, suggesting a lock or spin inside a synchronous FFI call.

Swapping the default `CryptoProvider` to `ring` resolves the hang entirely with no other change.

## Environment

- macOS 25.4 (Darwin 25.4.0), arm64
- Rust 1.92, `cargo` release build
- `aws-lc-rs` 1.16.3 (transitive via `reqwest 0.13` default `rustls` feature → `rustls-platform-verifier` 0.6.2)
- `rustls` 0.23, `tonic` 0.14.2
- Host: Tauri v2 with experimental CEF runtime (CEF 146.4.1+146.0.9). CEF statically links Chromium's bundled BoringSSL into the main browser process.

## Repro

Minimal standalone Tauri + CEF project, < 200 LOC Rust, attached as tarball (or mirrored at TBD if wanted):

- `reqwest 0.13` default `rustls` feature (pulls `aws-lc-rs`)
- `tonic 0.14` with `tls-native-roots` (uses whatever `CryptoProvider` is default)
- two feature variants: default (broken) vs `install-ring` (fixed)

Build + run under `make tauri-build`'s CEF bundling path (`cef-runtime` feature). Auto-probe on startup performs one reqwest GET and one tonic connect, logs to `/tmp/repro-probe.log`.

**Broken (default rustls → `aws-lc-rs` as process CryptoProvider default):**
- reqwest to `https://www.google.com` → `200 OK` in ~400 ms
- tonic to `https://zec.rocks:443` → stuck; no completion in 90 s; `tokio::time::timeout(25s)` does not fire

**Fixed (`rustls::crypto::ring::default_provider().install_default()` called before TLS):**
- reqwest: ~300 ms
- tonic: ~350 ms

Both builds resolve `aws-lc-rs 1.16.3`, `aws-lc-sys 0.40.0`, `ring 0.17.14`, `rustls-platform-verifier 0.6.2`. Only the `CryptoProvider` default differs.

## Why aws-lc-rs is implicated

- The hang persists across which reqwest + tonic call orders we try, as long as `aws-lc-rs` is the default provider.
- Replacing the default provider with `ring` (also BoringSSL-adjacent but a pure-Rust subset, no FFI into libcrypto) fully fixes it.
- `rustls-platform-verifier`, `webpki-roots`, and the rest of the graph remain identical across the two variants.
- CEF's `Chromium Embedded Framework.framework` ships its own BoringSSL. Same process, same address space, overlapping global symbol names (`SSL_*`, `EVP_*`, `CRYPTO_*`).

On-CPU sampling during the hang shows all tokio workers parked on `pthread_cond_wait` / `kevent`, with no active TLS stack. Rust frames are stripped in the release build but this pattern matches a worker waiting on a completion that never arrives because a peer thread holds a lock inside the native handshake path.

## Hypothesis

The symbol collision between `aws-lc-rs`'s statically-linked `libcrypto.a` (BoringSSL fork) and CEF's bundled BoringSSL produces nondeterministic resolution at dlopen/relocation time. Whichever single copy of a given `SSL_*` symbol ends up first in the flat namespace is the one both sides call into. When `aws-lc-rs`'s handshake ends up calling into CEF's copy (or vice versa), internal mutex state is split between two callers that don't agree on the invariants, and the handshake deadlocks.

Alternative: global locks like `CRYPTO_THREADID_*` or `CRYPTO_secure_used` end up duplicated and stranded.

I don't have a clean way to prove this from userland without symbol-unstripped binaries; happy to do the sampling with debug info if that would help.

## What I'd like

- Confirmation whether this matches known issues with `aws-lc-rs` in-process with other BoringSSL consumers (CEF, Electron, WebKit plugins).
- Guidance on whether `-fvisibility=hidden` + explicit namespacing is on the `aws-lc-rs` roadmap, or whether this should be treated as "not supported, use `ring`" in embedded-Chromium hosts.
- If applicable, a note in the README / docs warning downstream about this scenario.

## Related

- `tauri-apps/tauri#13878` (anchor, where this was first observed in the wild)
- `seanmonstar/reqwest#2924` (documentation gap on `rustls-no-provider` in 0.13+)
- `seanmonstar/reqwest#3009` (aws-lc-sys linker collision on Linux, different layer but same family)

## Workaround

For downstream projects combining Tauri CEF + reqwest 0.13 + rustls:

```toml
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-no-provider"] }
rustls = { version = "0.23", default-features = false, features = ["ring"] }
```

```rust
let _ = rustls::crypto::ring::default_provider().install_default();
```

at process start before any TLS use.
```

## Posting checklist (do not run without user approval)

1. Verify `/Users/bioharz/git/zcash/bagz/bagz-cef-git/plans/tauri-cef-tls-repro.tar.gz` exists and extracts.
2. Confirm both probe outputs quoted above match current logs.
3. Post the anchor comment first: `gh issue comment 13878 --repo tauri-apps/tauri --body-file plans/phase-c-anchor.md` (body extracted from section 1 above).
4. Attach tarball via GitHub web UI (CLI doesn't support file attachments on comments).
5. Wait for anchor comment to show as posted, link it from the next step.
6. Post the root-cause issue: `gh issue create --repo aws/aws-lc-rs --title "..." --body-file plans/phase-c-awslc.md`.
7. Update `plans/handoff-cef-tls-hang.md` with final URLs.

## Do-not post reminders

- No filings on `seanmonstar/reqwest` (plan rules it out; root cause is upstream of reqwest).
- No touching `seanmonstar/reqwest#2423` default-provider debate.
- No PR opened preemptively. Gated on maintainer reply.
