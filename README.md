# Zkore Desktop

A desktop-first privacy-by-design Zcash wallet with hardware wallet support and integrated DEX functionality.

## Overview

Zkore Desktop provides a privacy-focused Zcash experience built on strong security principles:

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
  zkore-core/       # Domain types and IPC contracts
  zkore-engine/     # Wallet operations (librustzcash wrapper)
  zkore-network/    # gRPC/HTTP clients, Tor transport
  zkore-keystone/   # Hardware wallet integration
  zkore-tor/        # Embedded Arti Tor client

apps/
  zkore-app-tauri/  # Tauri shell + React frontend
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

**Phase**: Specification and design complete. Implementation pending.

Platform targets: macOS, Windows, Linux

## Documentation

| Document | Description |
|----------|-------------|
| [Constitution](.specify/memory/constitution.md) | Non-negotiable principles governing development |
| [Feature Specification](specs/001-zkore-desktop-wallet/spec.md) | User stories and requirements |
| [Implementation Plan](specs/001-zkore-desktop-wallet/plan.md) | Architecture and project structure |
| [Data Model](specs/001-zkore-desktop-wallet/data-model.md) | Entities, relationships, and database schema |
| [Research](specs/001-zkore-desktop-wallet/research.md) | Technology decisions and rationale |
| [Quickstart](specs/001-zkore-desktop-wallet/quickstart.md) | Developer setup guide |

## Core Dependencies

- [zcash_client_backend](https://docs.rs/zcash_client_backend) - Zcash light client with PCZT and Tor support
- [zcash_client_sqlite](https://docs.rs/zcash_client_sqlite) - Wallet database implementation
- [Tauri v2](https://v2.tauri.app) - Desktop application framework
- ~~[Keystone SDK](https://dev.keyst.one) - Hardware wallet integration~~
- [Keystone QR tooling (`@keystonehq/animated-qr`)](https://github.com/KeystoneHQ/keystone-airgaped-base) - Multi-frame QR for PCZT signing; Zcash `zcash-pczt` UR payload is encoded/decoded in-app to stay browser-compatible
- [NEAR Intents](https://docs.near-intents.org) - DEX functionality

## Requirements

- Platform-specific dependencies (see [Quickstart](specs/001-zkore-desktop-wallet/quickstart.md))

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
