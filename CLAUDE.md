# Zkore Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-12-21

## Active Technologies

- Rust 1.92.0+ with edition 2024 (backend), TypeScript 5.x (frontend) (001-zkore-desktop-wallet)
- Bun 1.3.5+ (package manager)
- Key crates: zcash_client_backend 0.21+, zcash_client_sqlite 0.19+, zcash_primitives 0.26+

## Project Structure

```text
crates/                          # Rust backend workspace
  zkore-core/                    # Domain types and IPC contracts
  zkore-engine/                  # Wallet engine (librustzcash wrapper)
  zkore-network/                 # gRPC/HTTP clients, Tor transport
  zkore-keystone/                # Hardware wallet (PCZT, UFVK)
  zkore-tor/                     # Embedded Arti Tor manager
apps/
  zkore-app-tauri/               # Tauri app shell
    src-tauri/                   # Rust Tauri commands
    src/                         # React TypeScript frontend
tests/
  integration/                   # Cross-crate integration tests
  e2e/                           # End-to-end Tauri tests
specs/
  001-zkore-desktop-wallet/      # Feature specification and design docs
docs/                            # Project-wide documentation
```

## Commands

- `bun install` - Install frontend dependencies
- `bun tauri dev` - Run development server (Tauri CLI via @tauri-apps/cli)
- `bun tauri build` - Build production app
- `cargo test --workspace` - Run all Rust tests
- `cargo clippy --workspace` - Run Rust linter
- `cargo build --release --locked` - Production build with lock verification

## Code Style

Rust 1.92.0+ (backend), TypeScript 5.x (frontend): Follow standard conventions

## Network Configuration

- Wallets support mainnet and testnet
- Network is selected at wallet creation and is immutable
- Separate directories: ~/.zkore/wallets/mainnet/ and ~/.zkore/wallets/testnet/

## Default Server

- Default: zec.rocks (Zaino+Zebra infrastructure)
- Custom servers supported with validation

## Recent Changes

- 001-zkore-desktop-wallet: Added Rust 1.92.0+ (backend), TypeScript 5.x (frontend), Bun 1.3.5+ (package manager)

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
