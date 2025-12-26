# Implementation Plan: Zkore Desktop Wallet

**Branch**: `001-zkore-desktop-wallet` | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-zkore-desktop-wallet/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/scripts/bash/setup-plan.sh` for the setup workflow.

## Summary

Desktop-first shielded Zcash wallet with Zashi-style privacy-by-default (Sapling + Orchard shielded pools), Keystone hardware wallet support via air-gapped PCZT signing, NEAR Intents DEX integration for swaps/pay, and optional Tor anonymization. Built on Tauri (Rust backend + React TypeScript frontend) with strict trust boundaries ensuring spending secrets never reach the UI layer; mnemonic words (BIP-39 24-word English; no passphrase in v1) are only displayed/entered in explicitly permitted flows (create, backup verify, restore, view seed) and must never be persisted or logged by the UI.

## Technical Context

**Language/Version**: Rust 1.92.0 with edition 2024 (backend), TypeScript 5.x (frontend)
**Primary Dependencies**:
- Backend: zcash_client_backend 0.21+ (pczt, tor features), zcash_client_sqlite 0.19+, zcash_primitives 0.26+, zcash_protocol 0.7+, Tauri v2, tonic 0.14+ (gRPC)
- Frontend: React 18+, @keystonehq/animated-qr, @keystonehq/keystone-sdk, bun 1.3.5+ (package manager)

> **Version Strategy**: We use caret (^) semver constraints aligned with librustzcash/Zashi. This allows security fixes while maintaining compatibility. Always commit Cargo.lock and build with `--locked` in production.
**Storage**: Encrypted wallet DB (zcash_client_sqlite-backed) + separate SQLite app metadata DB
  - Wallet DB encryption uses SQLCipher with a per-wallet DEK wrapped by a password-derived KEK (Argon2id; parameters versioned per wallet). Optional OS keychain “remember unlock” stores unlock material in the OS credential store and MUST NOT satisfy per-action re-auth.
  - All schema changes include forward migration + rollback strategy + automated migration tests (gated in CI).
  - Wallet directory structure with network separation:
    - `~/.zkore/wallets/mainnet/{wallet-id}/` (mainnet wallets)
    - `~/.zkore/wallets/testnet/{wallet-id}/` (testnet wallets)
  - Network selection at wallet creation (immutable after creation)
  - Separate database files per network
**Testing**: cargo test (Rust), vitest/jest (TypeScript), integration tests against lightwalletd endpoints (at least two independent deployments in CI)
**Target Platform**: macOS, Windows, Linux (desktop)
**Project Type**: Desktop application with Rust backend and web frontend (Tauri)
**Performance Goals**: Wallet creation <60s, restore scan <10min for typical wallets, responsive UI during sync (60fps), balance/status updates <=2s (target <1s)
**Constraints**: No spending secrets in UI layer, payments funded from shielded pools (Sapling + Orchard; transparent inputs allowed only for explicit shielding transactions), fail-closed Tor mode, typed IPC only, memory zeroization for secrets, encrypt wallet DB at rest, manual wallet-password re-auth required per spend/seed-view (OS keychain must not satisfy re-auth)
**Scale/Scope**: Single-user desktop wallet, ~15 screens, supports typical wallet sizes up to 1GB database
**Logging**: tracing + tracing-appender for structured file logging with daily rotation. Logs stored at `~/.zkore/logs/`. No remote telemetry. Sensitive data (memos, full addresses) redacted by default.
**Accessibility**: Full keyboard navigation, ARIA labels via radix-ui primitives, visible focus indicators, standard shortcuts (Tab/Enter/Escape/arrows)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify compliance with `.specify/memory/constitution.md` core principles:

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Secrets Stay in Rust | [x] Pass | Spending keys and raw signing payloads handled exclusively in Rust backend. UI receives only derived addresses, balances, transaction summaries. Mnemonic is returned during CreateWallet for backup display, accepted for restore entry, and may be re-displayed only via explicit user action (manual wallet-password re-authentication); mnemonic is never persisted or logged by the UI. Memory zeroization for secret types. Logs redact sensitive data by default. |
| II. Shielded-by-Default Privacy | [x] Pass | Spending uses shielded pools (prefer Orchard; allow Sapling). Transparent funds are receive-only until explicitly shielded; transparent recipients are allowed only with explicit privacy-downgrade acknowledgement. Default receive address is shielded-only UA without transparent receiver. Transparent receive address is a labeled compatibility option (single non-rotating in v1). |
| III. Fail-Closed Safety | [x] Pass | Tor mode enabled: fails if Tor unhealthy, no silent fallback to direct connections. Actionable error prompts (retry, disable, change endpoint). Wallet state integrity preserved on failures. Beta features (Tor) clearly labeled with defined failure modes. |
| IV. Typed IPC Contracts | [x] Pass | All IPC commands/events use versioned, strongly typed request/response models in zkore-core. schema_version field in every top-level payload. Strict deserialization rejecting unknown fields. Command boundary uses IpcResult<T> (no thrown errors across IPC). Events are emitted on fixed channels: sync, balance, tx, swap, tor, wallet-status. |
| V. Test-Driven Quality | [x] Pass | Unit tests for domain logic and IPC serialization. Integration tests for database/sync boundaries against lightwalletd endpoints (at least two independent deployments in CI). Regression tests for privacy (fail-open, unintended transparent), key leakage via logs, malformed PCZT payload ingestion. CI covers multiple independent lightwalletd deployments. |
| VI. Data Minimization | [x] Pass | Wallet state in encrypted zcash_client_sqlite wallet DB. App state (prefs, backup flags, swap records, server config) in separate SQLite store. No raw payloads, memo bodies in logs, or hardware wallet identifiers stored. Schema changes require forward migration + rollback strategy + tests. |
| VII. Decision Traceability | [x] Pass | Architectural decisions documented with ADR/RFC format. Security-sensitive reviews require maintainer familiar with key management, tx construction, networking, signing. Every milestone links implementation, tests, acceptance criteria. Changelog highlights privacy/security impacts. |

For detailed rules, see `.specify/memory/constitution.md`.

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
│   │   ├── address_service.rs     # Shielded UA rotation, single compat t-addr
│   │   ├── sync_service.rs        # CompactTxStreamer sync, progress events
│   │   ├── tx_service.rs          # Send, shield, consolidate, submit
│   │   └── balance.rs             # Balance computation (shielded/transparent)
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
│       │   ├── useFocusTrap.ts    # Focus management for modals
│       │   └── useKeyboardShortcuts.ts  # Global keyboard shortcuts
│       └── types/                 # TypeScript type definitions
└── zkore-ui/                      # (Optional) shared UI package if needed

tests/
├── integration/                   # Cross-crate integration tests
│   ├── sync_tests.rs
│   ├── tx_tests.rs
│   └── keystone_tests.rs
└── e2e/                           # End-to-end tests (Tauri + UI)
```

**Structure Decision**: Desktop application with Rust workspace for backend crates (zkore-core, zkore-engine, zkore-network, zkore-keystone, zkore-tor) and Tauri app with React TypeScript frontend. Clear separation between wallet state (encrypted zcash_client_sqlite wallet DB) and app metadata (separate SQLite). Spending secrets stay in Rust crates; mnemonic words are handled only in explicitly permitted, transient flows in the UI via typed IPC.

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
| I. Secrets Stay in Rust | Confirmed | IPC contract returns mnemonic for CreateWallet backup display, accepts restore seed input, and supports user-initiated seed re-display gated by manual wallet-password re-authentication; mnemonic is never persisted or logged by the UI. All other commands return derived data only. |
| II. Shielded-by-Default Privacy | Confirmed | Data model enforces transparent UTXOs cannot be spent directly. AddressType enum separates ShieldedOnly (default) from Transparent (compatibility). Send flow supports UA/Sapling/Orchard recipients; transparent recipients require explicit privacy acknowledgement. |
| III. Fail-Closed Safety | Confirmed | TorState model has explicit Off/Connecting/On/Error states. IPC error codes include TOR_NOT_READY blocking operations when enabled but unhealthy. |
| IV. Typed IPC Contracts | Confirmed | ipc-v1.ts defines SCHEMA_VERSION=1, VersionedPayload base, and typed request/response for every command. Command boundary uses IpcResult<T> (no thrown errors across IPC). Events are emitted on fixed channels: sync, balance, tx, swap, tor, wallet-status. ErrorCodes provide stable codes. |
| V. Test-Driven Quality | Confirmed | quickstart.md defines cargo test + vitest workflow. Research.md specifies regression tests for privacy, key leakage, malformed PCZT. |
| VI. Data Minimization | Confirmed | data-model.md defines encrypted Wallet DB (zcash_client_sqlite) and separate App Metadata DB (custom SQLite). No raw payloads stored. |
| VII. Decision Traceability | Confirmed | research.md documents all technology decisions with rationale and alternatives considered. Plan links to spec.md for acceptance criteria. |

**Result**: All constitution principles remain satisfied after detailed design. No violations.

## Feature Implementation Notes

### Wallet Security (password, encryption, re-auth)
- Spend-capable secret material (mnemonic / spending capability) encrypted at rest with a user-defined wallet password; optional OS keychain “remember unlock” for convenience.
- Wallet defaults to locked on restart; prompt for wallet password on app launch unless keychain auto-unlock is enabled.
- Manual wallet-password re-authentication required for every spending attempt (send, shield, swap-from-ZEC) and for "View seed phrase"; OS keychain “remember unlock” MUST NOT satisfy per-action re-auth.
- Wallet DB encrypted at rest; transaction history, balances, addresses, and note metadata must not be readable without successful unlock.
- Memos treated as sensitive: memo plaintext must not be written to disk; encryption-at-rest must cover memo contents.
- Wallet DB key hierarchy (v1): wallet password → Argon2id KEK → unwrap per-wallet DEK; the DEK is the raw SQLCipher key for the wallet DB. Store only `wrapped_dek` + KDF params/salt in app metadata; prefer storing DEK (not password) in OS keychain for “remember unlock”.
- Migration safety (v1): before applying any app metadata DB or wallet DB migration, create a pre-migration DB snapshot; on failure, rollback by restoring the snapshot; migration tests are required and run in CI.

### Security & Persistence Design Notes (v1)

This section is **non-normative**. It clarifies how the requirements above are intended to be met so implementation tasks and tests can be unambiguous.

#### Wallet DB encryption approach (zcash_client_sqlite-backed)

- Zkore uses **SQLCipher** (SQLite page-level encryption) for the wallet DB (transaction history, balances, addresses, note metadata). The encryption mechanism MUST encrypt the entire DB file at rest (not just selected columns).
- Zkore uses a per-wallet **key hierarchy**:
  - **Wallet password** → KDF → **Key Encryption Key (KEK)**
  - Random 32-byte **Data Encryption Key (DEK)** provided as the raw SQLCipher key for the wallet DB (and used to encrypt any other wallet-private blobs)
  - The DEK is stored only as **wrapped_dek** (encrypted with the KEK) in app metadata; the DEK itself is never written to disk in plaintext
- **KDF (initial v1 parameters)**:
  - Algorithm: **Argon2id**
  - Parameters: memory = **64 MiB**, iterations = **3**, parallelism = **1**
  - Salt: **16 bytes**, random per wallet
  - Output: **32 bytes** (KEK)
  - KDF parameters and salt are stored per-wallet and are **versioned** to allow future tuning/migration
- **AEAD for wrapping**:
  - Wrap/unwrap the DEK using an AEAD (e.g., **XChaCha20-Poly1305**) with associated data binding to `(wallet_id, network, schema_version)` to prevent cross-wallet swapping
- **Unlock validation**:
  - The wallet password is validated by deriving the KEK and successfully unwrapping the DEK; there is no separate password hash stored

#### Unlock lifecycle, “remember unlock”, and re-auth separation

- **Locked by default on restart**. If the user enables “remember unlock”, the wallet MAY auto-unlock on launch via OS keychain, but the default is locked.
- While unlocked, the DEK (and other derived secret material) MUST exist only in Rust process memory and MUST be zeroized on lock and on process exit.
- **OS keychain “remember unlock”** stores unlock material only in the OS credential store (never plaintext on disk). For v1, store **DEK** (preferred) or an additional key-wrapping secret in the keychain; do not store the wallet password if avoidable.
- **Per-action re-auth** is always password-based:
  - Every spend (send, shield, swap-from-ZEC) and every “View seed phrase” MUST require the user to enter the wallet password to obtain a short-lived reauth token.
  - Keychain auto-unlock MUST NOT satisfy per-action re-auth and MUST NOT mint reauth tokens without explicit password entry.

#### Backup verification challenge semantics (v1)

- Backup verification MUST be performed via a backend-issued challenge containing:
  - `challenge_id` (opaque identifier)
  - exactly **4** distinct word indices in the range **1..=24** (1-based, user-facing “word #N”)
- Challenge validity:
  - Challenges MUST expire after **10 minutes**
  - After **5** failed attempts, the challenge MUST be invalidated and the user MUST request a new challenge
  - Challenges MUST be stored in-memory only (not persisted). App restart MUST invalidate all outstanding `challenge_id` values.
- Verification behavior:
  - A successful verification MUST mark backup as complete and clear the active challenge
  - A failed verification MUST increment the attempt counter and return a stable, user-safe error without revealing which word(s) were incorrect

#### Password loss and recovery

- If the user forgets the wallet password, the encrypted wallet DB cannot be unlocked. The recovery path is to **restore from seed phrase** into a new wallet (new password) after explicitly deleting/archiving the old encrypted DB.

#### Key rotation / password change

- Changing the wallet password is **out of scope for v1**, but the key hierarchy is designed to support it by re-wrapping the DEK with a newly derived KEK.
- DEK rotation (re-keying the DB to a new DEK) is also out of scope for v1; if added, it MUST be implemented as an explicit migration with rollback + tests (see NFR-016).

#### Schema migration safety (forward + rollback + tests)

- For both the **app metadata DB** and the **wallet DB**, any schema migration MUST:
  1. Create an on-disk backup snapshot of the pre-migration DB file(s)
  2. Apply the forward migration
  3. Run post-migration validation (e.g., can open DB, expected tables/columns present)
  4. If any step fails, rollback by restoring the snapshot
- Automated tests MUST exercise:
  - upgrade from at least one older schema fixture → current schema
  - rollback path (restore snapshot) when a migration/validation step fails

#### Transaction state source-of-truth (pending/confirmed)

- “Pending” and “Confirmed” are derived from two sources:
  - **Mempool detection** from the configured lightwalletd server via CompactTxStreamer mempool APIs (to satisfy FR-013 for incoming transactions, where supported)
  - **Chain inclusion** from compact block scanning (to satisfy FR-014)
- Outgoing transactions MUST be shown as **pending** once submission is accepted (even if mempool detection is delayed), and MUST transition to **confirmed** once mined; reorg handling may transition confirmed → pending again.

#### Broadcast retry queue (disconnect during broadcast)

- If broadcast fails after signing (e.g., network disconnect), the wallet MUST queue the signed tx bytes in encrypted wallet storage so the user can retry later (including after app restart).
- Retry MUST require explicit user intent (no silent re-broadcast) and MUST require manual wallet-password re-authentication.
- The queue MUST be bounded by time: entries are deleted after successful broadcast or after **7 days** (whichever comes first).
- Logs MUST NOT include signed tx bytes or other raw payloads for queued broadcasts.

#### Shield-and-consolidate semantics (transparent → Orchard)

- The one-click “Shield and Consolidate” action MUST spend **all spendable** transparent UTXOs for the wallet/account; v1 does not provide manual UTXO selection.
- Fees are paid from transparent inputs: the Orchard output value = sum(transparent inputs) − fee. The transaction MUST NOT create a transparent change output.
- If the transparent input set cannot fit into a single transaction due to size/limit constraints, the wallet MUST automatically batch into multiple shielding transactions and surface progress; the operation completes when no spendable transparent UTXOs remain.
- If the total spendable transparent balance is insufficient to cover the required fee (or would result in a zero/invalid output), the wallet MUST fail with a stable, user-safe error and provide actionable guidance (including required-minimum amount) to acquire additional transparent ZEC.

#### Spend-before-sync semantics (restore)

- During restore/sync, the wallet MUST distinguish between:
  - `shielded_spendable`: funds safe to spend with the wallet’s current witness/anchor state
  - `shielded_pending`: funds detected but not yet safe to spend (e.g., insufficient confirmations, incomplete witness tracking)
- Spend-before-sync means: **spending is allowed during restore only from `shielded_spendable`**, while restore continues in the background; spending MUST NOT draw from `shielded_pending`.

### Network Separation
- Network selection (mainnet/testnet) required at wallet creation
- Network choice is immutable after wallet creation (cannot be changed)
- Separate database files per network to prevent cross-network operations
- Network field stored in ServerConfig model

### Server Configuration
- **Default Servers**: lightwalletd + Zebra infrastructure (CompactTxStreamer gRPC)
  - Primary endpoint: `https://lwd.zec.pro` (team)
  - Regional endpoints: `https://zec.rocks`, `https://na.zec.rocks`, `https://eu.zec.rocks`, `https://sa.zec.rocks`
- **Testnet**: `https://lwd.testnet.zec.pro` (team lightwalletd + Zebra)
  - SSL via reverse proxy recommended for production-like testing
  - Development/CI only: configure default server override via `ZKORE_GRPC_URL` environment variable; production builds should rely on persisted server configuration and MUST NOT silently override user-selected servers via environment variables
- **Custom Server**: User can configure alternative lightwalletd endpoint
  - Security warning displayed when using custom servers
  - Validation of server connectivity and network match before saving
- **Compatibility testing**: CI must test against at least two independent lightwalletd deployments (primary + secondary)

### Privacy / Telemetry
- No remote telemetry or crash reporting: audit dependencies and build config to ensure nothing transmits telemetry/crash reports by default; only local logs are produced (see NFR-002).

### Swaps (NEAR Intents)
- Swaps are supported only for Mainnet wallets due to 1Click API deployment; disable Swap UI and return stable error for Testnet

### Tor Anonymization
- Implementation: zcash_client_backend's tor feature using Arti (Rust-native Tor client)
- Production validation: Zashi 2.1 reference
- **Beta status**: Opt-in toggle with clear beta labeling in UI
- Fail-closed mode: Operations fail if Tor enabled but unhealthy (no silent fallback)
