# CEF Password Manager Hardening (macOS)

## Scope
This document captures the CEF hardening applied to prevent browser-style credential prompts in zbag while keeping CEF startup stable on macOS. General CEF network hardening is covered in `docs/cef-network-hardening.md`.

## Implemented controls

### 1. Runtime CEF arguments
File: `apps/zbag-app-tauri/src-tauri/src/lib.rs`

- `--use-mock-keychain` is used by default on macOS (unless `ZBAG_USE_SYSTEM_KEYCHAIN=1`).
- `--disable-save-password-bubble` is enabled as part of the broader offline CEF switch set.

These are the stable runtime flags verified not to reintroduce `CrBrowserMain` startup crashes.

### 2. Profile-level password/autofill policy
File: `apps/zbag-app-tauri/src-tauri/src/lib.rs`

Before launching Tauri/CEF, the app writes CEF profile preferences at:

- `<per-launch temp CEF cache>/Default/Preferences`

The following values are enforced:

- `credentials_enable_service = false`
- `profile.password_manager_enabled = false`
- `profile.password_manager_leak_detection = false`
- `autofill.enabled = false`
- `autofill.profile_enabled = false`
- `autofill.credit_card_enabled = false`

This disables Chromium password manager behavior at the profile preference level.

### 3. Frontend field hardening
Files:

- `apps/zbag-app-tauri/src/App.tsx`
- `apps/zbag-app-tauri/src/components/ui/input.tsx`

Added client-side hardening for all forms/inputs:

- force `autocomplete="off"` on forms and non-password text fields
- force `autocomplete="new-password"` on password fields
- set `data-lpignore="true"` and `data-1p-ignore="true"`
- disable `autocorrect`, `autocapitalize`, and spellcheck

## Validation performed

- `bun run build` (frontend)
- `cargo check --manifest-path apps/zbag-app-tauri/src-tauri/Cargo.toml --features cef-runtime`
- CEF `.app` build via pinned Tauri CLI command
- Launch verification of built app process and helper processes
- User confirmation that password prompt issue is fixed

## Notes

- Earlier aggressive `--disable-features=...` bundles caused startup instability in this environment. The current CEF network-hardening set is guarded by static checks, parsed-argument tests, and a packaged-app smoke test.
- Policy-level preference enforcement plus frontend hardening remains the password-manager-specific fix path.
