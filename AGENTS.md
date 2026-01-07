# Repository Guidelines

## Project Structure & Module Organization

- `crates/`: Rust workspace libraries (`zkore-core`, `zkore-engine`, `zkore-network`, `zkore-keystone`, `zkore-tor`).
- `apps/zkore-app-tauri/`: Tauri desktop shell + React/TypeScript UI (`src/` for UI, `src-tauri/` for the app backend).
- `specs/`: Source-of-truth wallet specifications (start at `specs/001-zkore-desktop-wallet/`).
- `.specify/memory/constitution.md`: Non-negotiable security and product principles.
- `tests/`: Spec-kit scaffolds for future integration/e2e coverage.

## Build, Test, and Development Commands

- Rust toolchain is pinned in `rust-toolchain.toml` (Rust `1.92.0`, includes `rustfmt` + `clippy`).
- Build all Rust crates: `cargo build --workspace`
- Run Rust tests: `cargo test --workspace` (or scope: `cargo test -p zkore-engine`)
- Format + lint: `cargo fmt --all` and `cargo clippy --workspace --all-targets`
- Run UI (web): `cd apps/zkore-app-tauri && bun install && bun run dev`
- Run desktop app: `cd apps/zkore-app-tauri && bun run tauri dev`
- Bundle desktop app: `cd apps/zkore-app-tauri && bun run tauri build`

## Coding Style & Naming Conventions

- Rust: rely on `rustfmt`; prefer `thiserror` for library error types and `anyhow` at application boundaries.
- TypeScript/React: `PascalCase.tsx` components, `useX` hooks, and keep UI-facing types aligned with `crates/zkore-core`.
- Naming pattern: user-story work commonly uses `US<N>:` in commits and `us<N>_*.rs` in tests.

## Testing Guidelines

- Primary executable coverage is in `crates/*/tests/*.rs` (example: `crates/zkore-engine/tests/us4_restore.rs`).
- `tests/e2e/*.spec.ts` and `tests/integration/*.rs` are scaffolds (some are skipped/not wired); keep them in sync with specs, but don’t rely on them yet.

## Commit & Pull Request Guidelines

- Follow existing commit patterns: `US4: ...`, `docs: ...`, `chore: ...` (imperative, concise).
- PRs: link the relevant spec/issue, include a brief test plan (commands run), and add screenshots for UI changes.

## Security & Configuration Tips

- Never log or persist mnemonics/spending keys; keep secrets in the Rust backend and follow `.specify/memory/constitution.md`.
- Dev-only overrides live in `.env.development` (e.g. `ZKORE_GRPC_URL`, `RUST_LOG`); release behavior must not silently depend on environment variables.

## Agent-Specific Notes (Codex)

- Project skills live in `.codex/skills/`; if a task matches a skill, read its `SKILL.md` and follow it.
