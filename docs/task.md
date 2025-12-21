## Project-wide setup tasks

### Repo and workspace

* Create mono-repo with Rust workspace and frontend app folder

  * `crates/zkore-core`
  * `crates/zkore-engine`
  * `crates/zkore-network`
  * `crates/zkore-keystone`
  * `apps/zkore-app-tauri/src-tauri`
  * `apps/zkore-ui`
* Configure Rust edition 2024 in workspace Cargo.toml

  * Set `edition = "2024"` and `rust-version = "1.92"` in `[workspace.package]`
  * **Note**: Edition 2024 requires explicit `unsafe` blocks inside `unsafe fn`
  * **Note**: Avoid using `gen` as identifier (reserved keyword)
  * See `specs/001-zkore-desktop-wallet/quickstart.md` for API migration notes
* Add consistent formatting and linting

  * Rust: `rustfmt`, `clippy`, `deny(warnings)` in CI for critical crates
  * TS: `eslint`, `prettier`, `tsc --noEmit` in CI
* Define feature flags and build profiles

  * Rust cargo features: `orchard_only`, `pczt`, `tor`, `mainnet`, `testnet`
  * Ensure sapling spend and transparent spend code paths are not compiled or are unreachable behind a hard gate
* Update directory structure to include network-specific paths

  * `~/.zkore/wallets/mainnet/{wallet-id}/`
  * `~/.zkore/wallets/testnet/{wallet-id}/`

### librustzcash version alignment

* Use zcash_client_backend 0.21+, zcash_client_sqlite 0.19+, zcash_primitives 0.26+, zcash_protocol 0.7+
* Add `zip32 = "0.2"` as separate dependency (module relocated from zcash_primitives)
* Use `zcash_protocol::consensus` instead of `zcash_primitives::consensus`
* Use `zcash_protocol::memo` instead of `zcash_primitives::memo`
* Use `Zatoshis` type from `zcash_protocol::value` for amounts (not raw `u64`)
* Handle `TransparentAddressMetadata` as enum (not struct) with `.scope()` returning `Option`

### Typed IPC and eventing

* Define versioned IPC types in `zkore-core`

  * `ipc/v1/commands/*` request and response structs
  * `ipc/v1/events/*` payload structs
  * Add `schema_version: u32` field in every top-level payload
* Implement IPC codec rules

  * Strict deserialization (reject unknown fields) for command requests
  * Explicit mapping between internal domain types and IPC types
* Implement a backend EventBus

  * Publish events by topic string (sync, balance, tx, swap, tor)
  * Provide subscribe/unsubscribe for Tauri windows
  * Add buffering strategy (last-known state per category) so new windows can hydrate instantly

### Storage baseline

* Wallet DB integration

  * Initialize `zcash_client_sqlite` wallet database location and migration handling
  * Define and implement "wallet directory layout" (one wallet per folder)
* App metadata DB

  * Create a separate SQLite DB for app metadata
  * Add migrations and schema version table
  * Tables to create initially:

    * `app_flags` (first_run_completed, backup_required, backup_completed_at, last_opened_wallet_id)
    * `servers` (name, grpc_url, is_default, last_success_at)
    * `tor_settings` (enabled, last_status, last_error, updated_at)
    * `receive_rotation` (account_id, diversifier_index, created_at) optional
    * `swaps` (swap_id, kind, state, created_at, updated_at, remote_payload_json, last_error) placeholder until Milestone 5

### Logging and crash hygiene

* Implement a redaction layer in backend logging

  * Redact seed phrases, UFVK, PCZT payload bytes, raw memos
  * Enforce a "never log secrets" lint rule via wrapper macros (example: `log_safe!(...)`)
* Add a crash report policy

  * Disable any automatic payload dumps
  * Ensure panic hooks do not include sensitive state

### CI and dev tooling

* Add CI pipeline stages

  * Build Rust workspace (all features toggles relevant to each milestone)
  * Build UI
  * Run unit tests (Rust + TS)
  * Run integration smoke tests with a configurable lightwalletd endpoint
  * **Security audit**: Run `cargo audit` to check for known vulnerabilities
  * **Lock file verification**: Build with `cargo build --locked` to ensure reproducibility
  * **Clippy lints**: Run `cargo clippy -- -D warnings` for all crates
* Add developer scripts

  * `bun run dev` launches Tauri with hot reload
  * `bun run tauri dev` for Tauri development
  * `make testnet` switches configs quickly
* Add configuration system

  * App config file (server URL, network, Tor enabled)
  * Overrides via environment variables for dev and CI

### Toolchain and version pinning

* Create `rust-toolchain.toml` at workspace root

  * Pin to Rust 1.92.0 (minimum for edition 2024)
  * Include components: `rustfmt`, `clippy`
* Commit `Cargo.lock` to version control

  * Required for reproducible builds and security verification
  * Production builds MUST use `--locked` flag
* Install `cargo-audit` in CI

  * Add to CI pipeline for automated vulnerability scanning
  * Block merge on known vulnerabilities in dependencies

---

## Milestone 1 tasks: Wallet foundation and receive-first UX

### Backend: engine and wallet lifecycle

* Implement `WalletManager`

  * Implement network selection at wallet creation (mainnet/testnet choice)
  * Create wallet directory structure with network separation: `~/.zkore/wallets/mainnet/{wallet-id}/` and `~/.zkore/wallets/testnet/{wallet-id}/`
  * Create wallet folder and initialize wallet DB
  * Create first Orchard account
  * Store `backup_required = true` in metadata DB
  * Add Network field to ServerConfig and validate network match
  * Expose `lock_wallet` and `unlock_wallet` interfaces even if first version is no-op (so later adding encryption does not break IPC)
* Implement `AddressService`

  * Implement `get_fresh_shielded_ua(account_id)` returning UA with only Orchard receiver
  * Implement `get_compat_transparent_address(account_id)` returning a transparent address without embedding it in UA
  * Persist rotation index (diversifier index) in metadata DB or wallet DB as appropriate
* Implement `SyncService` basic sync

  * Implement gRPC client for CompactTxStreamer in `zkore-network`
  * Implement initial sync loop:

    * connect to configured server
    * download compact blocks
    * scan and update wallet DB
  * Publish `sync.progress` events periodically
  * Provide `start_sync`, `stop_sync`, `get_sync_state` commands

### Backend: balances and activity

* Implement balance computation

  * Provide `get_balances` returning:

    * orchard spendable
    * orchard pending
    * transparent total
  * Publish `balance.changed` whenever state changes
* Implement initial transaction listing

  * Provide `list_transactions` returning confirmed transactions only
  * Define transaction model for UI:

    * txid, time, mined_height, value_delta, memo_present flag, status
  * Publish `tx.changed` when new confirmed tx detected

### Tauri commands and window plumbing

* Implement Tauri command handlers in `zkore-app-tauri`

  * Wallet create
  * Load wallet
  * Get receive addresses
  * Start sync and get progress
  * Get balances and transactions
* Implement event subscription bridge

  * Main window subscribes to topics
  * Ensure events are sent to correct window labels

### Frontend: core screens

* Implement app shell and navigation

  * Home (status widget placeholder)
  * Receive
  * Activity
  * Settings (server selection basic)
* Implement Receive screen

  * On open, request fresh shielded UA
  * Render QR and copy button
  * Show transparent compatibility address in a secondary section with clear labeling
  * Add "regenerate" action that requests another shielded UA
* Implement Activity screen v1

  * List confirmed transactions
  * Live update via events

### Settings: server configuration

* Implement server list and selection

  * Default: zec.rocks with regional options (na.zec.rocks, eu.zec.rocks, me.zec.rocks, sa.zec.rocks)
  * Allow adding custom endpoint with security warning dialog
  * Persist selection in metadata DB
* Implement connection test action

  * Ping gRPC endpoint and show status
  * Validate URL and check network match
  * Save last success timestamp

### Testing

* Add unit tests

  * Address generation and rotation increments
  * IPC serialization and unknown-field rejection
* Add integration smoke test

  * Create wallet, start sync for a short duration, verify no crash, verify progress events emitted

---

## Milestone 2 tasks: Send, memo, and mandatory shielding rules

### Backend: transaction building and submission

* Implement `TxService.build_send`

  * Validate recipient supports Orchard
  * Reject transparent-only recipients
  * Support optional memo field
  * Select Orchard notes, compute fee, build transaction
  * Return unsigned bytes ready to submit (software wallet path)
* Implement `TxService.submit`

  * Submit via CompactTxStreamer endpoint
  * Insert pending activity entry immediately
  * Publish `tx.changed` for pending and later confirmed transitions
* Implement mandatory shielding rules in backend

  * Ensure transparent balance is visible but cannot be spent as a direct payment source
  * Enforce policy in `TxService` and any spend entrypoint

### Backend: shielding and consolidation

* Implement `TxService.build_shield_all(consolidate: bool)`

  * Collect all transparent UTXOs
  * Create Orchard output to a wallet-owned shielded address
  * Add optional self-send consolidation logic for many small Orchard notes (threshold-based)
* Implement status flag for shielding

  * Publish `balance.changed` and/or a dedicated `wallet.status` update when shielding is in progress

### Frontend: send flow

* Implement Send screen

  * Inputs: recipient, amount, optional memo
  * Validate recipient format and amount formatting
  * Fetch fee estimate and show review step
* Implement confirmation step

  * Show recipient, amount, fee, memo presence
  * Submit and navigate to Activity with pending entry visible
* Implement "Shield now" call-to-action

  * When transparent balance > 0, show a persistent action in Home widget area
  * Provide shield and shield+consolidate options (consolidate can be tucked under an advanced toggle)

### Activity updates

* Extend Activity list to show

  * Pending outgoing sends
  * Shielding transaction entries
  * Confirmed transition updates automatically via events

### Testing

* Unit tests

  * Reject transparent recipient sends
  * Memo presence reflected correctly
  * Shield-all builds only Orchard outputs for the wallet
* Integration test

  * On testnet: build and submit a small send, verify pending entry created

---

## Milestone 3 tasks: Backup gating, restore UX, and spend-before-sync plumbing

### Backend: backup state and enforcement

* Implement seed generation and storage rules

  * Generate BIP-39 mnemonic on wallet creation
  * Store seed encrypted at rest or in OS keychain if available, otherwise store only in memory and require user export immediately (choose one approach and enforce consistently)
* Implement backup gating

  * Metadata: `backup_required`, `backup_completed_at`
  * `TxService` must reject all spend attempts while `backup_required = true`
* Implement backup verification command

  * UI submits selected word indices and user-provided words
  * Backend validates against the mnemonic and flips `backup_required = false`

### Frontend: create and backup flow

* Implement wallet creation wizard

  * Create wallet quickly without waiting for full sync
  * Immediately land user on Home
* Implement persistent backup banner/widget

  * Visible on Home and Send
  * Clicking opens a drawer with backup steps
* Implement backup verification UI

  * Prompt user for N words by index
  * Handle paste and word-by-word entry with autocomplete
  * On success, show confirmation and remove gating banner

### Restore with birthday height

* Implement checkpoint table in backend

  * Curate a static mapping of date ranges to approximate block heights
  * Store as a file or embedded table with versioning so it can be updated later
* Implement restore command

  * Create wallet DB for restored seed
  * Convert chosen date to birthday height and start sync from that height
* Implement restore progress phases

  * SyncService reports phase changes:

    * preparing
    * downloading
    * scanning
    * enhancing
    * catching up
  * Publish detailed `sync.progress` events with phase and heights

### Spend-before-sync modeling

* Implement separate frontier tracking

  * Track `scan_frontier_height` and `wallet_tip_height`
  * Expose both via `sync.progress`
* Implement spendability model in engine

  * Compute spendable Orchard notes based on witnesses available
  * Compute pending Orchard notes separately
* Wire UI to show "available now" vs "still scanning"

  * Update Home and Send screens to reference spendable amount only
  * Add explanation in sync details drawer

### Testing

* Unit tests

  * Backup gating blocks send and shield actions
  * Backup verification success flips state and persists
  * Date to height conversion returns expected heights
* UX test checklist

  * Restore flow shows phase progression and does not freeze UI

---

## Milestone 4 tasks: Keystone watch-only and air-gapped signing

### Backend: watch-only and UFVK

* Implement `KeystoneService.import_ufvk`

  * Validate network and key format
  * Create watch-only account in wallet DB
  * Mark account as hardware-backed so spending routes through signing flow
* Extend account model in `zkore-core`

  * `AccountType`: software, watch_only, hardware_signer
  * Expose capabilities to UI so it can show correct actions

### Backend: PCZT build and finalize

* Implement PCZT construction for sends and shield-all

  * Build unsigned PCZT request payload
  * Ensure no spending keys are present in any payload
* Implement PCZT finalize

  * Validate signed payload
  * Finalize to raw transaction bytes
  * Submit via TxService and record activity

### Frontend: signing window and QR flows

* Implement multi-window flow in Tauri

  * Open full-screen signing window with a dedicated route
  * Ensure window can receive events and call commands
* Implement animated QR presentation

  * Convert unsigned PCZT to frames using Keystone SDK utilities
  * Add slow mode that reduces frame rate
  * Add on-screen checklist showing amount, recipient, fee, memo presence
* Implement animated QR scanning

  * Use webcam via `getUserMedia`
  * Decode frames and reconstruct signed payload
  * Provide fallback instructions if camera permissions fail

### MicroSD file fallback

* Implement export unsigned request

  * File dialog save to user-selected location
  * Use a deterministic file format (binary pczt or a structured container)
  * Do not include device identifiers
* Implement import signed response

  * File dialog open
  * Validate payload and finalize
* Implement UI controls

  * In signing flow, provide "Use file instead of camera" option
  * Provide clear step-by-step instructions

### Privacy and safety hardening

* Ensure signing payloads are never stored unencrypted in logs or crash dumps
* Add payload size limits and parsing timeouts to prevent UI freezing or memory issues

### Testing

* Unit tests

  * UFVK validation rejects wrong network
  * PCZT build contains expected outputs and no keys
* Manual QA script

  * QR display is readable at different window sizes
  * Slow mode works
  * File import/export round-trip works

---

## Milestone 5 tasks: Swaps and cross-chain pay via NEAR Intents 1Click

### Backend: network client and routing hooks

* Implement `zkore-network` HTTP client wrapper for 1Click

  * Quote request and response mapping
  * Deposit details retrieval if separate
  * Status polling endpoint mapping
* Ensure networking is backend-owned

  * UI never calls 1Click directly
  * All requests go through `SwapService`

### Backend: swap state machine and persistence

* Implement `SwapService` core methods

  * `request_quote`
  * `start_swap` (persist Draft then AwaitingDeposit)
  * `record_deposit_tx_hash` optional
  * `poll_status` and map remote statuses into local states
  * `cancel_or_refund` if supported by API flows
* Implement swap records table schema

  * swap_id, kind, input_asset, output_asset, amount_in, amount_out, deposit_address, destination, state, remote_id, created_at, updated_at, last_error
* Implement periodic polling scheduler

  * Timer-based poller in backend
  * Publish `swap.changed` events on transitions
  * Backoff strategy on repeated failures

### Frontend: swap and pay flows

* Implement Swap screens

  * Quote screen: input asset/output asset, amount, destination
  * Review screen: show fees, deadlines, and warnings
  * Deposit screen: show deposit address QR and copy, timer, optional tx hash entry
* Implement Activity integration for swaps

  * Add swap entries in Activity list
  * Live updates via `swap.changed`
* Implement pay flow

  * Exact output mode when supported: recipient gets X token Y
  * Show required ZEC spend and confirmation
  * Create and submit payment transaction, then track swap completion

### Privacy constraints handling

* Implement policy: prefer shielded ZEC funds for swap payments
* If any transparent deposit is required by constraints

  * Generate an intent-specific transparent address and never reuse it
  * Surface a clear privacy warning in UI

### Testing

* Unit tests

  * State mapping from remote statuses to local states
  * Persistence of swap state transitions
* Integration tests

  * Mock 1Click API responses for quote and status polling
  * Verify event emissions and UI renders state changes

---

## Milestone 6 tasks: Tor (Beta), fail-closed networking

### Backend: TorManager core

* Implement `TorManager` with explicit states

  * Off
  * Connecting
  * On
  * Error (with last_error message)
* Implement start/stop and health checks

  * Start embedded Tor client
  * Confirm circuit readiness before reporting On
  * Publish `tor.status` events on state changes

### Backend: transport routing policy

* Implement a transport abstraction in `zkore-network`

  * gRPC transport provider for CompactTxStreamer
  * HTTP transport provider for 1Click and rate APIs
* Enforce fail-closed behavior

  * If Tor enabled and Tor not On:

    * Block all sensitive actions (submit tx, fetch tx details, swap actions, rate lookups if configured)
    * Return actionable error codes for UI
  * Never silently fall back to direct connection

### Frontend: Tor settings and UX

* Implement Settings toggle for Tor

  * Show state and last error
  * Provide retry action
  * Provide disable action
* Add contextual prompts

  * If a blocked action occurs due to Tor not ready, show a clear prompt with next steps

### Testing

* Unit tests

  * Fail-closed logic blocks network calls when Tor enabled but not healthy
* Integration tests

  * Simulate Tor startup failure and verify UI and backend state transitions

---

## Cross-cutting hardening tasks to schedule alongside milestones

### Security and policy enforcement

* Add a single backend policy module that all spend-like actions must call

  * Backup gating
  * No transparent spending
  * Tor fail-closed when enabled
  * Recipient validation for Orchard-only
* Add fuzz or property tests for parsing inputs

  * Address parsing
  * UFVK parsing
  * PCZT payload parsing

### UX quality

* Keyboard-first navigation for seed and forms
* Consistent error codes and user-facing messages
* Empty states for Activity and swaps

### Compatibility matrix

* Add CI config variants

  * One Zaino endpoint and one lightwalletd-compatible endpoint (or mocks if CI cannot reach network)
  * Mainnet config build and testnet config build
* Add a "server diagnostics" page

  * Current server URL
  * Last sync height
  * Last error
  * Tor status
