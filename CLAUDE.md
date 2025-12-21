# Zkore Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-12-21

## Active Technologies

- Rust 1.92.0+ with edition 2024 (backend), TypeScript 5.x (frontend) (001-zkore-desktop-wallet)
- Bun 1.3.5+ (package manager)
- Key crates: zcash_client_backend 0.21+, zcash_client_sqlite 0.19+, zcash_primitives 0.26+

## Project Structure

```text
backend/
frontend/
tests/
```

## Commands

- `bun install` - Install frontend dependencies
- `bun run tauri dev` - Run development server
- `bun run tauri build` - Build production app
- `cargo test` - Run Rust tests
- `cargo clippy` - Run Rust linter

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
