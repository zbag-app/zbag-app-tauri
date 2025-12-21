# Implementation Plan: Zkore Desktop Wallet

**Branch**: `001-zkore-desktop-wallet` | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-zkore-desktop-wallet/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Desktop-first shielded Zcash wallet with Orchard-only transactions, Keystone hardware wallet support via air-gapped PCZT signing, NEAR Intents DEX integration for swaps/pay, and optional Tor anonymization. Built on Tauri (Rust backend + React TypeScript frontend) with strict trust boundaries ensuring secrets never reach the UI layer.

## Technical Context

**Language/Version**: Rust 1.92.0+ with edition 2024 (backend), TypeScript 5.x (frontend)
**Primary Dependencies**:
- Backend: zcash_client_backend 0.21+ (pczt, tor features), zcash_client_sqlite 0.19+, zcash_primitives 0.26+, zcash_protocol 0.7+, Tauri v2, tonic 0.14+ (gRPC)
- Frontend: React 18+, @keystonehq/animated-qr, @keystonehq/keystone-sdk, bun 1.3.5+ (package manager)

> **Version Strategy**: We use caret (^) semver constraints aligned with librustzcash/Zashi. This allows security fixes while maintaining compatibility. Always commit Cargo.lock and build with `--locked` in production.
**Storage**: zcash_client_sqlite wallet DB + separate SQLite app metadata DB
  - Wallet directory structure with network separation:
    - `~/.zkore/wallets/mainnet/{wallet-id}/` (mainnet wallets)
    - `~/.zkore/wallets/testnet/{wallet-id}/` (testnet wallets)
  - Network selection at wallet creation (immutable after creation)
  - Separate database files per network
**Testing**: cargo test (Rust), vitest/jest (TypeScript), integration tests against Zaino/lightwalletd endpoints
**Target Platform**: macOS, Windows, Linux (desktop)
**Project Type**: Desktop application with Rust backend and web frontend (Tauri)
**Performance Goals**: Wallet creation <60s, restore scan <10min for typical wallets, responsive UI during sync (60fps), sub-second balance/status updates
**Constraints**: No secrets in UI layer, Orchard-only spending, fail-closed Tor mode, typed IPC only, memory zeroization for secrets
**Scale/Scope**: Single-user desktop wallet, ~15 screens, supports typical wallet sizes up to 1GB database

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify compliance with `.specify/memory/constitution.md` core principles:

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Secrets Stay in Rust | [x] Pass | All secret material (seeds, spending keys, raw signing payloads) handled exclusively in Rust backend. UI receives only derived addresses, balances, transaction summaries. Backend never sends mnemonic words or raw seeds to UI. Memory zeroization for secret types. Logs redact sensitive data by default. |
| II. Orchard-Only Privacy | [x] Pass | All spending operations use Orchard shielded pool only. Transparent funds receive-only with mandatory shielding before spend. Default receive address is shielded-only UA without transparent receiver. Transparent address exposed only as labeled compatibility option. |
| III. Fail-Closed Safety | [x] Pass | Tor mode enabled: fails if Tor unhealthy, no silent fallback to direct connections. Actionable error prompts (retry, disable, change endpoint). Wallet state integrity preserved on failures. Beta features (Tor) clearly labeled with defined failure modes. |
| IV. Typed IPC Contracts | [x] Pass | All IPC commands/events use versioned, strongly typed request/response models in zkore-core. schema_version field in every top-level payload. Strict deserialization rejecting unknown fields. No panics across IPC boundaries. Errors map to stable codes + user-safe messages. |
| V. Test-Driven Quality | [x] Pass | Unit tests for domain logic and IPC serialization. Integration tests for database/sync boundaries against Zaino + lightwalletd. Regression tests for privacy (fail-open, unintended transparent), key leakage via logs, malformed PCZT payload ingestion. CI covers multiple server implementations. |
| VI. Data Minimization | [x] Pass | Wallet state in zcash_client_sqlite wallet DB. App state (prefs, backup flags, swap records, server config) in separate SQLite store. No raw payloads, memo bodies in logs, or hardware wallet identifiers stored. Schema changes require forward migration + rollback strategy + tests. |
| VII. Decision Traceability | [x] Pass | Architectural decisions documented with ADR/RFC format. Security-sensitive reviews require maintainer familiar with key management, tx construction, networking, signing. Every milestone links implementation, tests, acceptance criteria. Changelog highlights privacy/security impacts. |

For detailed rules, see `docs/constitution.md`.

## Project Structure

### Documentation (this feature)

```text
specs/001-zkore-desktop-wallet/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   └── ipc-v1.ts        # TypeScript IPC command/event type definitions
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
crates/
├── zkore-core/                    # Domain types and IPC contracts
│   ├── src/
│   │   ├── lib.rs
│   │   ├── domain/                # Wallet, Account, Transaction, Swap models
│   │   ├── ipc/
│   │   │   └── v1/
│   │   │       ├── commands/      # Request/response structs per command
│   │   │       └── events/        # Event payload structs
│   │   └── errors.rs              # Stable error codes + user-safe messages
│   └── tests/
├── zkore-engine/                  # Wallet engine wrapping librustzcash
│   ├── src/
│   │   ├── lib.rs
│   │   ├── wallet_manager.rs      # Create, load, lock/unlock wallet
│   │   ├── address_service.rs     # Shielded UA rotation, compat t-addr
│   │   ├── sync_service.rs        # CompactTxStreamer sync, progress events
│   │   ├── tx_service.rs          # Send, shield, consolidate, submit
│   │   └── balance.rs             # Balance computation (orchard/transparent)
│   └── tests/
├── zkore-network/                 # gRPC + HTTP clients, transport abstraction
│   ├── src/
│   │   ├── lib.rs
│   │   ├── grpc_client.rs         # CompactTxStreamer gRPC client
│   │   ├── http_client.rs         # NEAR Intents 1Click HTTP client
│   │   └── transport.rs           # Tor-aware transport abstraction
│   └── tests/
├── zkore-keystone/                # Hardware wallet integration
│   ├── src/
│   │   ├── lib.rs
│   │   ├── ufvk.rs                # UFVK import/validation
│   │   ├── pczt.rs                # PCZT create/finalize helpers
│   │   └── payload.rs             # QR/file encoding helpers
│   └── tests/
└── zkore-tor/                     # Tor manager (embedded Arti)
    ├── src/
    │   ├── lib.rs
    │   └── manager.rs             # Off/Connecting/On/Error state machine
    └── tests/

apps/
├── zkore-app-tauri/               # Tauri application shell
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── commands/          # Tauri command handlers
│   │   │   ├── events.rs          # Event subscription bridge
│   │   │   └── windows.rs         # Window management (main, signing)
│   │   ├── Cargo.toml
│   │   └── tauri.conf.json
│   └── src/                       # React frontend
│       ├── main.tsx
│       ├── App.tsx
│       ├── components/
│       │   ├── common/            # Shared UI components
│       │   ├── wallet/            # Wallet-specific components
│       │   └── signing/           # Keystone QR components
│       ├── pages/
│       │   ├── Home.tsx           # Status widget, balance overview
│       │   ├── Receive.tsx        # Address display, QR, rotation
│       │   ├── Send.tsx           # Send form, confirmation
│       │   ├── Activity.tsx       # Transaction + swap list
│       │   ├── Swap.tsx           # NEAR Intents swap flows
│       │   ├── Settings.tsx       # Server, Tor, backup
│       │   └── Signing.tsx        # Full-screen Keystone signing
│       ├── services/
│       │   ├── ipc.ts             # Tauri IPC wrapper
│       │   └── events.ts          # Event subscription hooks
│       ├── hooks/                 # React hooks
│       └── types/                 # TypeScript type definitions
└── zkore-ui/                      # (Optional) shared UI package if needed

tests/
├── integration/                   # Cross-crate integration tests
│   ├── sync_tests.rs
│   ├── tx_tests.rs
│   └── keystone_tests.rs
└── e2e/                           # End-to-end tests (Tauri + UI)
```

**Structure Decision**: Desktop application with Rust workspace for backend crates (zkore-core, zkore-engine, zkore-network, zkore-keystone, zkore-tor) and Tauri app with React TypeScript frontend. Clear separation between wallet state (zcash_client_sqlite) and app metadata (separate SQLite). All secrets stay in Rust crates; UI communicates only via typed IPC.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitution violations. All principles pass.

The multi-crate workspace structure (5 backend crates + 1 Tauri app) is justified by the clear separation of concerns required by the constitution:
- **zkore-core**: Domain types and IPC contracts (Principle IV)
- **zkore-engine**: Wallet operations with secrets (Principle I)
- **zkore-network**: Transport abstraction for Tor fail-closed (Principle III)
- **zkore-keystone**: PCZT/signing with anti-fingerprinting (Principle I)
- **zkore-tor**: Isolated Tor state machine (Principle III)

## Constitution Check - Post-Design Re-evaluation

*Verified after Phase 1 design completion.*

| Principle | Post-Design Status | Validation |
|-----------|-------------------|------------|
| I. Secrets Stay in Rust | Confirmed | IPC contract (ipc-v1.ts) shows CreateWalletResponse returns seed_phrase ONLY on create, never stored in UI. All other commands return derived data only. |
| II. Orchard-Only Privacy | Confirmed | Data model enforces TransparentUTXO cannot be spent directly. AddressType enum separates ShieldedOnly (default) from Transparent (compatibility). |
| III. Fail-Closed Safety | Confirmed | TorState model has explicit Off/Connecting/On/Error states. IPC error codes include TOR_NOT_READY blocking operations when enabled but unhealthy. |
| IV. Typed IPC Contracts | Confirmed | ipc-v1.ts defines SCHEMA_VERSION=1, VersionedPayload base, and typed request/response for every command. ErrorCodes provide stable codes. |
| V. Test-Driven Quality | Confirmed | quickstart.md defines cargo test + vitest workflow. Research.md specifies regression tests for privacy, key leakage, malformed PCZT. |
| VI. Data Minimization | Confirmed | data-model.md defines separate Wallet DB (zcash_client_sqlite) and App Metadata DB (custom SQLite). No raw payloads stored. |
| VII. Decision Traceability | Confirmed | research.md documents all technology decisions with rationale and alternatives considered. Plan links to spec.md for acceptance criteria. |

**Result**: All constitution principles remain satisfied after detailed design. No violations or amendments required.

## Feature Implementation Notes

### Network Separation
- Network selection (mainnet/testnet) required at wallet creation
- Network choice is immutable after wallet creation (cannot be changed)
- Separate database files per network to prevent cross-network operations
- Network field stored in ServerConfig model

### Server Configuration
- **Default Server**: zec.rocks (Zaino+Zebra infrastructure)
  - Primary endpoint: `https://zec.rocks`
  - Regional endpoints for improved performance:
    - North America: `https://na.zec.rocks`
    - Europe: `https://eu.zec.rocks`
    - Middle East: `https://me.zec.rocks`
    - South America: `https://sa.zec.rocks`
- **Custom Server**: User can configure alternative lightwalletd/Zaino endpoint
  - Security warning displayed when using custom servers
  - Validation of server connectivity before saving

### Tor Anonymization
- Implementation: zcash_client_backend's tor feature using Arti (Rust-native Tor client)
- Production validation: Zashi 2.1 reference
- **Beta status**: Opt-in toggle with clear beta labeling in UI
- Fail-closed mode: Operations fail if Tor enabled but unhealthy (no silent fallback)
