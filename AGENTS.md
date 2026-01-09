# Repository Guidelines

## Project Structure & Module Organization

- `crates/`: Rust workspace libraries (`zkore-core`, `zkore-engine`, `zkore-network`, `zkore-keystone`, `zkore-tor`).
- `apps/zkore-app-tauri/`: Tauri desktop shell + React/TypeScript UI (`src/` for UI, `src-tauri/` for the app backend).
- `apps/zkore-cli/`: Command-line interface.
- `apps/zkore-tui/`: Terminal UI.
- `specs/`: Source-of-truth wallet specifications (start at `specs/001-zkore-desktop-wallet/`).
- `.specify/memory/constitution.md`: Non-negotiable security and product principles.
- `tests/`: Spec-kit scaffolds for future integration/e2e coverage.

## Build, Test, and Development Commands

- Rust toolchain is pinned in `rust-toolchain.toml` (Rust `1.92.0`, includes `rustfmt` + `clippy`).
- Prefer Makefile targets (they exclude the Tauri crate where appropriate): `make build`, `make test`, `make fmt`, `make clippy`, `make pre-commit`, `make dev`.
- Rust (direct): `cargo build --workspace --exclude zkore-app-tauri` and `cargo test --workspace --exclude zkore-app-tauri` (or scope: `cargo test -p zkore-engine`).
- Format + lint (direct): `cargo fmt --all` and `cargo clippy --workspace --all-targets --exclude zkore-app-tauri`.
- Frontend (direct): `cd apps/zkore-app-tauri && bun install && bun run dev`.
- Desktop app (direct): `cd apps/zkore-app-tauri && bun run tauri dev` (bundle: `bun run tauri build`).

## Coding Style & Naming Conventions

- Rust: rely on `rustfmt`; prefer `thiserror` for library error types and `anyhow` at application boundaries.
- TypeScript/React: `PascalCase.tsx` components, `useX` hooks, and keep UI-facing types aligned with `crates/zkore-core`.
- Naming pattern: user-story work commonly uses `US<N>:` in commits and `us<N>_*.rs` in tests.

## Testing Guidelines

- Primary executable coverage is in `crates/*/tests/*.rs` (example: `crates/zkore-engine/tests/us4_restore.rs`).
- `tests/e2e/*.spec.ts` and `tests/integration/*.rs` are scaffolds (some are skipped/not wired); keep them in sync with specs, but don’t rely on them yet.

## Commit & Pull Request Guidelines

- Follow existing commit patterns: `US<N>: ...`, `docs: ...`, `chore: ...`, `fix: ...` (imperative, concise).
- PRs: link the relevant spec/issue, include a brief test plan (commands run), and add screenshots for UI changes.

## Security & Configuration Tips

- Never log or persist mnemonics/spending keys; keep secrets in the Rust backend and follow `.specify/memory/constitution.md`.
- Logs must be redacted (no seeds/keys/memos); Tor must fail closed (no silent downgrade).
- Shielded-by-default; transparent funds/inputs are for shielding only.
- Keep IPC contracts typed/versioned; ensure migrations are tested when touching persistence or IPC versions.
- Dev-only overrides live in `.env.development` (e.g. `ZKORE_GRPC_URL`, `RUST_LOG`); release behavior must not silently depend on environment variables.

## Agent-Specific Notes (Codex)

- Project skills live in `.codex/skills/`; if a task matches a skill, read its `SKILL.md` and follow it.
- For Tauri commands, follow the checklist in `CLAUDE.md` (register in both `apps/zkore-app-tauri/src-tauri/src/lib.rs` and `apps/zkore-app-tauri/src-tauri/src/main.rs`, update IPC types, and update the frontend IPC client/types).
