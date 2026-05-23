# CEF Network Hardening

## Incident

After moving the desktop shell to Tauri CEF, Chromium browser services appeared in macOS network tooling under the bagZ process tree. Observed domains included Google service/update/sign-in hosts, Google APIs, GVT/YouTube/domain-reliability hosts, Cloudflare DoH, and OpenDNS DoH.

This was introduced by the CEF runtime path. The wallet backend did not intentionally call these domains; they came from Chromium default browser behavior that was not disabled by the earlier password-manager-only CEF hardening.

## Policy

CEF is an offline renderer in bagZ. Wallet networking belongs to the Rust backend, where Tor and fail-closed behavior are enforced.

Runtime controls in `apps/bagz-app-tauri/src-tauri/src/lib.rs`:

- CEF uses a per-launch temp cache instead of a durable Chromium profile.
- CEF runs in incognito mode.
- The legacy persistent CEF cache and stale temp CEF caches are removed on startup; the current temp cache is removed after normal app exit.
- Chromium background networking, component updates, domain reliability, sync, field-trial config, first-run/default-browser flows, and pings are disabled.
- DNS-over-HTTPS is disabled.
- CEF host resolution maps every hostname to `0.0.0.0` except localhost-style Tauri/dev IPC hosts.
- Browser-service preferences disable Safe Browsing, search suggestions, spell service, translation, sign-in, network prediction, Privacy Sandbox, and WebRTC non-proxied UDP.

Optional escalation tiers are documented in `docs/cef-network-hardening-tiers.md`. Tier 2 and Tier 3 are not shipped unless the runtime smoke produces evidence that Tier 1 is insufficient.

Validation:

```bash
make check-cef-network-hardening
make check-cef-args
make cef-smoketest-selftest
make tauri-build
make cef-smoketest
```

The first three targets are included in `make pre-commit` and `make check`. `make cef-smoketest` requires a prebuilt packaged app at `target/release/bundle/macos/bagZ.app` and is run in CI immediately after the CEF Tauri build on macOS runners.

Guardrails:

- Layer 1: `scripts/check-cef-network-hardening.sh` verifies static anchors in `lib.rs` and rejects obvious forbidden switches or known Chromium service hostnames under `src-tauri`.
- Layer 2: `apps/bagz-app-tauri/src-tauri/tests/cef_runtime_args.rs` asserts on the parsed `Vec<(String, Option<String>)>` that Tauri receives.
- Layer 3: `cargo xtask cef-smoketest` launches the packaged app with isolated state, waits for the `.setup()` sentinel, and fails on any non-loopback TCP/UDP listener or peer observed in the app/helper process tree.
