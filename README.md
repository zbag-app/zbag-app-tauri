# zSTASH Desktop

A desktop-first privacy-by-design Zcash wallet with hardware wallet support and integrated DEX functionality.

## Overview

zSTASH Desktop provides a privacy-focused Zcash experience built on strong security principles:

- **Shielded spending** - Transparent funds must be shielded before use; sending to transparent recipients is allowed only with explicit acknowledgement
- **Secrets stay in Rust** - Spending keys never reach the UI layer; seed phrases are only displayed/entered in explicitly permitted flows (create, backup verify, restore, view seed) and are never persisted or logged by the UI
- **Fail-closed Tor** - Network anonymization that blocks rather than silently downgrades
- **Air-gapped signing** - Keystone hardware wallet support via QR codes

## Architecture

| Layer | Technology | Purpose |
|-------|------------|---------|
| Backend | Rust (librustzcash, Tauri) | Wallet engine, key management, networking |
| Frontend | React + TypeScript | User interface via WebView |
| IPC | Typed commands/events | Strict contract between layers |
| Storage | SQLite (dual database) | Wallet state + app metadata |

```
crates/
  zstash-core/       # Domain types and IPC contracts
  zstash-engine/     # Wallet operations (librustzcash wrapper)
  zstash-network/    # gRPC/HTTP clients, Tor transport
  zstash-keystone/   # Hardware wallet integration
  zstash-tor/        # Embedded Arti Tor client

apps/
  zstash-app-tauri/  # Tauri shell + React frontend
  zstash-cli/        # Command-line interface
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

**Phase**: Active development (Tauri desktop app and CLI implementations)

Platform targets: macOS, Windows, Linux

## Documentation

| Document | Description |
|----------|-------------|
| [Constitution](.specify/memory/constitution.md) | Non-negotiable principles governing development |
| [Feature Specification](specs/001-zstash-desktop-wallet/spec.md) | User stories and requirements |
| [Implementation Plan](specs/001-zstash-desktop-wallet/plan.md) | Architecture and project structure |
| [Data Model](specs/001-zstash-desktop-wallet/data-model.md) | Entities, relationships, and database schema |
| [Research](specs/001-zstash-desktop-wallet/research.md) | Technology decisions and rationale |
| [Quickstart](specs/001-zstash-desktop-wallet/quickstart.md) | Developer setup guide |
| [E2E Testing](docs/E2E_TESTING.md) | Test bridge architecture and Playwright setup |

## Core Dependencies

- [zcash_client_backend](https://docs.rs/zcash_client_backend) - Zcash light client with PCZT and Tor support
- [zcash_client_sqlite](https://docs.rs/zcash_client_sqlite) - Wallet database implementation
- [Tauri v2](https://v2.tauri.app) - Desktop application framework
- ~~[Keystone SDK](https://dev.keyst.one) - Hardware wallet integration~~
- [Keystone QR tooling (`@keystonehq/animated-qr`)](https://github.com/KeystoneHQ/keystone-airgaped-base) - Multi-frame QR for PCZT signing; Zcash `zcash-pczt` UR payload is encoded/decoded in-app to stay browser-compatible
- [NEAR Intents](https://docs.near-intents.org) - DEX functionality

## Development

A Makefile is provided for common development tasks:

| Target | Description |
|--------|-------------|
| `make install` | Install frontend dependencies |
| `make dev` | Run Tauri development server |
| `make build` | Build Rust library crates |
| `make test` | Run all tests |
| `make pre-commit` | Format and lint before committing |
| `make check` | Full CI-like validation |

Run `make help` for all available targets.

See [Quickstart](specs/001-zstash-desktop-wallet/quickstart.md) for full setup instructions.

## E2E Testing (Test Bridge)

The test bridge exposes Tauri IPC commands over HTTP for Playwright. It is **feature-gated**
and bound to localhost only; do not enable it in release builds.

> **WARNING:** Never use production wallets or real seed phrases with the test bridge. Test data is ephemeral and all wallet operations are exposed over HTTP. Always use dedicated test seed phrases.

Quickstart:

```bash
# Terminal 1: start the Rust test bridge with an isolated data directory
export ZSTASH_TEST_HOME="$(mktemp -d)"
cargo run -p zstash-app-tauri --features test-bridge

# Terminal 2: start the Vite dev server with the test bridge transport
cd apps/zstash-app-tauri
VITE_TEST_BRIDGE=true bun run dev

# Terminal 3: install Playwright (first time) and run E2E tests
cd apps/zstash-app-tauri
bunx playwright install chromium
bun run test:e2e
```

To reset test data between runs, remove the directory in `ZSTASH_TEST_HOME`. In test-bridge mode, `ZSTASH_TEST_HOME` must be set to a non-empty path; empty or whitespace-only values are rejected.

See [docs/E2E_TESTING.md](docs/E2E_TESTING.md) for architecture details, CI workflow, and troubleshooting.

## Requirements

- Platform-specific dependencies (see [Quickstart](specs/001-zstash-desktop-wallet/quickstart.md))

## License

[MIT](LICENSE)

## Security

This wallet enforces strict security boundaries. Key principles:

- Seed phrases are generated in the Rust backend and only displayed/entered in explicitly permitted flows (create, backup verify, restore, view seed); never persisted or logged by the UI
- "View seed phrase" requires manual wallet-password re-authentication
- The UI operates on derived, non-sensitive data, except for transient mnemonic display/entry during those permitted flows
- Spending transparent funds (using transparent inputs) is architecturally impossible; transparent funds must be shielded before they can fund payments
- All network requests route through backend-controlled transports

For vulnerability reports, please use private disclosure. See the [Constitution](.specify/memory/constitution.md#security-reporting-and-incident-response) for the incident response policy.
