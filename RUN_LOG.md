# Overnight Run Log: bagz license, rebrand, dep refresh

Started: 2026-05-14T22:50:43Z
Branch: overnight/bagz-rebrand-refresh-20260515
Plan file: /Users/bioharz/git/zcash/bagz/plans/polished-cooking-flute.md

## Tiers

- 0-setup: created branch, pushed draft PR #2.
- 0a-baseline-clippy: baseline strict clippy failed before plan edits; fixed existing clippy-only issues in `zstash-engine` and restored the light Rust gate. Last green: `ad95c13e3734ea837b045704577e53874b248180`.
- 1-license-polyform-shield: replaced MIT with official PolyForm Shield License 1.0.0 text from `https://polyformproject.org/licenses/shield/1.0.0.txt`, using `Copyright (c) 2026 devdotbo (Reza Shokri)`. Previous LICENSE header was `Copyright (c) 2025 devdotbo (dev.bo)`. `cargo metadata --no-deps --format-version 1` and `cargo check --workspace --exclude zstash-app-tauri` passed. Last green: `cc1f1306df3dd3ca2660d0fd23b95e7c6530cf70`.
- 2-ring-provider-fix: cherry-picked `7469fd4` as `ae0c7cecf4ec3a00307cbfc501c789328217eda5`. `make fmt-check`, strict clippy excluding the Tauri app, and `cargo test -p zstash-network` passed.
- 3-ring-provider-regression-test: added `ring_provider_installs_and_is_default`. `cargo fmt --all --check` and `cargo test -p zstash-network ring_provider_installs_and_is_default` passed. Last green: `5d27f2cb1d82b09bf1f26642dcadaa0dd5ed6aa4`.
- 4a-tauri-cef-rev-bump: bumped `tauri` and `tauri-build` patch rev to upstream `tauri-apps/tauri@6fd733b2d6255d358e88ad19cb15dc7d22b293ac` (`feat/cef`, 2026-05-14). This moves Rust Tauri to `2.11.1`, `tauri-build` to `2.6.1`, and CEF from `146.4.1+146.0.9` to Tauri-pinned `148.0.0+147.0.10` (latest standalone `cef-rs` is `148.1.0+147.0.14`, but Tauri runtime pins `148.0.0`). Synced `@tauri-apps/api` to `2.11.0` and `@tauri-apps/cli` to `2.11.1` because the new Tauri CLI rejects mismatched JS/Rust minor versions. Fixed the test bridge build by enabling `tauri/wry` only for `test-bridge` and by passing `SensitiveString` by value in the restore bridge handler. `make check`, `make tauri-build`, CEF framework presence check, and `make test-e2e` passed. Bundle outputs: `target/release/bundle/macos/zSTASH.app`, `target/release/bundle/dmg/zSTASH_0.2.1_aarch64.dmg`. Last green: `ec3b3681b183cb24482babce74c09aa57fe0a245`.
