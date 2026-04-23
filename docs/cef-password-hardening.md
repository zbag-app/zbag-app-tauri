# CEF Password Manager Hardening (macOS)

## Scope
This document captures the CEF hardening applied to prevent browser-style credential prompts in zSTASH while keeping CEF startup stable on macOS.

## Implemented controls

### 1. Runtime CEF arguments
File: `apps/zstash-app-tauri/src-tauri/src/lib.rs`

- `--use-mock-keychain` is used by default on macOS (unless `ZSTASH_USE_SYSTEM_KEYCHAIN=1`).
- `--disable-save-password-bubble` is enabled.

These are the stable runtime flags verified not to reintroduce `CrBrowserMain` startup crashes.

### 2. Profile-level password/autofill policy
File: `apps/zstash-app-tauri/src-tauri/src/lib.rs`

Before launching Tauri/CEF, the app writes CEF profile preferences at:

- `~/Library/Caches/<bundle_identifier>/cef/Default/Preferences`

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

- `apps/zstash-app-tauri/src/App.tsx`
- `apps/zstash-app-tauri/src/components/ui/input.tsx`

Added client-side hardening for all forms/inputs:

- force `autocomplete="off"` on forms and non-password text fields
- force `autocomplete="new-password"` on password fields
- set `data-lpignore="true"` and `data-1p-ignore="true"`
- disable `autocorrect`, `autocapitalize`, and spellcheck

## Validation performed

- `bun run build` (frontend)
- `cargo check --manifest-path apps/zstash-app-tauri/src-tauri/Cargo.toml --features cef-runtime`
- CEF `.app` build via pinned Tauri CLI command
- Launch verification of built app process and helper processes
- User confirmation that password prompt issue is fixed

## Notes

- Aggressive `--disable-features=...` bundles were intentionally avoided because they caused startup instability in this environment.
- Policy-level preference enforcement plus frontend hardening provided a stable fix path.
