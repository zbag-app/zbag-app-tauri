# CEF Guide (zSTASH)

Last updated: February 18, 2026

## Purpose

This document is the single entry point for CEF usage in zSTASH: build path, hardening policy, validation gates, and common troubleshooting.

## Current baseline

- Runtime: Tauri v2 with `cef-runtime` feature
- Reproducible build entrypoint: `make tauri-build`
- Packaging target on macOS: `.app` and `.dmg`
- Known-good output paths:
  - `target/release/bundle/macos/zSTASH.app`
  - `target/release/bundle/dmg/zSTASH_0.2.1_aarch64.dmg`

## Build and run

### Production CEF build (recommended)

```bash
make tauri-build
```

This now uses `scripts/tauri-cef-build.sh`, which resolves the pinned `cargo-tauri` from `Cargo.lock` and avoids non-reproducible local CLI drift.

### Launch built app

```bash
open -na target/release/bundle/macos/zSTASH.app
```

## Required quality gates

The release readiness path is:

1. `make build`
2. `make test`
3. `make pre-commit`
4. `make tauri-build`

All four must pass for CEF release confidence.

## CEF hardening policy

Implemented behavior:

- Disable Chromium password-save UX
- Disable default browser-like context menu behavior in app UI
- Keep drag/movable-element behavior locked down in UI
- Use mock keychain by default on macOS unless explicitly overridden

Key implementation files:

- `apps/zstash-app-tauri/src-tauri/src/lib.rs`
- `apps/zstash-app-tauri/src/App.tsx`
- `apps/zstash-app-tauri/src/components/ui/input.tsx`

## Common troubleshooting

### `Failed to request http://localhost:1420/`

Cause: launching a build that expects dev server mode while no Vite dev server is running.

Fix:

- For production usage, run `make tauri-build` and open the built `.app`.
- For dev usage, run `make dev` and keep the dev server active.

### Chromium Safe Storage / keychain password prompt

Default behavior is configured to avoid per-launch keychain prompts using mock keychain mode.

If you intentionally want system keychain integration, launch with:

```bash
ZSTASH_USE_SYSTEM_KEYCHAIN=1
```

### Chromium “Save password?” popup

The fix is policy + frontend hardening:

- CEF preferences enforce password manager/autofill disabled
- Form/input attributes explicitly mark credentials as non-storable

If this regresses, check:

- `~/Library/Caches/app.zstash.desktop/cef/Default/Preferences`
- recent changes in the files listed in **CEF hardening policy**

## Related docs

- `docs/cef-password-hardening.md`
- `docs/cef-reproducibility-and-gates.md`
