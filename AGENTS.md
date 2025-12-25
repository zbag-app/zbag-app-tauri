# Repository Guidelines

## Project Structure & Module Organization
- This repo is spec-first. The canonical docs live in `README.md`, `CLAUDE.md`, `.specify/memory/constitution.md`, and `specs/`.
- Feature scope is under `specs/001-zkore-desktop-wallet/`, with `spec.md`, `plan.md`, `data-model.md`, `research.md`, `tasks.md`, and `quickstart.md`. IPC contracts live in `specs/001-zkore-desktop-wallet/contracts/ipc-v1.ts`.
- If implementation is added, follow the layout described in `specs/001-zkore-desktop-wallet/quickstart.md` (e.g., `crates/`, `apps/zkore-app-tauri/`, `tests/`).

## Build, Test, and Development Commands
- This repo contains specs only; no build is required here.
- `bun install` installs frontend deps in `apps/zkore-app-tauri/`.
- `bun tauri dev` runs the Tauri dev shell.
- `bun tauri build` builds the production app.
- `cargo test --workspace` runs Rust tests.
- `bun test` and `bun test:a11y` run frontend and a11y tests.

## Coding Style & Naming Conventions
- Docs: keep headings consistent and update `specs/`, `README.md`, and `.specify/memory/constitution.md` when scope changes.
- Rust/TS: follow standard formatting; run `rustfmt` and `cargo clippy --workspace` for Rust.
- Naming patterns: use `zkore-*` for crates and `zkore-app-tauri` for the app; keep new feature specs under `specs/00x-*/`.

## Testing Guidelines
- Planned layout: `tests/integration/` for cross-crate tests and `tests/e2e/` for Tauri end-to-end.
- If a change affects the IPC contract, update `specs/001-zkore-desktop-wallet/contracts/ipc-v1.ts` and add or adjust tests where applicable.

## Commit & Pull Request Guidelines
- Commit messages in history use short, imperative summaries and often a scope prefix (e.g., `docs: add implementation plan`). Follow that pattern when applicable.
- PRs should describe the spec sections changed, link the related issue/task, and note validation (e.g., `cargo test --workspace` or `N/A` for docs-only).

## Security & Configuration Tips
- Follow the Constitution in `.specify/memory/constitution.md`, especially for private disclosure.
- For local config in implementation phases, use `.env.development` with `ZKORE_GRPC_URL`, `ZKORE_NETWORK`, and `RUST_LOG` (see Quickstart).
- Production builds should use `cargo build --release --locked` and commit `Cargo.lock`.
