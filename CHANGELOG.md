# Changelog

All notable changes to zSTASH will be documented in this file.

## [Unreleased]

### Bug Fixes

- Test bridge integration issues

- CI failures for bun tests and playwright timeout

- Prevent e2e-test.sh from deleting user-provided test directories

- Add missing wallet_id to stop_sync cleanup calls in E2E tests

- Validate TEST_BRIDGE_TIMEOUT and fix package.json indentation

- Add logging, CI timeouts, and debugging docs for test bridge

- Apply clippy auto-fixes to test bridge and related code

- Remove unused APIRequestContext import from onboarding.spec.ts

- Share wallet logic in test bridge

- Harden test bridge handlers

- Tighten test-bridge safety checks

- Add async job model for long-running operations

- Validate proposals before async jobs

- Align send job completion

- Gate test-bridge data root fallback

- Improve memo display with structured memos and byte validation

- Implement memo enhancement for received transactions

- Log error when memo enhancement query fails

- Clarify memo_count deduplication semantics and fix CI race condition

- Expand supported swap assets list with provider-driven tokens

- Add error handling to token loader and IPC contract tests

- Add error handling and safe decimals parsing


### Documentation

- Add E2E testing documentation


### Features

- Implement all remaining test bridge commands

- Add E2E tests, CI workflow, and test isolation

- Add zod runtime validation for test bridge IPC responses

- Add concurrency limit to test bridge for runaway test protection

- Add searchable TokenPicker component for swap asset selection


### Miscellaneous

- Update bun.lock and ignore test-results

- Organize job service imports


### Refactoring

- Centralize tauri app startup


### Testing

- Add shield_funds E2E test for test bridge coverage

- Add sync polling workflow E2E test demonstrating documented pattern

## [0.1.10] - 2026-01-22

### Bug Fixes

- Add try/catch to IPC calls in useFiatDisplay to prevent stuck loading state

- Add build timestamp and conditional commit display to About section

- Use git describe format for version display

- Handle thrown IPC errors in fetchRate and improve build timestamp test

- Add retry jitter and reject scientific notation in fiat input

## [0.1.9] - 2026-01-22

### Bug Fixes

- Use AbortController to prevent state updates on unmounted component in useFiatDisplay

## [0.1.8] - 2026-01-22

### Bug Fixes

- Add no-op v2 migration stub for forward compatibility (#117)

- Remove unused tauri_plugin_shell to reduce attack surface (#44)

- Clear restore flow data immediately after seed is persisted

- Extend transaction signing TTL from 5 to 10 minutes (#51)

- Clear restore flow data immediately after seed is persisted (#101)

- Configure SQLite busy_timeout for concurrent operations (#53)

- Avoid DEK copy in hex::encode call (#111)

- Add serde default for ServerInfo and validate empty passwords (#120)

- Remove example.com dependency from Tor health check (#50)

- Improve swap UX with formatted amounts, ZEC input, and upfront privacy warning

- Make tauri-build deterministic

- Add app/affiliate fee (50 bps) to swap quotes and display in UI (#71)

- Add optional fiat value display with privacy warning (#35)

- Use single source of truth for fiat currency symbols

- Remove tilde prefix from fiat value displays

- Increase vite chunk size warning limit to 2000 kB

- Add bidirectional ZEC/fiat input on Send page

- Replace native fiat currency select with custom styled dropdown

- Use saturating_sub in staleness check and clarify force_refresh docs

- Remove unused FIAT_CURRENCY_DISPLAY_NAMES constant


### Other

- Revert "fix: clear restore flow data immediately after seed is persisted"

This reverts commit d884e812e775b43acfac82f7d6859746603f0f52.

## [0.1.1] - 2026-01-18

### Bug Fixes

- Change bundle identifier to avoid macOS .app conflict

- Prevent E9002 crash when typing in backup verification inputs (#55)

- Remove password requirement from logout (#70)


### Documentation

- Add Linux build guide for non-technical users (#61)


### Miscellaneous

- Integrate zSTASH brand assets from zstash-ux (#72)

- Align desktop app design with website brand (#92) (#93)

## [0.1.0] - 2026-01-15

### Documentation

- Add macOS build guide for non-technical users


### Other

- ZSTASH Desktop -> zSTASH

- Update logos

$ cargo tauri icon 20260111_zSTASH_SqLogoPreview.png

## [0.0.5] - 2026-01-09

### Bug Fixes

- Use correct column name in sent_notes memo query

- Update CSP to allow fonts and IPC connections

- Show error dialog when Tor toggle fails

- Apply dark theme to all screens and dialogs

- Enter tokio runtime context for Tor toggle command

- Improve Tor sync handling with silent wait and connection cleanup

- Enter tokio runtime context for Tor startup in setup hook

- Add macOS Info.plist for camera access in QR scanner

- Replace bc-ur-registry-zcash with custom CBOR decoder to fix black screen crash

- Enable Keystone hardware wallet signing with proper PCZT flow

- Complete Keystone hardware wallet signing fixes

- Improve seed phrase backup UX flow during wallet creation

- Add progress bar to Keystone QR scanner for multi-part scanning

- Migrate to new DEFUSE 1Click API format

- Add refund address field to swap flow

- Add quote retry logic and use centralized asset ID constants

- Resolve CI failures for clippy and bun lockfile


### Documentation

- Update README and quickstart for current implementation status

- Update constitution to v2.0.0 and add error codes to data model

- Update AGENTS.md guidance


### Features

- Add wallet picker on app startup

- Add back button to unlock screen

- Improve transaction display and streamline UI

- Redesign UI with Tailwind CSS and component library

- Restyle seed phrase dialog and remove redundant sync status

- Add standalone Keystone hardware wallet support

- Add privacy blur toggle for QR scanner camera


### Miscellaneous

- Use ZEC amounts in send flow

- Enable auto-sync

- Display ZEC amounts instead of zatoshis in UI

- Improve build cache invalidation and suppress chunk warning

- Increase chunk size warning limit to 1.5 MB

- Add UR fountain decode script for PCZT testing


### Other

- Update gitignore

## [0.0.4] - 2026-01-07

### Bug Fixes

- Enable TLS for https lightwalletd

- Make zkore_create_wallet sync to satisfy Tauri lifetime requirements

- Replace deprecated v_tx_sent view with v_tx_outputs in tx retry

- Eliminate critical silent errors (phase 1)

- Add sync error codes and propagation (phase 2)

- Add logging for medium-priority silent errors (phase 3)

- Add logging for low-priority silent errors (phase 4)

- Resolve CheckpointConflict during incremental sync

- Correct GetBlockRange off-by-one in download_blocks_with_retry

- Improve sync performance with larger batches and cache cleanup

- Resolve CI failures (clippy lint + tauri exclusion)

- Handle transport-level timeouts in mempool probe


### Documentation

- Add RPC integration testing runbook

- Add Makefile reference and Tauri command registration guide

- Add sync checkpoint conflict investigation


### Features

- Sync performance improvements - pipelining, retries, timeouts

- Major sync optimizations - connection pooling, reduced RPC calls, smart birthday

- Add zkore-cli for agent-testable wallet operations

- Add wallet logout with re-authentication

- Add zkore-tui crate scaffold

- **cli**: Add --progress-log flag for sync benchmarking

- Fix sync progress percent/ETA

- Smooth sync ETA/progress

- Further stabilize sync ETA

- Update ETA during stalls and benchmark accuracy


### Miscellaneous

- Phase 16 polish and CI gates

- Prevent tokio::spawn panic outside runtime

- Add dev logging for sync failures

- Add integration wallet address tool

- Cargo fmt

- Add working documents to .gitignore and cargo fmt

- Cargo fmt

- Add sync benchmark script

- Ignore local docs and sessions

- Streamline CI for self-hosted runner

- Revert to ubuntu-latest runners

## [0.0.3] - 2026-01-07

### Documentation

- Add repository guidelines


### Features

- Send flow + activity + broadcast retry

- Shield transparent funds

- Implement wallet restore flow

- Receive rotation; US6 UFVK import

- Keystone signing flow

- Fix Tauri white screen from Keystone deps

- Swap to ZEC; US7 Keystone payload

- Swap from ZEC; US8: Confirming->Completed

- Add Tor support and fail-closed networking

## [0.0.2] - 2026-01-07

### Features

- Wallet create/backup/receive plumbing

- Balance events + Settings page


### Other

- Wallet DB encryption, keychain unlock, mempool probe

- Complete US1 milestone tests

## [0.0.1] - 2026-01-07

### Documentation

- Create consolidated constitution v1.0.0

- Add zkore desktop wallet feature specification

- Add implementation plan, data model, and IPC contracts

- Add project README with architecture overview and doc links

- Simplify requirements section, defer to quickstart

- Add network selection, server config, and update tooling

- Align tooling with librustzcash ecosystem

- Update Rust toolchain to 1.92.0, clarify MSRV compatibility

- Adopt proposal-based send flow and clarify mnemonic handling

- Update testnet server config and NEAR Intents API details

- Add scope boundaries, NFRs, and multi-device assumptions

- Add logging infrastructure and accessibility patterns

- Add implementation task list for zkore desktop wallet

- Add repository guidelines

- Add lwd.zec.pro as primary mainnet server

- Harden IPC contract and execution tasks

- Clarify wallet encryption and seed viewing

- Clarify swap asset scope

- Add wallet encryption metadata schema

- Update app metadata database schema and migration tasks

- Move security appendix to plan

- Update default server URLs in data model for clarity

- Clarify wallet_type and account behavior in data model

- Update wallet and account descriptions for clarity

- Add encryption prerequisites and update dependencies for wallet security

- Refine SQLCipher integration for wallet DB encryption

- Update IPC contract serialization tests for enhanced security

- Update user stories and acceptance criteria for shielded transactions

- Enhance README and specifications for shielded transactions

- Update default server URLs to include HTTPS scheme

- Update specifications and documentation for shielded transactions

- Address inconsistencies and clarify specifications for wallet implementation

- Remove outdated files and address specification inconsistencies

- Update project documentation to reflect new structure and remove outdated references

- Clarify the role of the canonical constitution for Zkore Desktop

- Update specifications and IPC contracts for enhanced clarity and compliance

- Introduce comprehensive notes for alignment and specification corrections

- Remove notes.md file and address alignment gaps in specifications

- Update constitution and specifications for CI validation and privacy principles

- Add notes.md with required corrections and recommendations for Zkore specifications

- Remove notes.md and constitution.md to streamline documentation

- Update AGENTS.md and add notes for specification corrections

- Remove notes.md to finalize specification corrections and align documentation

- Update data model and quickstart documentation for wallet handling

- Enhance AGENTS.md with updated frontend logging hygiene guidelines

- Remove AGENTS.md and update specifications for clarity

- Update quickstart and tasks for wallet milestones and traceability

- Update constitution and specifications for privacy policy and versioning

- Update wallet restoration and server configuration documentation

- Enhance data model and specifications for wallet features

- Update tasks and traceability for wallet features and server configurations

- Update testing frameworks and enhance wallet structure documentation

- Enhance wallet data model and specifications

- Enhance wallet data model and quickstart documentation

- Update wallet encryption specifications and quickstart guide

- Clarify privacy tradeoffs and enhance wallet interaction specifications

- Enhance transaction status definitions and account selection in wallet specifications

- Clarify edge-case behaviors and transaction lifecycle in wallet specifications


### Miscellaneous

- Ignore changes.md file

- Scaffold zkore desktop wallet workspace


### Other

- Initial commit from Specify template

- Add initial docs

- Align specs with IPC contract

- Update ignore list and plan template

- Align wallet security model

- Address encryption, migrations, and CI coverage

- Enhance IPC contract tests and logging security

- Resolve pre-implement inconsistencies


