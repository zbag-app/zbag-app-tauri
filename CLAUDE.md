# zbag Desktop

## Overview
Tauri v2 Zcash wallet (Rust backend + React frontend).

- **Secrets stay in Rust** - mnemonic only crosses IPC for create/restore/backup/view
- **SensitiveString is defense-in-depth** - Rust drop zeroization only; not a guarantee against core dumps, debugger inspection, or frontend/JS retention
- **Use Zeroizing vs SensitiveString** - `Zeroizing<T>` for short-lived internal secrets; `SensitiveString` for IPC-facing string fields (serde-transparent)
- **Shielded-by-default** - transparent inputs only for shielding tx
- **Fail-closed Tor** - blocks rather than downgrades
- **Typed IPC contracts** - versioned request/response

## Build

```bash
make build          # Rust library crates
make test           # All Rust tests
make fmt            # Format
make clippy         # Lint
make pre-commit     # Format + lint

make install        # Frontend deps (bun)
make build-frontend # Required before full workspace
make dev            # Full Tauri development
```

Override lightwalletd: `ZBAG_GRPC_URL`. Run `make help` for all targets.

## CEF Network Hardening

CEF is an offline renderer in zbag. Do not regress the no-network policy without updating all three guardrails:

- `scripts/check-cef-network-hardening.sh` for static pre-commit checks
- `apps/zbag-app-tauri/src-tauri/tests/cef_runtime_args.rs` for parsed runtime arguments
- `cargo xtask cef-smoketest` for packaged-app socket smoke testing

Editing `cef_runtime_args`, `CEF_DISABLED_FEATURES`, or `CEF_HOST_RESOLVER_RULES` requires updating the matching `EXPECTED_*` constants in the integration test. The runtime smoke uses `ZBAG_GRPC_URL=https://127.0.0.1:1` and isolated `HOME`/`XDG_*` directories so it exercises CEF cold start without a real wallet profile.

## Version Control

Standard git workflow. Common commands:
- `git status` / `git diff` - View changes
- `git add` / `git commit` - Stage and commit
- `git push` - Push to remote
- `git pull --rebase` - Update from remote

**Force pushing:** Always use `--force-with-lease` instead of `--force`. It's safer because it fails if the remote has new commits you haven't fetched, preventing accidental overwrites of others' work.

### Git Worktrees

**Always create new worktrees one level up** from this checkout:
```bash
git worktree add ../zbag-issue-<N> -b fix/description
```

**Important:**
- Do NOT make changes to `main` or `dev` branches in this directory unless the user explicitly requests it
- This directory should remain on `dev` for reference; use worktrees for feature work
- Existing worktrees: `git worktree list`

## Architecture

```
crates/
  zbag-core/      # Types, IPC, errors
  zbag-engine/    # Wallet ops, sync, tx
  zbag-network/   # gRPC, Tor transport
  zbag-keystone/  # Hardware wallet (PCZT)
  zbag-tor/       # Arti client

apps/zbag-app-tauri/
  src-tauri/       # Tauri commands
  src/             # React UI
```

### Key Files

- `crates/zbag-engine/src/wallet_manager.rs` - Wallet lifecycle
- `crates/zbag-engine/src/sync_service.rs` - Blockchain sync
- `crates/zbag-engine/src/tx_service.rs` - Transaction building/broadcast
- `crates/zbag-core/src/ipc/` - IPC types
- `apps/zbag-app-tauri/src/services/ipc.ts` - Frontend IPC client

### Adding Tauri Commands

Register commands in BOTH `apps/zbag-app-tauri/src-tauri/src/lib.rs` and
`apps/zbag-app-tauri/src-tauri/src/main.rs`.

Update:
- `apps/zbag-app-tauri/src-tauri/src/lib.rs`: `commands::wallet::zbag_xxx`
- `apps/zbag-app-tauri/src-tauri/src/main.rs`: `zbag_app_tauri_lib::commands::wallet::zbag_xxx`
- `crates/zbag-core/src/ipc/v1/commands/` - Request/Response types
- `apps/zbag-app-tauri/src/types/ipc.ts` + `apps/zbag-app-tauri/src/services/ipc.ts`

## Toolchain

- Rust 1.92.0 (edition 2024) - `rust-toolchain.toml`
- Bun 1.3.5+
- Tauri v2 CLI via `@tauri-apps/cli`

## Testing

- Unit/integration: `crates/*/tests/*.rs`
- User story tests: `us<N>_*.rs`
- Migration tests: `make test-migrations`

## Coding Conventions

- Rust: `rustfmt`; `thiserror` for libs, `anyhow` at boundaries
- TypeScript: `PascalCase.tsx` components, `useX` hooks
- Commits: `US<N>:`, `docs:`, `chore:`, `fix:`
- Pre-commit: `make pre-commit`

## Changelog & Releases

Uses [git-cliff](https://git-cliff.org/) for automated changelog generation from conventional commits.

```bash
make changelog           # Regenerate CHANGELOG.md from git history
make changelog-unreleased # Preview unreleased changes
```

**Releasing a version:**
1. `make changelog`
2. `git add CHANGELOG.md && git commit -m "chore: update changelog for v0.1.4"`
3. `git tag v0.1.4`
4. `git push && git push origin --tags`

Config: `cliff.toml`

## Zcash Imports (librustzcash 0.21+)

`zcash_protocol::{consensus, memo, value}` | `zip32::ExtendedSpendingKey`

## Done Criteria

Work is not complete until:
1. All tests pass (`make test`)
2. Pre-commit checks pass (`make pre-commit`)
3. Full Tauri build succeeds (`make tauri-build`)

Do not consider a task finished until `make tauri-build` completes without errors.

## Constitution

Before merging wallet/signing/network/persistence:
- Secrets in Rust only (except mnemonic flows)
- Logs redacted (no seeds/keys/memos)
- Transparent funds for shielding only
- Tor fails closed
- IPC versioned; migrations tested

See `.specify/memory/constitution.md`
