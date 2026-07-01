# CEF Reproducibility and Gate Results

## Objective
Make the CEF build path reproducible in both local and CI runs, and verify full required gates pass end-to-end on that same path.

## Build path hardening

### New script
- `scripts/tauri-cef-build.sh`

Behavior:
- resolves the pinned Tauri git revision from `Cargo.lock`
- locates matching `tauri-cli` manifest in Cargo git checkouts
- runs `cargo-tauri build` from that pinned manifest
- applies feature/bundle settings from environment variables

Defaults:
- `TAURI_FEATURES=cef-runtime`
- macOS bundles default to `app,dmg`

### Makefile integration
- `Makefile` `tauri-build` now uses:
  - `./scripts/tauri-cef-build.sh`
- this replaces the previous `bun run tauri build` path for production bundling

### CI integration
- CI already invokes `make tauri-build` in `.github/workflows/ci.yml`
- no workflow command change needed after Makefile switch
- result: CI and local use the same pinned build path

## Gate run execution
Executed on February 18, 2026 (local machine):

1. `make build` ✅
2. `make test` ✅
3. `make pre-commit` ✅
4. `make tauri-build` ✅

`make tauri-build` outputs confirmed:
- `.app`: `target/release/bundle/macos/zbag.app`
- `.dmg`: `target/release/bundle/dmg/zbag_0.2.2_aarch64.dmg`

Size snapshot:
- app bundle: `372M`
- dmg: `186M`

## Notes
- This path avoids ad-hoc manual `cargo-tauri` manifest commands and keeps CEF bundling deterministic.
- DMG generation now runs in the same reproducible path and completed successfully.
