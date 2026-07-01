# zbag Desktop

A desktop-first privacy-by-design Zcash wallet with hardware wallet support and integrated DEX functionality.

## Release Track

This repository contains the current **Tauri alpha**. It is published for source
access, review, and self-builds, but this alpha track is not the actively
maintained product track and **no prebuilt app binaries are provided here**.
Build from source with `just install` and `just app-build`.

The next maintained beta is the Flutter redesign. That beta is planned to ship
prebuilt releases, including properly signed macOS builds and best-effort
Windows builds/signing when ready.

## Overview

zbag Desktop provides a privacy-focused Zcash experience built on strong security principles:

- **Shielded spending** - Transparent funds must be shielded before use; sending to transparent recipients is allowed only with explicit acknowledgement
- **Secrets stay in Rust** - Spending keys never reach the UI layer; seed phrases are only displayed/entered in explicitly permitted flows (create, backup verify, restore, view seed) and are never persisted or logged by the UI
- **Fail-closed Tor** - Network anonymization that blocks rather than silently downgrades
- **Air-gapped signing** - Keystone hardware wallet support via QR codes

## Screenshots

Tauri alpha screenshots are hosted as GitHub release assets rather than stored
in the source repository history. Wallet addresses, QR codes, transaction
identifiers, and balances were redacted before upload.

| Wallet selection | Unlock |
| --- | --- |
| <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-wallet-select.webp" width="420" alt="Wallet selection"> | <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-unlock.webp" width="420" alt="Unlock wallet"> |

| Home | Send |
| --- | --- |
| <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-home.webp" width="420" alt="Home dashboard"> | <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-send.webp" width="420" alt="Send flow"> |

| Receive | Swap to ZEC |
| --- | --- |
| <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-receive.webp" width="420" alt="Receive flow"> | <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-swap-to-zec.webp" width="420" alt="Swap to ZEC"> |

| Swap from ZEC | Activity |
| --- | --- |
| <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-swap-from-zec.webp" width="420" alt="Swap from ZEC"> | <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-activity.webp" width="420" alt="Activity"> |

| Servers | Settings |
| --- | --- |
| <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-servers.webp" width="420" alt="Server settings"> | <img src="https://github.com/zbag-app/zbag-app-tauri/releases/download/docs-screenshots-2026-07-01/alpha-settings.webp" width="420" alt="Settings"> |

## Architecture

| Layer | Technology | Purpose |
|-------|------------|---------|
| Backend | Rust (librustzcash, Tauri) | Wallet engine, key management, networking |
| Frontend | React + TypeScript | User interface via WebView |
| IPC | Typed commands/events | Strict contract between layers |
| Storage | SQLite (dual database) | Wallet state + app metadata |

```
crates/
  zbag-core/       # Domain types and IPC contracts
  zbag-engine/     # Wallet operations (librustzcash wrapper)
  zbag-network/    # gRPC/HTTP clients, Tor transport
  zbag-keystone/   # Hardware wallet integration
  zbag-tor/        # Embedded Arti Tor client

apps/
  zbag-app-tauri/  # Tauri shell + React frontend
  zbag-cli/        # Command-line interface
```

## Key Features

| Feature | Description |
|---------|-------------|
| Wallet Management | Create, restore from seed phrase with birthday optimization |
| Shielded Transactions | Send/receive with optional encrypted memos |
| Address Rotation | Fresh shielded address on each receive request; single transparent compatibility address |
| Hardware Signing | Keystone via PCZT (QR or microSD) |
| DEX Integration | Swap to/from ZEC via NEAR Intents (mainnet only) |
| Tor Anonymization | Optional network privacy with fail-closed behavior |

## Status

**Phase**: Tauri alpha, source-build only

Platform targets: macOS, Windows, Linux

## Documentation

| Document | Description |
|----------|-------------|
| [Constitution](.specify/memory/constitution.md) | Non-negotiable principles governing development |
| [Feature Specification](specs/001-zbag-desktop-wallet/spec.md) | User stories and requirements |
| [Implementation Plan](specs/001-zbag-desktop-wallet/plan.md) | Architecture and project structure |
| [Data Model](specs/001-zbag-desktop-wallet/data-model.md) | Entities, relationships, and database schema |
| [Research](specs/001-zbag-desktop-wallet/research.md) | Technology decisions and rationale |
| [Quickstart](specs/001-zbag-desktop-wallet/quickstart.md) | Developer setup guide |
| [E2E Testing](docs/E2E_TESTING.md) | Test bridge architecture and Playwright setup |

## Core Dependencies

- [zcash_client_backend](https://docs.rs/zcash_client_backend) - Zcash light client with PCZT and Tor support
- [zcash_client_sqlite](https://docs.rs/zcash_client_sqlite) - Wallet database implementation
- [Tauri v2](https://v2.tauri.app) - Desktop application framework
- ~~[Keystone SDK](https://dev.keyst.one) - Hardware wallet integration~~
- [Keystone QR tooling (`@keystonehq/animated-qr`)](https://github.com/KeystoneHQ/keystone-airgaped-base) - Multi-frame QR for PCZT signing; Zcash `zcash-pczt` UR payload is encoded/decoded in-app to stay browser-compatible
- [NEAR Intents](https://docs.near-intents.org) - DEX functionality

## Development

A `justfile` is provided as the preferred command surface. The Makefile remains
available underneath for lower-level legacy targets.

| Recipe | Description |
|--------|-------------|
| `just install` | Install frontend dependencies |
| `just app-dev` | Run Tauri development server |
| `just build` | Build Rust library crates |
| `just test` | Run Rust tests |
| `just pre-commit` | Format and lint before committing |
| `just app-build` | Build the Tauri app bundle |
| `just verify` | Full local validation including app bundle build |

Run `just --list` for all available recipes, or `make help` for the full legacy target list.

See [Quickstart](specs/001-zbag-desktop-wallet/quickstart.md) for full setup instructions.

### Temporary Send Debugging

For deeper send/broadcast diagnostics (including terminal output), set:

```bash
export ZBAG_TEMP_DEBUG=1
```

This is disabled by default. When enabled, zbag adds extra timed send/broadcast debug events and mirrors logs to stderr.

## E2E Testing (Test Bridge)

The test bridge exposes Tauri IPC commands over HTTP for Playwright. It is **feature-gated**
and bound to localhost only; do not enable it in release builds.

> **WARNING:** Never use production wallets or real seed phrases with the test bridge. Test data is ephemeral and all wallet operations are exposed over HTTP. Always use dedicated test seed phrases.

Quickstart:

```bash
# Terminal 1: start the Rust test bridge with an isolated data directory
export ZBAG_TEST_HOME="$(mktemp -d)"
cargo run -p zbag-app-tauri --features test-bridge

# Terminal 2: start the Vite dev server with the test bridge transport
cd apps/zbag-app-tauri
VITE_TEST_BRIDGE=true bun run dev

# Terminal 3: install Playwright (first time) and run E2E tests
cd apps/zbag-app-tauri
bunx playwright install chromium
bun run test:e2e
```

To reset test data between runs, remove the directory in `ZBAG_TEST_HOME`. In test-bridge mode, `ZBAG_TEST_HOME` must be set to a non-empty path; empty or whitespace-only values are rejected.

See [docs/E2E_TESTING.md](docs/E2E_TESTING.md) for architecture details, CI workflow, and troubleshooting.

## Requirements

- Platform-specific dependencies (see [Quickstart](specs/001-zbag-desktop-wallet/quickstart.md))

## License

zbag is source-available under FSL-1.1-ALv2. See [`LICENSE`](./LICENSE) and
[`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md) for the current license,
future Apache-2.0 conversion terms, dependency notices, and historical
provenance.

## Security

This wallet enforces strict security boundaries. Key principles:

- Seed phrases are generated in the Rust backend and only displayed/entered in explicitly permitted flows (create, backup verify, restore, view seed); never persisted or logged by the UI
- "View seed phrase" requires manual wallet-password re-authentication
- The UI operates on derived, non-sensitive data, except for transient mnemonic display/entry during those permitted flows
- Spending transparent funds (using transparent inputs) is architecturally impossible; transparent funds must be shielded before they can fund payments
- All network requests route through backend-controlled transports

For vulnerability reports, please use private disclosure. See the [Constitution](.specify/memory/constitution.md#security-reporting-and-incident-response) for the incident response policy.
