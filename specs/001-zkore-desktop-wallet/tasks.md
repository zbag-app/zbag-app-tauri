# Tasks: Zkore Desktop Wallet

**Input**: Design documents from `/specs/001-zkore-desktop-wallet/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ipc-v1.ts

**Tests**: Tests are NOT explicitly requested - tasks focus on implementation only.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Rust backend**: `crates/zkore-{core,engine,network,keystone,tor}/src/`
- **Tauri app**: `apps/zkore-app-tauri/src-tauri/src/` (Rust), `apps/zkore-app-tauri/src/` (React)
- **Integration tests**: `tests/integration/`
- **E2E tests**: `tests/e2e/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, workspace structure, and toolchain configuration

- [ ] T001 Create Cargo.toml workspace manifest at repository root with workspace members and dependencies per quickstart.md
- [ ] T002 [P] Create crates/zkore-core/Cargo.toml with workspace package inheritance
- [ ] T003 [P] Create crates/zkore-engine/Cargo.toml with zcash_client_backend and zcash_client_sqlite dependencies
- [ ] T004 [P] Create crates/zkore-network/Cargo.toml with tonic and reqwest dependencies
- [ ] T005 [P] Create crates/zkore-keystone/Cargo.toml with pczt feature dependencies
- [ ] T006 [P] Create crates/zkore-tor/Cargo.toml with tor feature dependencies
- [ ] T007 Create apps/zkore-app-tauri directory structure using bun create tauri-app template (React TypeScript)
- [ ] T008 Configure apps/zkore-app-tauri/src-tauri/Cargo.toml to reference workspace crates
- [ ] T009 [P] Create rust-toolchain.toml pinning Rust 1.92.0 with rustfmt and clippy components
- [ ] T010 [P] Create .env.development with ZKORE_GRPC_URL and RUST_LOG configuration
- [ ] T011 [P] Install frontend dependencies: @keystonehq/animated-qr, @keystonehq/keystone-sdk, @radix-ui/*, @tanstack/react-query, react-hotkeys-hook
- [ ] T012 [P] Configure apps/zkore-app-tauri/src-tauri/tauri.conf.json per quickstart.md
- [ ] T013 Create tests/integration/ and tests/e2e/ directory structure

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**CRITICAL**: No user story work can begin until this phase is complete

### 2.1: Domain Types (zkore-core)

- [ ] T014 Create crates/zkore-core/src/lib.rs with module exports for domain, ipc, and errors
- [ ] T015 [P] Create crates/zkore-core/src/domain/mod.rs with submodule exports
- [ ] T016 [P] Create crates/zkore-core/src/domain/wallet.rs with Wallet, WalletType, Network, WalletInfo structs
- [ ] T017 [P] Create crates/zkore-core/src/domain/account.rs with Account, AccountType, AccountInfo structs
- [ ] T018 [P] Create crates/zkore-core/src/domain/address.rs with Address, AddressType, AddressInfo structs
- [ ] T019 [P] Create crates/zkore-core/src/domain/transaction.rs with Transaction, TransactionType, TransactionStatus, TransactionInfo structs
- [ ] T020 [P] Create crates/zkore-core/src/domain/balance.rs with Balance struct (orchard_spendable, orchard_pending, transparent_total, total)
- [ ] T021 [P] Create crates/zkore-core/src/domain/sync.rs with SyncProgress and SyncPhase types
- [ ] T022 [P] Create crates/zkore-core/src/domain/backup.rs with BackupStatus and BackupAction types
- [ ] T023 [P] Create crates/zkore-core/src/domain/transparent_utxo.rs with TransparentUTXO struct
- [ ] T024 [P] Create crates/zkore-core/src/domain/server.rs with ServerConfig and ServerInfo structs
- [ ] T025 Create crates/zkore-core/src/errors.rs with stable error codes matching ErrorCodes in ipc-v1.ts

### 2.2: IPC Contracts (zkore-core)

- [ ] T026 Create crates/zkore-core/src/ipc/mod.rs with version modules
- [ ] T027 Create crates/zkore-core/src/ipc/v1/mod.rs with command and event submodules
- [ ] T028 [P] Create crates/zkore-core/src/ipc/v1/common.rs with SCHEMA_VERSION, VersionedPayload, IpcError, IpcResult
- [ ] T029 [P] Create crates/zkore-core/src/ipc/v1/commands/wallet.rs with CreateWallet, LoadWallet, ListWallets request/response types
- [ ] T030 [P] Create crates/zkore-core/src/ipc/v1/commands/address.rs with GetReceiveAddress request/response types
- [ ] T031 [P] Create crates/zkore-core/src/ipc/v1/commands/sync.rs with StartSync, StopSync, GetSyncProgress request/response types
- [ ] T032 [P] Create crates/zkore-core/src/ipc/v1/commands/balance.rs with GetBalance request/response types
- [ ] T033 [P] Create crates/zkore-core/src/ipc/v1/commands/transaction.rs with ListTransactions, PrepareSend, ConfirmSend, CancelSend, ShieldFunds request/response types
- [ ] T034 [P] Create crates/zkore-core/src/ipc/v1/commands/backup.rs with VerifyBackup, RestoreWallet request/response types
- [ ] T035 [P] Create crates/zkore-core/src/ipc/v1/events/mod.rs with SyncProgressEvent, BalanceChangedEvent, TransactionChangedEvent, WalletStatusEvent (re-export event structs)
- [ ] T035a [P] Create crates/zkore-core/src/ipc/v1/commands/keystone.rs with ImportUfvk, BuildSigningRequest, FinalizeSigning request/response types
- [ ] T035b [P] Create crates/zkore-core/src/ipc/v1/commands/server.rs with AddServer, SetDefaultServer, TestServer, ListServers request/response types (update commands/mod.rs re-exports)

### 2.3: App Metadata Database

- [ ] T036 Create crates/zkore-engine/src/db/mod.rs with app metadata database module structure
- [ ] T037 Create crates/zkore-engine/src/db/schema.rs with SQLite table definitions per data-model.md (wallets, backup_status, servers, tor_settings, swaps, receive_rotation, _app_migrations)
- [ ] T038 Create crates/zkore-engine/src/db/migrations.rs with migration runner and version tracking
- [ ] T039 Create crates/zkore-engine/src/db/wallet_meta.rs with CRUD operations for wallet metadata table
- [ ] T040 Create crates/zkore-engine/src/db/backup_meta.rs with CRUD operations for backup_status table
- [ ] T041 Create crates/zkore-engine/src/db/server_meta.rs with CRUD operations for servers table

### 2.4: Wallet Engine Foundation

- [ ] T042 Create crates/zkore-engine/src/lib.rs with module exports
- [ ] T043 Create crates/zkore-engine/src/wallet_manager.rs with WalletManager struct skeleton (create, load, list, lock/unlock)
- [ ] T044 Create crates/zkore-engine/src/key_store.rs with KeyStore trait for mnemonic storage abstraction (OS keychain, encrypted file, memory-only)
- [ ] T045 Create crates/zkore-engine/src/birthday.rs with birthday height estimation from date (static checkpoint table per research.md)

### 2.5: Network Foundation

- [ ] T046 Create crates/zkore-network/src/lib.rs with module exports
- [ ] T047 Create crates/zkore-network/src/transport.rs with Transport trait abstraction (direct vs Tor)
- [ ] T048 Create crates/zkore-network/src/grpc_client.rs with CompactTxStreamer gRPC client skeleton

### 2.6: Tauri App Shell

- [ ] T049 Create apps/zkore-app-tauri/src-tauri/src/main.rs with Tauri app setup, state management, and command registration
- [ ] T050 Create apps/zkore-app-tauri/src-tauri/src/state.rs with AppState struct holding WalletManager and service references
- [ ] T051 Create apps/zkore-app-tauri/src-tauri/src/commands/mod.rs with command module structure
- [ ] T052 Create apps/zkore-app-tauri/src-tauri/src/events.rs with event emission helpers for zkore:// channels

### 2.7: Frontend Foundation

- [ ] T053 Copy specs/001-zkore-desktop-wallet/contracts/ipc-v1.ts to apps/zkore-app-tauri/src/types/ipc.ts
- [ ] T054 Create apps/zkore-app-tauri/src/services/ipc.ts with Tauri invoke wrappers per quickstart.md
- [ ] T055 Create apps/zkore-app-tauri/src/services/events.ts with Tauri listen wrappers for event channels
- [ ] T056 Create apps/zkore-app-tauri/src/App.tsx with React Query provider and router setup
- [ ] T057 Create apps/zkore-app-tauri/src/main.tsx with React entry point
- [ ] T058 [P] Create apps/zkore-app-tauri/src/hooks/useFocusTrap.ts for modal focus management
- [ ] T059 [P] Create apps/zkore-app-tauri/src/hooks/useKeyboardShortcuts.ts for global keyboard shortcuts

### 2.8: Logging Infrastructure

- [ ] T060 Create crates/zkore-engine/src/logging.rs with tracing + tracing-appender setup per research.md (daily rotation, 7 days retained, ~/.zkore/logs/)
- [ ] T061 Create crates/zkore-core/src/ipc/v1/commands/logs.rs with GetLogLocation request/response types

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - Create New Wallet and Receive Funds (Priority: P1)

**Goal**: A new user creates a wallet, sees receive address, and can receive funds. Backup reminder persists until verified. Spending blocked until backup complete.

**Independent Test**: Create wallet, receive testnet ZEC to shielded address, verify backup, confirm balance is spendable after backup completion.

### Implementation for User Story 1

- [ ] T062 [US1] Implement mnemonic generation in crates/zkore-engine/src/wallet_manager.rs using bip39 crate (24-word English wordlist)
- [ ] T063 [US1] Implement wallet directory creation with network separation (~/.zkore/wallets/{network}/{wallet-id}/) in crates/zkore-engine/src/wallet_manager.rs
- [ ] T064 [US1] Implement zcash_client_sqlite WalletDb initialization in crates/zkore-engine/src/wallet_manager.rs
- [ ] T065 [US1] Implement UFVK derivation from mnemonic and account insertion in crates/zkore-engine/src/wallet_manager.rs
- [ ] T066 [US1] Implement mnemonic storage via KeyStore trait in crates/zkore-engine/src/wallet_manager.rs
- [ ] T067 [US1] Implement backup_challenge_indices generation (4 random word indices) in crates/zkore-engine/src/wallet_manager.rs
- [ ] T068 [US1] Implement CreateWallet Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/wallet.rs
- [ ] T069 [P] [US1] Create apps/zkore-app-tauri/src/pages/CreateWallet.tsx with network selection (Mainnet/Testnet), wallet name input
- [ ] T070 [P] [US1] Create apps/zkore-app-tauri/src/pages/SeedDisplay.tsx showing 24 seed words with copy protection, continue to backup challenge
- [ ] T071 [US1] Create crates/zkore-engine/src/address_service.rs with get_fresh_shielded_ua() using diversifier rotation
- [ ] T072 [US1] Implement diversifier index tracking in crates/zkore-engine/src/db/rotation_meta.rs (receive_rotation table)
- [ ] T073 [US1] Implement GetReceiveAddress Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/address.rs
- [ ] T074 [P] [US1] Create apps/zkore-app-tauri/src/pages/Receive.tsx with shielded address display, QR code (qrcode.react), one-click copy
- [ ] T075 [US1] Implement backup verification word challenge in crates/zkore-engine/src/wallet_manager.rs (verify 4 specific word indices)
- [ ] T076 [US1] Implement VerifyBackup Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/backup.rs
- [ ] T077 [US1] Create apps/zkore-app-tauri/src/pages/BackupChallenge.tsx with word index challenges and verification
- [ ] T078 [US1] Create crates/zkore-engine/src/sync_service.rs with sync_wallet() skeleton using CompactTxStreamer
- [ ] T079 [US1] Implement StartSync Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/sync.rs
- [ ] T080 [US1] Implement SyncProgress event emission to zkore://sync channel in crates/zkore-engine/src/sync_service.rs
- [ ] T081 [US1] Implement balance computation from zcash_client_sqlite in crates/zkore-engine/src/balance.rs
- [ ] T082 [US1] Implement GetBalance Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/balance.rs
- [ ] T083 [US1] Create apps/zkore-app-tauri/src/pages/Home.tsx with balance display, sync progress, and persistent backup reminder (undismissable if backup_required)
- [ ] T084 [US1] Create apps/zkore-app-tauri/src/components/common/BackupReminder.tsx showing backup status and action button
- [ ] T085 [US1] Implement backup_required check blocking send UI in apps/zkore-app-tauri/src/pages/Home.tsx

**Checkpoint**: User Story 1 complete - wallet creation, receiving, and backup verification functional

---

## Phase 4: User Story 2 - Send Shielded Transaction with Memo (Priority: P1)

**Goal**: User with backed-up wallet sends ZEC to another shielded address with optional memo using Orchard pool only.

**Independent Test**: Send testnet ZEC from funded wallet to shielded address with and without memo, verify transaction appears in Activity.

### Implementation for User Story 2

- [ ] T086 [US2] Create crates/zkore-engine/src/tx_service.rs with transaction construction module structure
- [ ] T087 [US2] Implement proposal-based send flow in crates/zkore-engine/src/tx_service.rs: prepare_send() creates proposal, returns proposal_id, summary, fee
- [ ] T088 [US2] Implement proposal storage (in-memory with expiration) in crates/zkore-engine/src/tx_service.rs
- [ ] T089 [US2] Implement confirm_send() in crates/zkore-engine/src/tx_service.rs: sign and broadcast from proposal_id
- [ ] T090 [US2] Implement cancel_send() in crates/zkore-engine/src/tx_service.rs: remove proposal from memory
- [ ] T091 [US2] Implement PrepareSend Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/transaction.rs
- [ ] T092 [US2] Implement ConfirmSend Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/transaction.rs
- [ ] T093 [US2] Implement CancelSend Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/transaction.rs
- [ ] T094 [US2] Implement transaction broadcast via grpc_client in crates/zkore-network/src/grpc_client.rs
- [ ] T095 [US2] Implement backup_required guard in prepare_send() returning BACKUP_REQUIRED error in crates/zkore-engine/src/tx_service.rs
- [ ] T096 [P] [US2] Create apps/zkore-app-tauri/src/pages/Send.tsx with recipient address input, amount input, memo textarea (optional)
- [ ] T097 [P] [US2] Create apps/zkore-app-tauri/src/pages/SendConfirm.tsx showing TransactionSummary (recipient, amount, fee, total_spend, memo_present)
- [ ] T098 [US2] Implement ListTransactions Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/transaction.rs
- [ ] T099 [US2] Create apps/zkore-app-tauri/src/pages/Activity.tsx with transaction list displaying txid, type, value, status, memo_present
- [ ] T100 [US2] Implement TransactionChangedEvent emission on tx state change in crates/zkore-engine/src/tx_service.rs

**Checkpoint**: User Story 2 complete - sending shielded transactions with memo functional

---

## Phase 5: User Story 3 - Shield Transparent Funds (Priority: P1)

**Goal**: User shields transparent funds before spending. Transparent funds marked "not spendable until shielded". One-click Shield Now action.

**Independent Test**: Receive testnet ZEC to transparent address, verify shows as unspendable, click Shield Now, confirm funds become shielded and spendable.

### Implementation for User Story 3

- [ ] T101 [US3] Implement transparent balance tracking in crates/zkore-engine/src/balance.rs (transparent_total from TransparentUTXOs)
- [ ] T102 [US3] Implement shield_funds() in crates/zkore-engine/src/tx_service.rs using transparent-inputs feature with BACKUP_REQUIRED guard
- [ ] T103 [US3] Implement ShieldFunds Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/transaction.rs
- [ ] T104 [US3] Add transparent balance display to apps/zkore-app-tauri/src/pages/Home.tsx with "Needs Shielding" label and Shield Now button
- [ ] T105 [US3] Implement TRANSPARENT_SPEND_BLOCKED error when attempting direct transparent spend in crates/zkore-engine/src/tx_service.rs
- [ ] T106 [US3] Create apps/zkore-app-tauri/src/components/wallet/ShieldPrompt.tsx modal for shielding confirmation and fee display

**Checkpoint**: User Story 3 complete - transparent funds shielding functional

---

## Phase 6: User Story 4 - Restore Wallet from Seed Phrase (Priority: P2)

**Goal**: User restores wallet from seed phrase with optional birthday date to reduce scan time. Progress UI shows distinct phases.

**Independent Test**: Restore testnet wallet with known history, verify progress UI phases, confirm historical transactions discovered.

### Implementation for User Story 4

- [ ] T107 [US4] Implement restore_wallet() in crates/zkore-engine/src/wallet_manager.rs with seed phrase validation and birthday height estimation
- [ ] T108 [US4] Implement birthday height lookup from date in crates/zkore-engine/src/birthday.rs (checkpoint table lookup)
- [ ] T109 [US4] Implement RestoreWallet Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/backup.rs
- [ ] T110 [P] [US4] Create apps/zkore-app-tauri/src/pages/RestoreWallet.tsx with seed phrase textarea, word autocomplete, paste support
- [ ] T111 [P] [US4] Create apps/zkore-app-tauri/src/pages/RestoreBirthday.tsx with optional date picker for first transaction date
- [ ] T112 [US4] Implement SyncPhase transitions (Idle, Preparing, Downloading, Scanning, Enhancing, CatchingUp) in crates/zkore-engine/src/sync_service.rs
- [ ] T113 [US4] Implement eta_seconds calculation in sync progress in crates/zkore-engine/src/sync_service.rs
- [ ] T114 [US4] Create apps/zkore-app-tauri/src/components/wallet/SyncProgressWidget.tsx showing phase name, progress bar, ETA
- [ ] T115 [US4] Implement spend-before-sync balance distinction (orchard_spendable vs orchard_pending) in crates/zkore-engine/src/balance.rs

**Checkpoint**: User Story 4 complete - wallet restoration with progress tracking functional

---

## Phase 7: User Story 5 - Receive to Fresh Shielded Address (Priority: P2)

**Goal**: Each Receive screen open generates fresh shielded-only UA via diversifier rotation. Transparent address available as separate compatibility option.

**Independent Test**: Open Receive screen multiple times, verify different addresses each time, confirm funds to any address arrive in same wallet.

### Implementation for User Story 5

- [ ] T116 [US5] Implement shielded-only UA generation (Orchard receiver only, no transparent) in crates/zkore-engine/src/address_service.rs
- [ ] T117 [US5] Implement transparent address derivation (separate from UA) in crates/zkore-engine/src/address_service.rs
- [ ] T118 [US5] Update GetReceiveAddress to support AddressType parameter in crates/zkore-engine/src/address_service.rs
- [ ] T119 [US5] Add transparent compatibility toggle to apps/zkore-app-tauri/src/pages/Receive.tsx with clear labeling
- [ ] T120 [US5] Create apps/zkore-app-tauri/src/components/wallet/AddressDisplay.tsx with large QR and one-click copy

**Checkpoint**: User Story 5 complete - address rotation and compatibility addresses functional

---

## Phase 8: User Story 6 - Keystone Hardware Wallet Watch-Only (Priority: P2)

**Goal**: Import UFVK from Keystone for watch-only account. Balances visible, spending prompts for Keystone signing.

**Independent Test**: Import UFVK from Keystone, verify balances appear, confirm send attempt prompts signing flow.

### Implementation for User Story 6

- [ ] T121 Create crates/zkore-keystone/src/lib.rs with module structure
- [ ] T122 [US6] Create crates/zkore-keystone/src/ufvk.rs with UFVK parsing and validation
- [ ] T123 [US6] Implement import_ufvk() in crates/zkore-engine/src/wallet_manager.rs creating HardwareSigner account
- [ ] T124 [US6] Implement ImportUfvk Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/keystone.rs
- [ ] T125 [P] [US6] Create apps/zkore-app-tauri/src/pages/ImportKeystone.tsx with UFVK text input and QR scan option
- [ ] T126 [US6] Add watch-only badge to account display in apps/zkore-app-tauri/src/pages/Home.tsx
- [ ] T127 [US6] Implement WATCH_ONLY_CANNOT_SPEND check redirecting to signing flow in apps/zkore-app-tauri/src/pages/Send.tsx

**Checkpoint**: User Story 6 complete - Keystone watch-only import functional

---

## Phase 9: User Story 7 - Keystone Air-Gapped Signing (Priority: P2)

**Goal**: Full air-gapped signing flow: unsigned tx as QR, scan on Keystone, scan signed response back, verify and broadcast.

**Independent Test**: Create transaction from watch-only account, sign on Keystone device, import signed result, broadcast.

### Implementation for User Story 7

- [ ] T128 [US7] Create crates/zkore-keystone/src/pczt.rs with PCZT building helpers using pczt feature
- [ ] T129 [US7] Create crates/zkore-keystone/src/payload.rs with QR frame encoding using @keystonehq/animated-qr compatible format
- [ ] T130 [US7] Implement build_signing_request() in crates/zkore-engine/src/tx_service.rs returning SigningRequest with qr_frames and summary
- [ ] T131 [US7] Implement BuildSigningRequest Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/keystone.rs
- [ ] T132 [US7] Create apps/zkore-app-tauri/src/pages/Signing.tsx full-screen signing window with animated QR display
- [ ] T133 [US7] Create apps/zkore-app-tauri/src/components/signing/AnimatedQRDisplay.tsx using @keystonehq/animated-qr
- [ ] T134 [US7] Create apps/zkore-app-tauri/src/components/signing/QRScanner.tsx for webcam-based animated QR scanning
- [ ] T135 [US7] Implement finalize_signing() in crates/zkore-engine/src/tx_service.rs to complete and broadcast signed PCZT
- [ ] T136 [US7] Implement FinalizeSigningRequest Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/keystone.rs
- [ ] T137 [US7] Create apps/zkore-app-tauri/src/components/signing/SigningVerify.tsx showing recipient, amount, fee, memo_present for confirmation
- [ ] T138 [US7] Implement microSD fallback: file export (.pczt) in crates/zkore-keystone/src/payload.rs
- [ ] T139 [US7] Create apps/zkore-app-tauri/src/components/signing/FileImport.tsx for microSD file import
- [ ] T140 [US7] Implement slow QR mode (3 fps) toggle in apps/zkore-app-tauri/src/components/signing/AnimatedQRDisplay.tsx
- [ ] T141 [US7] Create apps/zkore-app-tauri/src-tauri/src/windows.rs for dedicated signing window management

**Checkpoint**: User Story 7 complete - full Keystone air-gapped signing functional

---

## Phase 10: User Story 8 - Swap To ZEC via NEAR Intents (Priority: P3)

**Goal**: Acquire ZEC from external crypto via NEAR Intents 1Click API. Quote, deposit QR, status tracking in Activity.

**Independent Test**: Initiate testnet/sandbox swap flow, verify quote display, track status updates in Activity.

### Implementation for User Story 8

- [ ] T142 Create crates/zkore-core/src/domain/swap.rs with SwapIntent, SwapType, SwapState, SwapInfo, SwapQuote structs
- [ ] T143 [P] Create crates/zkore-core/src/ipc/v1/commands/swap.rs with RequestSwapQuote, StartSwap, GetSwapStatus, ListSwaps request/response types
- [ ] T144 [P] Create crates/zkore-core/src/ipc/v1/events/swap.rs with SwapChangedEvent
- [ ] T145 Create crates/zkore-network/src/http_client.rs with base HTTP client using reqwest
- [ ] T146 [US8] Create crates/zkore-network/src/near_intents.rs with 1Click API client (GET /v0/quote, POST /v0/deposit/submit, GET /v0/status)
- [ ] T147 [US8] Implement request_swap_quote() in crates/zkore-engine/src/swap_service.rs calling NEAR Intents quote endpoint
- [ ] T148 [US8] Implement start_swap() in crates/zkore-engine/src/swap_service.rs transitioning Draft to AwaitingDeposit
- [ ] T149 [US8] Implement swap status polling in crates/zkore-engine/src/swap_service.rs (5s interval, exponential backoff on error)
- [ ] T150 [US8] Implement status mapping from v0 API statuses to SwapState in crates/zkore-network/src/near_intents.rs
- [ ] T151 [US8] Create crates/zkore-engine/src/db/swap_meta.rs with CRUD operations for swaps table
- [ ] T152 [US8] Implement RequestSwapQuote Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/swap.rs
- [ ] T153 [US8] Implement StartSwap Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/swap.rs
- [ ] T154 [US8] Implement GetSwapStatus Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/swap.rs
- [ ] T155 [US8] Implement ListSwaps Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/swap.rs
- [ ] T156 [P] [US8] Create apps/zkore-app-tauri/src/pages/Swap.tsx with swap type selection, asset selection, amount input
- [ ] T157 [P] [US8] Create apps/zkore-app-tauri/src/pages/SwapQuote.tsx showing quote details, fees, deadline countdown
- [ ] T158 [US8] Create apps/zkore-app-tauri/src/pages/SwapDeposit.tsx with deposit QR code for external wallet payment
- [ ] T159 [US8] Add swap entries to apps/zkore-app-tauri/src/pages/Activity.tsx with real-time status from SwapChangedEvent
- [ ] T160 [US8] Implement SwapChangedEvent emission in crates/zkore-engine/src/swap_service.rs on state transitions
- [ ] T160a [US8] Reject swap requests for Testnet wallets in crates/zkore-engine/src/swap_service.rs with stable error
- [ ] T160b [US8] Disable Swap UI for Testnet wallets with clear explanation in apps/zkore-app-tauri/src/pages/Swap.tsx and apps/zkore-app-tauri/src/pages/SwapFromZec.tsx

**Checkpoint**: User Story 8 complete - Swap to ZEC via NEAR Intents functional

---

## Phase 11: User Story 9 - Swap From ZEC via NEAR Intents (Priority: P3)

**Goal**: Convert shielded ZEC to external crypto. Uses shielded ZEC by default. Privacy tradeoffs explained for transparent interactions.

**Independent Test**: Initiate off-ramp flow with testnet ZEC, verify shielded spend, track completion.

### Implementation for User Story 9

- [ ] T161 [US9] Implement swap_from_zec flow in crates/zkore-engine/src/swap_service.rs using shielded ZEC
- [ ] T162 [US9] Implement ephemeral transparent address generation for unavoidable transparent interactions in crates/zkore-engine/src/swap_service.rs
- [ ] T163 [US9] Create apps/zkore-app-tauri/src/pages/SwapFromZec.tsx with target asset, destination address input
- [ ] T164 [US9] Create apps/zkore-app-tauri/src/components/swap/PrivacyWarning.tsx explaining transparent interaction tradeoffs
- [ ] T165 [US9] Add FromZec validation ensuring shielded ZEC spend in crates/zkore-engine/src/swap_service.rs
- [ ] T165a [US9] Enforce BACKUP_REQUIRED guard for swap_from_zec and pay flows in crates/zkore-engine/src/swap_service.rs

**Checkpoint**: User Story 9 complete - Swap from ZEC via NEAR Intents functional

---

## Phase 12: User Story 10 - Enable Tor Anonymization (Priority: P3)

**Goal**: Opt-in Tor toggle in settings. All network traffic routed through Tor. Fail-closed behavior (no silent fallback).

**Independent Test**: Enable Tor, verify connection status, confirm disabling Tor connectivity causes error not silent fallback.

### Implementation for User Story 10

- [ ] T166 Create crates/zkore-tor/src/lib.rs with module structure
- [ ] T167 Create crates/zkore-core/src/domain/tor.rs with TorState and TorStatus types
- [ ] T168 [P] Create crates/zkore-core/src/ipc/v1/commands/tor.rs with SetTorEnabled, GetTorState request/response types
- [ ] T169 [P] Create crates/zkore-core/src/ipc/v1/events/tor.rs with TorStatusEvent
- [ ] T170 [US10] Create crates/zkore-tor/src/manager.rs with Tor state machine (Off, Connecting, On, Error) using Arti via zcash_client_backend tor feature
- [ ] T171 [US10] Implement circuit establishment with 60s timeout in crates/zkore-tor/src/manager.rs
- [ ] T172 [US10] Implement health check before marking status as On in crates/zkore-tor/src/manager.rs
- [ ] T173 [US10] Implement Tor-aware transport selection in crates/zkore-network/src/transport.rs
- [ ] T174 [US10] Implement fail-closed check in grpc_client blocking requests when Tor enabled but unhealthy in crates/zkore-network/src/grpc_client.rs
- [ ] T174a [US10] Add Tor-aware transport support to crates/zkore-network/src/http_client.rs
- [ ] T174b [US10] Update crates/zkore-network/src/near_intents.rs to use Tor-aware http_client
- [ ] T174c [US10] Enforce fail-closed behavior for HTTP when Tor enabled but unhealthy in crates/zkore-network/src/http_client.rs
- [ ] T175 [US10] Create crates/zkore-engine/src/db/tor_meta.rs with tor_settings table operations
- [ ] T176 [US10] Implement SetTorEnabled Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/tor.rs
- [ ] T177 [US10] Implement GetTorState Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/tor.rs
- [ ] T178 [US10] Implement TorStatusEvent emission on state changes in crates/zkore-tor/src/manager.rs
- [ ] T179 [P] [US10] Create apps/zkore-app-tauri/src/pages/Settings.tsx with Tor toggle, beta label, status indicator
- [ ] T180 [US10] Create apps/zkore-app-tauri/src/components/common/TorStatusBadge.tsx showing Off/Connecting/On/Error
- [ ] T181 [US10] Create apps/zkore-app-tauri/src/components/common/TorErrorDialog.tsx with retry and disable options

**Checkpoint**: User Story 10 complete - Tor anonymization with fail-closed behavior functional

---

## Phase 13: User Story 11 - Wallet Status Widget (Priority: P2)

**Goal**: Home screen widget summarizing wallet state: backup, sync, transparent funds, privacy posture. Actionable buttons for next best action.

**Independent Test**: Place wallet in various states, verify widget displays correct status and actions.

### Implementation for User Story 11

- [ ] T182 Create crates/zkore-core/src/domain/wallet_status.rs with WalletStatus, BackupAction, SyncStatus, ShieldAction, PrivacyPosture types
- [ ] T183 [US11] Implement compute_wallet_status() in crates/zkore-engine/src/wallet_manager.rs aggregating all status fields
- [ ] T184 [US11] Implement GetWalletStatus Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/wallet.rs
- [ ] T185 [US11] Implement WalletStatusEvent emission on any status component change in crates/zkore-engine/src/wallet_manager.rs
- [ ] T186 [US11] Create apps/zkore-app-tauri/src/components/wallet/StatusWidget.tsx with backup, sync, shield, privacy status cards
- [ ] T187 [US11] Implement real-time status updates via WalletStatusEvent subscription in apps/zkore-app-tauri/src/components/wallet/StatusWidget.tsx

**Checkpoint**: User Story 11 complete - wallet status widget functional

---

## Phase 14: User Story 12 - Network Selection (Priority: P2)

**Goal**: Network selection at wallet creation (Mainnet/Testnet). Immutable after creation. Visual indicators distinguish networks.

**Independent Test**: Create wallets on both networks, verify visual distinctions persist, confirm address prefixes differ per network.

### Implementation for User Story 12

- [ ] T188 [US12] Add network field immutability enforcement in crates/zkore-engine/src/wallet_manager.rs
- [ ] T189 [US12] Implement network-aware address prefix validation in crates/zkore-engine/src/address_service.rs
- [ ] T190 [US12] Add network badge/color coding to apps/zkore-app-tauri/src/pages/Home.tsx header
- [ ] T191 [US12] Create apps/zkore-app-tauri/src/components/common/NetworkBadge.tsx with Mainnet (green) and Testnet (orange) styling
- [ ] T192 [US12] Add network display (read-only) to apps/zkore-app-tauri/src/pages/Settings.tsx

**Checkpoint**: User Story 12 complete - network selection and visual distinction functional

---

## Phase 15: Server Configuration (Cross-Story)

**Purpose**: Custom server support with validation and security warnings (supports multiple user stories)

- [ ] T193 Implement server connection test via GetLightdInfo in crates/zkore-network/src/grpc_client.rs
- [ ] T194 Implement server network validation (must match wallet network) in crates/zkore-engine/src/wallet_manager.rs
- [ ] T195 Implement AddServer Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/server.rs
- [ ] T196 Implement SetDefaultServer Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/server.rs
- [ ] T197 Implement TestServer Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/server.rs
- [ ] T198 Implement ListServers Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/server.rs
- [ ] T199 Create apps/zkore-app-tauri/src/pages/ServerSettings.tsx with server list, add custom, set default
- [ ] T200 Create apps/zkore-app-tauri/src/components/settings/ServerSecurityWarning.tsx for custom server warning

---

## Phase 16: Polish and Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

### Accessibility

- [ ] T201 [P] Verify all interactive elements have ARIA labels in apps/zkore-app-tauri/src/
- [ ] T202 [P] Verify Tab order follows logical reading order in all pages
- [ ] T203 [P] Verify visible focus indicators on all focusable elements
- [ ] T204 [P] Implement keyboard shortcuts: Escape (close modals), Enter (confirm), Tab (navigation)

### Error Handling

- [ ] T205 Create apps/zkore-app-tauri/src/components/common/ErrorBoundary.tsx for React error boundary
- [ ] T206 Create apps/zkore-app-tauri/src/components/common/ErrorDialog.tsx for user-friendly error display with stable error codes

### Logging

- [ ] T207 Implement sensitive data redaction (memos, addresses beyond 8 chars) in crates/zkore-engine/src/logging.rs
- [ ] T208 Implement GetLogLocation Tauri command in apps/zkore-app-tauri/src-tauri/src/commands/logs.rs
- [ ] T209 Add log location display to apps/zkore-app-tauri/src/pages/Settings.tsx for support requests

### Security

- [ ] T210 Implement memory zeroization for mnemonic and spending keys using zeroize crate in crates/zkore-engine/src/wallet_manager.rs
- [ ] T211 Remove hardware wallet identifiers from PCZT payloads in crates/zkore-keystone/src/pczt.rs
- [ ] T212 Verify no secrets logged (mnemonic, raw seeds, spending keys) across all crates

### Validation

- [ ] T213 Run cargo clippy --workspace and fix all warnings
- [ ] T214 Run cargo test --workspace to verify all tests pass
- [ ] T215 Verify build with cargo build --release --locked
- [ ] T216 Run quickstart.md setup validation to verify project builds and runs

---

## Dependencies and Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-14)**: All depend on Foundational phase completion
  - P1 stories (US1, US2, US3) should complete before P2/P3 stories
  - Within same priority, stories can proceed in parallel if staffed
- **Server Configuration (Phase 15)**: Can proceed after Phase 2, used by multiple stories
- **Polish (Phase 16)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: No dependencies on other stories - MVP foundation
- **User Story 2 (P1)**: Depends on US1 (wallet creation, backup verification)
- **User Story 3 (P1)**: Depends on US1 (wallet creation, balance display)
- **User Story 4 (P2)**: No dependencies on other stories (restore is alternative to create)
- **User Story 5 (P2)**: Depends on US1 (address generation foundation)
- **User Story 6 (P2)**: No dependencies (watch-only is alternative to software wallet)
- **User Story 7 (P2)**: Depends on US6 (Keystone import)
- **User Story 8 (P3)**: Depends on US1 (wallet, Activity display)
- **User Story 9 (P3)**: Depends on US2 and US8 (sending, swap infrastructure)
- **User Story 10 (P3)**: No dependencies (Tor is independent infrastructure)
- **User Story 11 (P2)**: Depends on US1-US3 (status aggregation needs backup, sync, shielding)
- **User Story 12 (P2)**: Included in US1 (network selection at creation)

### Parallel Opportunities

**Within Phase 2 (Foundational)**:
```
T002, T003, T004, T005, T006 (all crate Cargo.toml files)
T015, T016, T017, T018, T019, T020, T021, T022, T023, T024 (domain types)
T028, T029, T030, T031, T032, T033, T034, T035 (IPC contracts)
T058, T059 (React hooks)
```

**Within User Stories**:
```
US1: T069, T070, T074 (pages can be developed in parallel)
US2: T096, T097 (Send pages)
US4: T110, T111 (Restore pages)
US8: T143, T144, T156, T157 (IPC types and pages)
```

**Across User Stories (after Phase 2)**:
```
US1, US4, US6, US10, US12 can all start in parallel (no inter-story dependencies)
```

---

## Parallel Example: Phase 2 Foundational

```bash
# Launch all domain types in parallel:
Task: "Create crates/zkore-core/src/domain/wallet.rs"
Task: "Create crates/zkore-core/src/domain/account.rs"
Task: "Create crates/zkore-core/src/domain/address.rs"
Task: "Create crates/zkore-core/src/domain/transaction.rs"
Task: "Create crates/zkore-core/src/domain/balance.rs"

# Launch all IPC command types in parallel:
Task: "Create crates/zkore-core/src/ipc/v1/commands/wallet.rs"
Task: "Create crates/zkore-core/src/ipc/v1/commands/address.rs"
Task: "Create crates/zkore-core/src/ipc/v1/commands/sync.rs"
```

---

## Implementation Strategy

### MVP First (User Stories 1-3 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1 (wallet creation, receiving, backup)
4. Complete Phase 4: User Story 2 (sending with memo)
5. Complete Phase 5: User Story 3 (shielding transparent)
6. **STOP and VALIDATE**: Test all three stories independently
7. Deploy/demo if ready - this is a functional MVP

### Incremental Delivery

1. Setup + Foundational -> Foundation ready
2. Add US1 -> Test independently -> Demo (create/receive)
3. Add US2 -> Test independently -> Demo (send)
4. Add US3 -> Test independently -> Demo (shield) - **MVP Complete**
5. Add US4-US5 -> Test -> Demo (restore, address rotation)
6. Add US6-US7 -> Test -> Demo (Keystone support)
7. Add US8-US9 -> Test -> Demo (NEAR Intents swaps)
8. Add US10 -> Test -> Demo (Tor anonymization)
9. Add US11-US12 -> Test -> Demo (status widget, network selection polish)

---

## Notes

- [P] tasks = different files, no dependencies - can run in parallel
- [USn] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Constitution principles enforced: secrets in Rust only, Orchard-only spending, fail-closed Tor, typed IPC
