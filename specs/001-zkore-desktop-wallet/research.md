# Research: Zkore Desktop Wallet

**Branch**: `001-zkore-desktop-wallet`
**Status**: Complete
**Purpose**: Resolve unknowns from Technical Context and establish best practices for key dependencies

## Research Topics

### 1. zcash_client_backend Integration

**Decision**: Use zcash_client_backend with `pczt` and `tor` feature flags

**Rationale**:
- Provides official Zcash light client implementation with Orchard support
- PCZT (Partially Created Zcash Transaction) feature enables air-gapped signing workflows required for Keystone
- Tor feature provides embedded Arti-based Tor client for fail-closed anonymization
- zcash_client_sqlite provides standard wallet database format with automatic migrations

**Alternatives Considered**:
- Custom wallet implementation: Rejected due to complexity of Zcash cryptography and lack of auditing
- Using only zcash_primitives: Rejected as it lacks light client sync, transaction construction, and witness management

**Implementation Notes**:
- Enable features: `orchard`, `pczt`, `tor` in Cargo.toml
- Use `WalletDb` from zcash_client_sqlite for wallet persistence
- Use `LightWalletReader` trait implementations for CompactTxStreamer compatibility

### 2. Keystone PCZT Signing Protocol

**Decision**: Use PCZT (ZIP-320) for unsigned transaction format with Keystone SDK for QR encoding

**Rationale**:
- PCZT is the official Zcash standard for partially signed transactions (similar to PSBT for Bitcoin)
- Keystone firmware supports PCZT format for Zcash Orchard transactions
- @keystonehq/animated-qr handles multi-frame QR for large payloads
- @keystonehq/keystone-sdk provides encoding/decoding utilities

**Alternatives Considered**:
- Custom binary format: Rejected for interoperability concerns
- JSON-based format: Rejected due to payload size and lack of standard

**Implementation Notes**:
- Use `pczt::Pczt` for building unsigned transactions
- Maximum QR frame size: 2953 bytes (version 40, L error correction)
- Animated QR frame rate: 10 fps default, 3 fps for "slow mode"
- File fallback: Export as `.pczt` binary file for microSD transfer

### 3. NEAR Intents 1Click API Integration

**Decision**: Backend-owned HTTP client with typed request/response models

**Rationale**:
- 1Click API provides complete swap flow: quote -> deposit -> status polling
- Backend ownership ensures Tor routing and prevents API key exposure in UI
- Stateless API with idempotent operations simplifies error recovery

**Alternatives Considered**:
- Direct UI integration: Rejected per constitution (backend-owned networking)
- WebSocket for status updates: Rejected as 1Click uses polling model

**Implementation Notes**:
- Base URL: `https://1click.chaindefuser.com/v1`
- Endpoints: `/quote`, `/deposit-address`, `/status/{intent_id}`
- Poll interval: 5 seconds for active swaps, exponential backoff on errors
- Timeout: 30 seconds per request
- State mapping: `PENDING` -> `Pending`, `COMPLETED` -> `Completed`, `FAILED` -> `Failed`, `REFUNDED` -> `Refunded`

### 4. Tor Integration (Arti-based)

**Decision**: Embedded Arti Tor client via zcash_client_backend tor feature

**Rationale**:
- Arti version: 0.35+ (latest in librustzcash)
- Zashi 2.1 uses this in production (beta feature)
- Proven fail-closed behavior in production
- Routes: tx submit, tx fetch, swap APIs, rate APIs

**Alternatives Considered**:
- System Tor daemon: Rejected for deployment complexity and platform differences
- tor-hidden-services crate: Rejected as Arti is recommended by Zcash ecosystem

**Implementation Notes**:
- State machine: Off -> Connecting -> On -> Error
- Circuit establishment timeout: 60 seconds
- Health check: Verify circuit before marking as On
- Fail-closed: Block all sensitive requests when enabled but not healthy
- Sensitive requests: tx submit, tx fetch, swap API calls

### 5. Tauri v2 IPC Architecture

**Decision**: Typed commands with versioned schemas and event-based updates

**Rationale**:
- Tauri v2 provides improved IPC with better TypeScript integration
- Typed commands ensure constitution compliance (Principle IV)
- Event system enables real-time UI updates without polling

**Alternatives Considered**:
- Electron: Rejected for larger binary size and memory footprint
- Wails: Rejected for less mature ecosystem

**Implementation Notes**:
- Command prefix: `zkore_` for all Tauri commands
- Event channels: `sync`, `balance`, `tx`, `swap`, `tor`
- Schema versioning: `schema_version: u32` field in all payloads
- Error format: `{ code: string, message: string, details?: object }`

### 6. Address Rotation Strategy

**Decision**: Diversifier-based rotation with persistent index

**Rationale**:
- Unified Addresses support diversifiers for unlimited unique addresses
- Same viewing key, different address appearance
- All addresses resolve to same wallet for incoming funds

**Alternatives Considered**:
- Account-based rotation: Rejected as it complicates balance tracking
- Random diversifiers: Rejected for reproducibility concerns

**Implementation Notes**:
- Store `next_diversifier_index: u64` in app metadata DB
- Increment on each `get_fresh_shielded_ua()` call
- Shielded-only UA: Encode with only Orchard receiver
- Transparent compatibility: Separate derivation path, not embedded in UA

### 7. Backup Verification Protocol

**Decision**: Word-index challenge with 4-word minimum verification

**Rationale**:
- Verifies user has recorded seed phrase correctly
- Prevents false confidence from partial recording
- Industry standard approach used by Zashi and other wallets

**Alternatives Considered**:
- Full phrase re-entry: Rejected for UX friction
- Checksum verification: Rejected as it doesn't prove recording

**Implementation Notes**:
- Challenge: Request 4 words at random indices (e.g., words 3, 7, 15, 22)
- Validation: Backend compares submitted words against stored mnemonic
- State update: Set `backup_required = false`, `backup_completed_at = now()`
- UI: Never display full seed phrase after initial creation

### 8. Spend-Before-Sync Implementation

**Decision**: Phased implementation with clear "spendable now" vs "still scanning" distinction

**Rationale**:
- Matches Zashi direction for fund availability
- Requires careful witness/anchor tracking in zcash_client_backend
- UI must clearly communicate what is available

**Alternatives Considered**:
- Block all spending until full sync: Rejected for poor UX on large wallets
- Immediate spending without distinction: Rejected for potential confusion

**Implementation Notes**:
- Track `scan_frontier_height` and `wallet_tip_height` separately
- Compute `spendable_orchard` (notes with valid witnesses)
- Compute `pending_orchard` (detected but not yet spendable)
- Phase 1: UI shows distinction, backend enforces spendable-only sends
- Phase 2: Enable actual spend-before-sync when engine supports it

### 9. Birthday Height Estimation

**Decision**: Static checkpoint table with periodic updates

**Rationale**:
- Reduces scan time significantly for restore operations
- Checkpoint table is small and can be bundled with app
- Fallback to genesis if date predates checkpoints

**Alternatives Considered**:
- Server-side birthday estimation: Rejected for additional dependency
- Always scan from genesis: Rejected for performance

**Implementation Notes**:
- Table format: `[(date_range_start, block_height), ...]`
- Granularity: Weekly checkpoints
- Source: Derived from Zcash block explorer data
- Update mechanism: App update or optional server fetch

### 10. Wallet Database Migration Strategy

**Decision**: Use zcash_client_sqlite migrations + custom app metadata migrations

**Rationale**:
- zcash_client_sqlite handles wallet schema migrations automatically
- App metadata requires separate migration system for custom tables
- Both must be atomic and support rollback

**Alternatives Considered**:
- Single database: Rejected per constitution (separate wallet/app state)
- No-SQL for app state: Rejected for transaction safety

**Implementation Notes**:
- Wallet DB: Managed by zcash_client_sqlite, location per wallet directory
- App DB: Separate SQLite with custom migration runner
- App DB tables: `app_flags`, `servers`, `tor_settings`, `swaps`, `receive_rotation`
- Migration version table: `_app_migrations(version, applied_at)`

### 11. Network Selection Strategy

**Decision**: Runtime network selection at wallet creation, immutable after

**Rationale**:
- Separate database directories per network prevent data corruption
- Same seed generates different addresses (BIP-44 coin_type: 133' mainnet, 1' testnet)
- Address prefixes prevent cross-network mistakes (mainnet: u1..., testnet: utest1...)
- Network is a fundamental wallet property that should not change

**Alternatives Considered**:
- Network switching on existing wallet: Rejected for data integrity and confusion
- Global network setting: Rejected to support multiple wallets on different networks

**Implementation Notes**:
- Network selection during wallet creation flow
- Store network in wallet metadata (immutable field)
- Database path includes network: `wallets/{wallet_id}/{network}/`
- UI clearly indicates network in wallet list and detail screens
- No UI affordance for changing network after creation

### 12. Server Configuration

**Decision**: Support custom servers with security warnings

**Rationale**:
- Default: zec.rocks (Zaino+Zebra, replaced lightwalletd+zcashd April 2025)
- Regional endpoints improve latency and reliability
- Custom server support enables enterprise and privacy-focused deployments
- Connection test prevents invalid configurations

**Alternatives Considered**:
- Hardcoded servers only: Rejected for reduced flexibility
- No default server: Rejected for poor UX
- No connection validation: Rejected for error-prone setup

**Implementation Notes**:
- Default server: `https://zec.rocks`
- Regional endpoints: `na.zec.rocks`, `eu.zec.rocks`, `me.zec.rocks`, `sa.zec.rocks`
- Connection test: Call `GetLightdInfo` before saving server config
- Server network validation: Must match wallet network (mainnet/testnet)
- Security warning: Display when user configures non-default server
- Server stored in app metadata DB per wallet

## Resolved Clarifications

All technical context items have been resolved. No outstanding clarifications needed.

| Original Unknown | Resolution |
|-----------------|------------|
| Rust version | 1.92.0+ (development toolchain, MSRV 1.85.1 for librustzcash compatibility) |
| Rust edition | 2024 (aligned with librustzcash/Zashi) |
| Package manager | bun 1.3.5+ |
| Primary dependencies | zcash_client_backend 0.21+, zcash_client_sqlite 0.19+, zcash_primitives 0.26+, zcash_protocol 0.7+, Tauri v2, tonic 0.14+, Arti |
| Storage | Dual SQLite (wallet + app metadata) |
| Testing | cargo test + vitest + integration tests |
| Target platforms | macOS, Windows, Linux |
| Performance goals | <60s wallet creation, <10min typical restore |
| Constraints | Secrets in Rust only, Orchard-only, fail-closed Tor |
| Default server | zec.rocks (Zaino+Zebra) |
| Network selection | Runtime at wallet creation, immutable after |
| Version strategy | Caret (^) semver constraints, commit Cargo.lock, build with --locked |

### Edition 2024 Rationale

We use Rust edition 2024 because:
1. **Ecosystem alignment**: librustzcash and Zashi use edition 2024
2. **Improved safety**: Explicit `unsafe` blocks in `unsafe fn` reduce accidental unsafety
3. **Production-proven**: Stable since Rust 1.85.0, used in Zcash infrastructure
4. **Future-ready**: Prepared for generators and better async ergonomics

We target Rust 1.92.0 as the development toolchain while maintaining MSRV 1.85.1 compatibility with librustzcash.

Key migration considerations:
- RPIT lifetime capture has new semantics (may need `use<..>` bounds)
- `gen` keyword is reserved (avoid as identifier)
- Temporal scope changes affect drop order

### Dependency Version Strategy

We follow the same approach as Zashi (zcash-light-client-ffi):
- Use **caret constraints** (e.g., `"0.21"`) for semver-compatible updates
- Align with librustzcash releases for ecosystem compatibility
- Always commit `Cargo.lock` for reproducible builds
- Build production with `cargo build --release --locked`
- Run `cargo audit` in CI for security scanning

## References

### Zcash Libraries
- [zcash_client_backend docs](https://docs.rs/zcash_client_backend)
- [zcash_client_sqlite docs](https://docs.rs/zcash_client_sqlite)
- [librustzcash repository](https://github.com/zcash/librustzcash) - source of truth for version alignment
- [PCZT specification (ZIP-320)](https://zips.z.cash/zip-0320)

### Rust Edition 2024
- [Rust 2024 Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
- [Announcing Rust 1.85.0 and Rust 2024](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/)
- [Changes to impl Trait in Rust 2024](https://blog.rust-lang.org/2024/09/05/impl-trait-capture-rules.html)

### Infrastructure
- [Keystone SDK documentation](https://dev.keyst.one/docs/integration-guide-basics/install-the-sdk)
- [NEAR Intents 1Click API](https://docs.near-intents.org/near-intents/integration/distribution-channels/1click-api)
- [Tauri v2 documentation](https://v2.tauri.app)
- [Arti (Tor implementation in Rust)](https://gitlab.torproject.org/tpo/core/arti)
- [Zaino GitHub](https://github.com/zingolabs/zaino)
- [Zashi 2.1 Tor announcement](https://electriccoin.co/blog/zashi-2-1-enhanced-privacy-with-tor-beta/)
- [zec.rocks infrastructure](https://zec.rocks)
