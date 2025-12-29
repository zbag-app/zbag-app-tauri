# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Zkore Desktop is a privacy-by-design Zcash wallet built with Tauri v2 (Rust backend + React frontend). Key architectural principles:

- **Secrets stay in Rust** - Spending keys never reach the UI; mnemonic only crosses IPC for create/restore/backup/view flows
- **Shielded-by-default** - Transparent funds must be shielded before spending; transparent inputs only allowed for shielding transactions
- **Fail-closed Tor** - Network anonymization blocks rather than silently downgrades
- **Typed IPC contracts** - Versioned request/response models between UI and backend

## Build Commands

```bash
# Rust-only (excludes Tauri app which requires frontend dist)
cargo build --workspace --exclude zkore-app-tauri  # Build library crates
cargo test --workspace --exclude zkore-app-tauri   # Run library tests
cargo test -p zkore-engine           # Test specific crate
cargo fmt --all                      # Format
cargo clippy --workspace --all-targets --exclude zkore-app-tauri  # Lint

# Frontend (from apps/zkore-app-tauri)
bun install                          # Install dependencies
bun run build                        # Build frontend dist (required before full workspace build)
bun run dev                          # Vite dev server only
bun run tauri dev                    # Full Tauri development
bun run tauri build                  # Production build
bun test                             # Run tests

# Full workspace (requires frontend dist to exist)
cargo build --workspace              # Fails without `bun run build` first
```

Note: `cargo build --workspace` will fail with "frontendDist path doesn't exist" unless you first run `bun run build` in `apps/zkore-app-tauri` to generate the `dist` folder.

Note: To override the default lightwalletd server in debug builds, set `ZKORE_GRPC_URL` before running `tauri dev`.

## Architecture

```
crates/
  zkore-core/       # Domain types, IPC contracts, errors
  zkore-engine/     # Wallet operations (librustzcash wrapper), sync, tx service
  zkore-network/    # gRPC/HTTP clients, Tor transport
  zkore-keystone/   # Hardware wallet integration (PCZT)
  zkore-tor/        # Embedded Arti Tor client

apps/zkore-app-tauri/
  src-tauri/        # Tauri commands, app state
  src/              # React UI (pages/, components/, services/, hooks/)
```

### Key Files

- `crates/zkore-engine/src/wallet_manager.rs` - Core wallet lifecycle (create, restore, lock/unlock, accounts)
- `crates/zkore-engine/src/sync_service.rs` - Blockchain synchronization
- `crates/zkore-engine/src/tx_service.rs` - Transaction building and broadcasting
- `crates/zkore-core/src/ipc/` - IPC request/response types
- `apps/zkore-app-tauri/src/services/ipc.ts` - Frontend IPC client

## Toolchain

- Rust 1.92.0 (edition 2024) - pinned in `rust-toolchain.toml`
- Bun 1.3.5+
- Tauri v2 CLI via `@tauri-apps/cli` (dev dependency, not global)

## Testing

- Rust unit/integration tests: `crates/*/tests/*.rs`
- User story tests follow naming: `us<N>_*.rs`
- CI runs tests against two lightwalletd servers (constitution requirement)
- Migration tests: `cargo test -p zkore-engine --test app_db_migrations --test wallet_db_encryption_and_migrations`

## Coding Conventions

- Rust: `rustfmt` formatting; `thiserror` for library errors, `anyhow` at app boundaries
- TypeScript: `PascalCase.tsx` components, `useX` hooks
- Commit patterns: `US<N>: ...`, `docs: ...`, `chore: ...`, `fix: ...`

## Zcash Library Notes (librustzcash 0.21+)

```rust
// Protocol types moved to zcash_protocol
use zcash_protocol::consensus::{BlockHeight, Network};
use zcash_protocol::memo::Memo;
use zcash_protocol::value::Zatoshis;
use zip32::ExtendedSpendingKey;  // separate crate
```

## Constitution Requirements

Before merging wallet/signing/networking/persistence changes, verify:

- Secrets cannot reach the UI (except permitted mnemonic flows)
- Logs remain redacted (no seeds, keys, payloads, raw memos)
- Transparent funds cannot fund payments (shielding only)
- Tor mode cannot silently downgrade
- IPC types are versioned and validated
- Migrations are tested

See `.specify/memory/constitution.md` for full requirements.

## Reference Documentation

- Feature spec: `specs/001-zkore-desktop-wallet/spec.md`
- Implementation plan: `specs/001-zkore-desktop-wallet/plan.md`
- Data model: `specs/001-zkore-desktop-wallet/data-model.md`
- Quickstart: `specs/001-zkore-desktop-wallet/quickstart.md`
