# CEF Guide (bagZ)

Last updated: February 18, 2026

## Purpose

This document is the single entry point for CEF usage in bagZ: build path, hardening policy, validation gates, and common troubleshooting.

## Current baseline

- Runtime: Tauri v2 with `cef-runtime` feature
- Reproducible build entrypoint: `make tauri-build`
- Packaging target on macOS: `.app` and `.dmg`
- Known-good output paths:
  - `target/release/bundle/macos/bagZ.app`
  - `target/release/bundle/dmg/bagZ_0.2.2_aarch64.dmg`

## Build and run

### Production CEF build (recommended)

```bash
make tauri-build
```

This now uses `scripts/tauri-cef-build.sh`, which resolves the pinned `cargo-tauri` from `Cargo.lock` and avoids non-reproducible local CLI drift.

### Launch built app

```bash
open -na target/release/bundle/macos/bagZ.app
```

## Slim CEF profiles (no CEF rebuild)

The default build remains unchanged:

```bash
make tauri-build
```

Optional slim builds stage a local CEF copy, prune optional assets, then build with `CEF_PATH` pointed at the staged copy.

### Size audit

```bash
make cef-audit
```

### SAFE profile (recommended)

```bash
make tauri-build-slim-safe
```

Behavior:
- keeps CEF runtime core files
- keeps `resources.pak`, `chrome_100_percent.pak`, `chrome_200_percent.pak`
- keeps minimal English locales (`en.lproj`, `en_GB.lproj` by default)
- keeps ANGLE/SwiftShader libraries

### AGGRESSIVE profile (opt-in)

```bash
make tauri-build-slim-aggressive
```

Behavior:
- starts from SAFE profile pruning
- additionally removes ANGLE/SwiftShader payload where present:
  - `libEGL*`
  - `libGLESv2*`
  - `libvk_swiftshader*`
  - `vk_swiftshader_icd*`
  - related shader cache payload

### Rollback / revert

- Use the default build path at any time: `make tauri-build`.
- Remove staged slim artifacts: `rm -rf target/cef-stage`.
- No codec flags are changed and no CEF source build is required.

## Required quality gates

The release readiness path is:

1. `make build`
2. `make test`
3. `make pre-commit`
4. `make tauri-build`
5. `make cef-smoketest`

`make cef-smoketest` requires the packaged macOS bundle produced by `make tauri-build`. CI runs it inside the existing `tauri-build-cef` job after the bundle is built.

## CEF hardening policy

Implemented behavior:

- Run CEF with a per-launch temp cache, not a durable Chromium profile
- Run CEF in incognito mode
- Remove the legacy persistent CEF cache and stale temp CEF caches on startup; remove the current temp cache after normal exit
- Disable Chromium background networking, component updater, domain reliability, sync, field trials, metrics upload paths, and first-run/default-browser flows
- Disable Chromium DNS-over-HTTPS and map all CEF hostname resolution to `0.0.0.0` except localhost-style hosts used by Tauri/dev IPC
- Disable browser services in CEF profile preferences: Safe Browsing, search suggestions, network prediction, spell service, translation, sign-in, Privacy Sandbox, and WebRTC non-proxied UDP
- Disable Chromium password-save UX
- Disable default browser-like context menu behavior in app UI
- Keep drag/movable-element behavior locked down in UI
- Use mock keychain by default on macOS unless explicitly overridden

Key implementation files:

- `apps/bagz-app-tauri/src-tauri/src/lib.rs`
- `apps/bagz-app-tauri/src/App.tsx`
- `apps/bagz-app-tauri/src/components/ui/input.tsx`
- `scripts/check-cef-network-hardening.sh`
- `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs`
- `scripts/cef-network-smoketest.sh`

## Upgrading CEF

Current pin (branch `cef`):

- Tauri `feat/cef` rev: `6fd733b2d6255d358e88ad19cb15dc7d22b293ac`
- `cef` + `cef-dll-sys`: Tauri-pinned `148.0.0+147.0.10`
- `tauri` crate: `2.11.1`
- `@tauri-apps/api`: `2.11.0`
- `@tauri-apps/cli`: `2.11.1`

The CEF version is controlled by the pinned Tauri rev (`tauri-runtime-cef/Cargo.toml` contains `cef = "=<version>"`). Do not bump `cef` directly.

### Upgrade steps

1. Resolve the new Tauri `feat/cef` HEAD:
   ```bash
   git ls-remote https://github.com/tauri-apps/tauri refs/heads/feat/cef
   ```
2. Inspect the candidate rev for the CEF version and the JS package versions:
   ```bash
   curl -sL "https://raw.githubusercontent.com/tauri-apps/tauri/<rev>/crates/tauri-runtime-cef/Cargo.toml" | grep '^cef '
   curl -sL "https://raw.githubusercontent.com/tauri-apps/tauri/<rev>/packages/cli/package.json" | head -5
   curl -sL "https://raw.githubusercontent.com/tauri-apps/tauri/<rev>/packages/api/package.json" | head -5
   ```
3. Bump `Cargo.toml` `[patch.crates-io]` `tauri` + `tauri-build` to the new rev.
4. Bump `apps/bagz-app-tauri/package.json` `@tauri-apps/cli` + `@tauri-apps/api`.
5. Refresh locks:
   ```bash
   cargo update -p tauri -p tauri-build
   cd apps/bagz-app-tauri && bun install
   ```
6. Rebuild and verify the framework actually landed in the bundle:
   ```bash
   make tauri-build
   ls "target/release/bundle/macos/bagZ.app/Contents/Frameworks/"
   ```
   Expect `Chromium Embedded Framework.framework` plus five `bagz-app-tauri Helper*.app` bundles. If `Frameworks/` is missing, see **Troubleshooting: missing CEF framework in bundle** below.
7. Launch via `open -na target/release/bundle/macos/bagZ.app` and confirm renderer + GPU helper processes spawn.

### Known breakages when bumping Tauri

- **Both `wry` and `cef` features enabled → `error[E0252]: webview_version` defined multiple times.** `tauri/src/lib.rs` re-exports `webview_version` from both runtimes. Fix: in `apps/bagz-app-tauri/src-tauri/Cargo.toml` keep `tauri = { version = "2", default-features = false, features = ["compression", "common-controls-v6", "dynamic-acl"] }`. Do not add `wry` back.
- **`AppHandle` has no default `Runtime` when `wry` is off.** `#[default_runtime(crate::Wry, wry)]` only supplies the default when the `wry` feature is enabled. Every function that holds a Tauri handle needs an explicit runtime generic, e.g. `pub fn f<R: Runtime>(app: &AppHandle<R>)`. Same rule applies to `WebviewWindow<R>`, `Window<R>`, `Manager<R>`. Currently only `apps/bagz-app-tauri/src-tauri/src/windows.rs` is affected, but any new handle-taking function will have to follow suit.

## Common troubleshooting

### `Failed to request http://localhost:1420/`

Cause: launching a build that expects dev server mode while no Vite dev server is running.

Fix:

- For production usage, run `make tauri-build` and open the built `.app`.
- For dev usage, run `make dev` and keep the dev server active.

### Missing CEF framework in bundle (app panics on launch)

Symptom: `make tauri-build` exits 0, `.app` and `.dmg` are produced, but launching panics with:

```
thread 'main' panicked at cef-<ver>/src/library_loader.rs:
called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, ... }
```

and `.app/Contents/Frameworks/` is absent.

Cause: `tauri-cli`'s `ensure_cef_directory` (`crates/tauri-cli/src/cef/exporter.rs`) tries to download/verify the pinned CEF version before the bundler runs. If the CDN lookup fails (logged as `CEF Failed to ensure CEF directory: version <ver> not found`), the exporter returns `Err`, `CEF_PATH` never gets set, and `tauri-bundler` silently skips `copy_cef_framework` + `create_cef_helpers`. The linker-side CEF (via `cef-dll-sys` build script) still succeeds, so compilation reports success.

Fix (when exporter fails but `cef-dll-sys` already produced the framework):

```bash
ROOT=$(pwd)
CEF_OUT=$(find target/release/build -type d -name cef_macos_aarch64 | head -1)
mkdir -p target/cef-cache
ln -sfn "$ROOT/$CEF_OUT" "target/cef-cache/<version>"  # e.g. 146.4.1
CEF_PATH="$ROOT/target/cef-cache" make tauri-build
```

The exporter reads `CEF_PATH/<version>/archive.json`, sees the version matches, skips the download, and forwards `CEF_PATH` to the bundler. The staged dir does not have to be real; a symlink to the `cef-dll-sys` build output works.

Verify after rebuild:

```bash
ls target/release/bundle/macos/bagZ.app/Contents/Frameworks/
# Chromium Embedded Framework.framework
# bagz-app-tauri Helper.app
# bagz-app-tauri Helper (GPU).app
# bagz-app-tauri Helper (Renderer).app
# bagz-app-tauri Helper (Plugin).app
# bagz-app-tauri Helper (Alerts).app
```

A passing `make tauri-build` is not proof the framework shipped. Always check `Frameworks/` before claiming a build succeeded.

### Chromium Safe Storage / keychain password prompt

Default behavior is configured to avoid per-launch keychain prompts using mock keychain mode.

If you intentionally want system keychain integration, launch with:

```bash
BAGZ_USE_SYSTEM_KEYCHAIN=1
```

### Chromium “Save password?” popup

The fix is policy + frontend hardening:

- CEF preferences enforce password manager/autofill disabled
- Form/input attributes explicitly mark credentials as non-storable

If this regresses, check:

- `~/Library/Caches/app.bagz.desktop/cef/Default/Preferences`
- recent changes in the files listed in **CEF hardening policy**

## Related docs

- `docs/cef-network-hardening.md`
- `docs/cef-password-hardening.md`
- `docs/cef-reproducibility-and-gates.md`
