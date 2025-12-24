# Feature Specification: Zkore Desktop Wallet

**Feature Branch**: `001-zkore-desktop-wallet`
**Status**: Draft
**Input**: Desktop-first shielded Zcash wallet with Orchard-only transactions, Keystone hardware wallet support, NEAR Intents DEX integration, and Tor anonymization

## Clarifications

### Session 2025-12-21

- Q: What features are explicitly out of scope for v1? → A: Mobile apps, multi-currency support, cloud backup/sync
- Q: What observability/telemetry approach should the wallet use? → A: Local logs only, no remote telemetry
- Q: Should wallet support data export beyond seed phrase? → A: Seed phrase only; no transaction/metadata export
- Q: What accessibility requirements are needed for v1? → A: Basic keyboard navigation and screen reader labels
- Q: How should multi-device usage of the same seed be handled? → A: Allowed but unsupported; no sync, user manages conflicts

### Session 2025-12-23

- Q: How should Zkore protect spend-capable secret material at rest on disk (seed/spending keys)? → A: Encrypt with wallet password; optional OS keychain remember
- Q: When should Zkore prompt for the wallet password to unlock a spend-capable wallet? → A: Prompt on app launch (keychain remember optional); require manual password re-auth for every spend (no keychain)
- Q: Can users re-view the seed phrase after the initial wallet creation flow? → A: Yes; "View seed phrase" behind manual wallet-password re-auth (constitution amended)
- Q: How should Zkore store transaction memos on disk? → A: Encrypt memos at rest using the wallet password
- Q: Should the wallet database (transaction history, balances, addresses/notes) be encrypted at rest with the wallet password? → A: Yes; encrypt the entire wallet DB at rest

## Out of Scope

The following features are explicitly excluded from the initial release:

- **Mobile applications**: No iOS or Android versions; desktop-only (macOS, Windows, Linux)
- **Multi-currency support**: ZEC-only wallet balances; swap flows may reference non-ZEC assets for quoting/deposit/payout, but these are not tracked as wallet balances
- **Cloud backup/sync**: No cloud-based seed backup, wallet sync, or cross-device synchronization
- **Transaction history export**: No CSV, JSON, or other export of transaction data; seed phrase is the sole backup mechanism
- **Multi-device synchronization**: No cross-device sync or conflict resolution; protocol-level nullifier rejection handles conflicts
- **BIP-39 passphrase and alternate wordlists**: No “25th word” passphrase support and no non-English BIP-39 wordlists in v1 (24-word English mnemonic only)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create New Wallet and Receive Funds (Priority: P1)

A new user downloads Zkore Desktop and creates a wallet. They can immediately receive shielded ZEC without being forced to complete backup first, but the app persistently reminds them to back up until they verify their seed phrase. They can only spend after backup verification.

**Why this priority**: This is the foundational user journey. Without wallet creation, no other functionality is accessible. Fast onboarding reduces friction while backup gating protects users from fund loss.

**Independent Test**: Can be fully tested by creating a wallet, receiving testnet ZEC to the shielded address, verifying backup, and confirming the balance is spendable after backup completion.

**Acceptance Scenarios**:

1. **Given** a user launches Zkore for the first time, **When** they select "Create Wallet", **Then** a new wallet is created in under 60 seconds and a shielded receive address is displayed
2. **Given** a user has created a wallet but not backed up, **When** they view the home screen, **Then** a persistent backup reminder is visible and cannot be dismissed
3. **Given** a user has created a wallet but not backed up, **When** they attempt to send funds, **Then** they are blocked and prompted to complete backup verification first
4. **Given** a user is verifying backup, **When** they correctly re-enter 4 specific seed words requested by a backend-issued challenge (by 1-based word number), **Then** backup is marked complete and spending is enabled
5. **Given** a user has previously created a wallet, **When** they restart Zkore, **Then** the most recently opened wallet is loaded and they can resume without re-creating a wallet

---

### User Story 2 - Send Shielded Transaction with Memo (Priority: P1)

A user with backed-up wallet and shielded funds sends ZEC to an Orchard-capable recipient address (Unified Address with an Orchard receiver) with an optional memo. The transaction is constructed using Orchard only.

**Why this priority**: Core wallet functionality. Sending is as essential as receiving for a functional wallet.

**Independent Test**: Can be fully tested by sending testnet ZEC from a funded wallet to an Orchard-capable recipient address, with and without memo, and verifying the transaction appears in both sender and recipient activity.

**Acceptance Scenarios**:

1. **Given** a user has shielded funds and completed backup, **When** they enter a valid Orchard-capable recipient address and amount, **Then** they can send the transaction successfully
2. **Given** a user enters a Sapling-only or transparent-only recipient address, **When** they attempt to proceed, **Then** the wallet blocks the send and shows a clear error that the recipient must support Orchard
3. **Given** a user is composing a send, **When** they add an optional memo, **Then** the memo is included in the shielded transaction
4. **Given** a transaction is broadcast, **When** it enters the mempool, **Then** it appears as "pending" in Activity and transitions to "confirmed" after mining

---

### User Story 3 - Shield Transparent Funds (Priority: P1)

A user who has received ZEC to a transparent address (for exchange/legacy wallet compatibility) must shield those funds before they can spend them. The wallet provides a one-click "Shield and Consolidate" action that sweeps all spendable transparent funds into Orchard (fee deducted from transparent inputs), batching into multiple shielding transactions if needed.

**Why this priority**: Critical for privacy enforcement. Transparent funds being unspendable directly is a core privacy guarantee from the constitution.

**Independent Test**: Can be fully tested by receiving multiple testnet transactions to the compatibility transparent address, verifying transparent funds show as "unspendable", clicking "Shield Now", and confirming all spendable transparent funds are shielded (transparent spendable goes to zero) and become spendable.

**Acceptance Scenarios**:

1. **Given** a user has transparent funds, **When** they view their balance, **Then** transparent funds are displayed separately and marked as "not spendable until shielded"
2. **Given** a user has transparent funds, **When** they attempt to include them in a send, **Then** the wallet prevents this and prompts them to shield first
3. **Given** a user clicks "Shield Now", **When** the shielding transaction completes, **Then** the funds appear as shielded and spendable
4. **Given** a wallet has multiple transparent UTXOs, **When** the user shields, **Then** all spendable transparent funds are swept into Orchard (minus fees) and the transparent spendable balance becomes zero
5. **Given** a wallet has too many transparent UTXOs to fit in one transaction, **When** the user shields, **Then** the wallet automatically batches into multiple shielding transactions and shows progress until completion
6. **Given** a wallet’s spendable transparent balance is too small to cover the shielding fee, **When** the user shields, **Then** the wallet fails with a clear error and guidance (including the minimum required amount)

---

### User Story 4 - Restore Wallet from Seed Phrase (Priority: P2)

A returning user restores their wallet using their seed phrase. They can provide an approximate first-transaction date to reduce scan time. The wallet shows clear progress states and supports spend-before-sync for already-discovered funds.

**Why this priority**: Essential for disaster recovery and device migration, but less frequent than daily wallet operations.

**Independent Test**: Can be fully tested by restoring a testnet wallet with known transaction history, verifying the progress UI, and confirming historical transactions are discovered.

**Acceptance Scenarios**:

1. **Given** a user enters a valid seed phrase, **When** they optionally provide an approximate first-transaction date, **Then** the wallet begins scanning from an estimated birthday height
2. **Given** restore is in progress, **When** the user views the home screen, **Then** they see distinct phases, progress percentage, and estimated time remaining
3. **Given** funds are discovered during restore, **When** spend-before-sync is available, **Then** the user can spend discovered funds before full sync completes
4. **Given** seed entry is in progress, **When** the user types seed words, **Then** word autocomplete and paste from clipboard are supported

---

### User Story 5 - Receive to Fresh Shielded Address (Priority: P2)

A user opens the Receive screen to get an address for incoming funds. The default address is a shielded-only Unified Address without transparent receiver. Each time the screen opens, a fresh address is generated (address rotation).

**Why this priority**: Receiving is core functionality, but address rotation and shielded-only defaults are privacy enhancements that build on basic receive capability.

**Independent Test**: Can be fully tested by opening Receive screen multiple times, verifying each shows a different address, and confirming funds sent to any generated address arrive in the same wallet.

**Acceptance Scenarios**:

1. **Given** a user opens the Receive screen, **When** the screen loads, **Then** a fresh shielded-only Unified Address is displayed
2. **Given** a user needs a transparent address for compatibility, **When** they explicitly select the compatibility option, **Then** a transparent address is shown with clear labeling
3. **Given** a user views a receive address, **When** they want to share it, **Then** one-click copy and a large scannable QR code are available

---

### User Story 6 - Keystone Hardware Wallet Watch-Only (Priority: P2)

A user imports a Unified Full Viewing Key from their Keystone hardware wallet. Zkore displays balances and transactions but cannot spend. Spending requires air-gapped signing via the Keystone device.

**Why this priority**: Hardware wallet support significantly increases security for high-value holdings, but is an advanced feature not needed by all users.

**Independent Test**: Can be fully tested by importing a UFVK from Keystone, verifying balances appear, and confirming that attempting to send prompts for Keystone signing.

**Acceptance Scenarios**:

1. **Given** a user initiates Keystone import, **When** they scan or enter the UFVK, **Then** a watch-only account is created and clearly labeled as such
2. **Given** a watch-only account exists, **When** the user views balances, **Then** balances and transaction history are displayed
3. **Given** a watch-only account exists, **When** the user attempts to send, **Then** the wallet initiates the air-gapped signing flow

---

### User Story 7 - Keystone Air-Gapped Signing (Priority: P2)

A user with a Keystone watch-only account initiates a send. Zkore generates an unsigned transaction displayed as a QR code. The user scans this on Keystone, signs, and scans the signed response back into Zkore using webcam or imports via microSD.

**Why this priority**: Completes the Keystone integration for actual spending, essential for hardware wallet security model.

**Independent Test**: Can be fully tested end-to-end with a Keystone device, creating a transaction, signing on device, and importing the signed result.

**Acceptance Scenarios**:

1. **Given** a user creates a send from a watch-only account, **When** the unsigned transaction is ready, **Then** it is displayed as a large QR code in a dedicated full-screen signing window
2. **Given** the signing window is open, **When** the Keystone has signed the transaction, **Then** the user can scan the animated signed QR using their webcam
3. **Given** webcam scanning is unavailable, **When** the user chooses fallback, **Then** they can export/import via microSD file
4. **Given** a signed transaction is imported, **When** displayed for verification, **Then** recipient, amount, fee, and memo presence are shown for user confirmation before broadcast

---

### User Story 8 - Swap To ZEC via NEAR Intents (Priority: P3)

A user wants to acquire ZEC using another cryptocurrency. They select source asset and amount, review the quote, and receive a QR code to pay from an external wallet. The swap status is tracked in Activity.

**Why this priority**: DEX integration expands utility but is not core wallet functionality. Users can acquire ZEC through other means.

**Independent Test**: Can be fully tested on mainnet with small amounts, or using mocked 1Click API responses in integration tests.

**Acceptance Scenarios**:

1. **Given** a user initiates "Swap to ZEC", **When** they select source asset and amount, **Then** a quote with fees and deadlines is displayed
2. **Given** a quote is accepted, **When** the user views the deposit QR, **Then** they can pay from an external wallet
3. **Given** a swap is in progress, **When** the user views Activity, **Then** the swap status auto-updates through all states: Awaiting deposit, Pending, Confirming, Completed/Refunded/Failed

---

### User Story 9 - Swap From ZEC via NEAR Intents (Priority: P3)

A user wants to convert shielded ZEC to another cryptocurrency. They select target asset and destination address, review the quote, and execute. The swap uses shielded ZEC by default.

**Why this priority**: Off-ramp functionality is valuable but not core to wallet operations.

**Independent Test**: Can be fully tested on mainnet with small amounts, or using mocked 1Click API responses in integration tests.

**Acceptance Scenarios**:

1. **Given** a user initiates "Swap from ZEC", **When** they enter target asset and destination address, **Then** estimated ZEC cost and fees are displayed
2. **Given** a swap from ZEC is executed, **When** the transaction is constructed, **Then** it uses shielded ZEC (not transparent)
3. **Given** ephemeral transparent interaction is required, **When** displayed to user, **Then** privacy tradeoffs are clearly explained

---

### User Story 10 - Enable Tor Anonymization (Priority: P3)

A privacy-conscious user enables Tor in settings. All wallet network traffic is routed through Tor. If Tor fails, the wallet does not silently fall back to direct connections (fail-closed).

**Why this priority**: Advanced privacy feature that enhances anonymity but adds complexity and performance overhead.

**Independent Test**: Can be fully tested by enabling Tor, verifying connection status, and confirming that disabling Tor connectivity causes the wallet to error rather than proceed without Tor.

**Acceptance Scenarios**:

1. **Given** a user enables Tor in settings, **When** Tor connects successfully, **Then** the UI indicates Tor status as "On"
2. **Given** Tor is enabled but connection fails, **When** the wallet attempts network operations, **Then** operations fail with clear error (no silent fallback)
3. **Given** Tor connection fails, **When** the user views the error, **Then** they are offered options to retry or disable Tor

---

### User Story 11 - Wallet Status Widget (Priority: P2)

A user views the home screen and sees a status widget summarizing wallet state: backup status, sync progress, transparent funds present, and privacy posture. The widget provides actionable buttons for the next best action.

**Why this priority**: Improves user awareness and guides them toward best practices, but builds on core functionality.

**Independent Test**: Can be fully tested by placing wallet in various states (unbackup, syncing, transparent funds present) and verifying the widget displays correct status and actions.

**Acceptance Scenarios**:

1. **Given** backup is incomplete, **When** user views home screen, **Then** widget prompts "Back up now" with action button
2. **Given** transparent funds exist, **When** user views home screen, **Then** widget prompts "Shield now" with action button
3. **Given** sync is in progress, **When** user views home screen, **Then** widget shows progress and "Continue restore" if applicable
4. **Given** all funds are shielded and backed up, **When** user views home screen, **Then** widget indicates optimal privacy posture

---

### User Story 12 - Network Selection (Priority: P2)

A user creating a new wallet chooses between mainnet and testnet. The network is immutable after wallet creation. Visual indicators (color coding, badges) distinguish networks throughout the UI, and address prefixes differ per network to prevent cross-network mistakes.

**Why this priority**: Network selection is essential for testing and development workflows, but is set once at creation and impacts fewer users than core wallet operations.

**Independent Test**: Can be fully tested by creating wallets on both mainnet and testnet, verifying visual distinctions persist throughout the UI, and confirming that addresses have network-appropriate prefixes.

**Acceptance Scenarios**:

1. **Given** a user initiates wallet creation, **When** they reach the network selection step, **Then** they can choose between mainnet and testnet with clear explanations
2. **Given** a wallet has been created, **When** the user views wallet settings, **Then** the network is displayed but cannot be changed
3. **Given** a user has mainnet and testnet wallets, **When** they view the wallet list or home screen, **Then** visual indicators (color coding, badges) clearly distinguish the networks
4. **Given** a user views a receive address, **When** checking the address prefix, **Then** mainnet and testnet addresses have distinct prefixes preventing cross-network sends

---

### Edge Cases

- What happens when a user enters an invalid or checksum-failed seed phrase during restore?
  - The wallet must validate the seed phrase and display a clear error before proceeding
- How does the system handle network disconnection during a transaction broadcast?
  - The signed transaction is queued in encrypted wallet storage and the user is prompted to retry when connectivity resumes (never auto re-broadcast). Queued entries are deleted after successful broadcast or after 7 days; retry requires explicit user action and manual wallet-password re-authentication.
- What happens when Keystone QR scanning fails repeatedly?
  - The wallet offers "slow QR mode" with frame rate/brightness tips and microSD fallback
- How does the wallet handle if a swap deadline expires?
  - The swap transitions to "Refunded" or "Failed" state with clear explanation and any refund outcome surfaced
- What happens when transparent funds are received while Tor is enabled?
  - The funds are received and displayed as unspendable until shielded; Tor applies to network operations, not on-chain receiving
- How does the system handle insufficient transparent balance to cover the shielding fee?
  - The wallet explains that shielding fees are deducted from transparent inputs; if the total is below the minimum, it shows a clear error (including required-minimum amount) and suggests acquiring minimal additional transparent ZEC
- What happens when there are too many transparent UTXOs to shield in a single transaction?
  - The wallet automatically batches shielding into multiple transactions and shows progress until all spendable transparent UTXOs are shielded

## Requirements *(mandatory)*

### Functional Requirements

**Wallet Creation & Restoration**
- **FR-001**: System MUST create a new wallet with BIP-39 24-word English mnemonic generation in under 60 seconds (no BIP-39 passphrase support in v1)
- **FR-002**: System MUST allow receiving funds before backup is completed
- **FR-003**: System MUST display a persistent, undismissable backup reminder until backup is verified
- **FR-004**: System MUST block all spending until backup verification is complete (re-entering 4 specific seed words requested by a backend-issued challenge)
- **FR-005**: System MUST support wallet restoration from a BIP-39 24-word English mnemonic with word autocomplete and paste support (no BIP-39 passphrase support in v1)
- **FR-006**: System MUST accept an optional approximate first-transaction date during restore to reduce scan time
- **FR-007**: System MUST display distinct restore phases, progress percentage, and estimated time remaining
- **FR-008**: System MUST support spend-before-sync for funds discovered during an ongoing restore
- **FR-008a**: System MUST support reopening an existing wallet after restart (list wallets, load by id, and persist last_opened_at)
- **FR-008b**: System MUST provide a user-initiated "View seed phrase" flow after wallet creation, gated by manual wallet-password re-authentication

**Shielded Transactions (Orchard Only)**
- **FR-009**: System MUST construct all spends using only Orchard shielded funds
- **FR-009a**: System MUST validate that recipients are Orchard-capable (Unified Address with Orchard receiver); Sapling-only or transparent-only addresses MUST be rejected
- **FR-010**: System MUST NOT allow spending transparent funds directly
- **FR-011**: System MUST provide a one-click "Shield and Consolidate" action for transparent funds
- **FR-012**: System MUST support optional memos on shielded sends
- **FR-013**: System MUST display incoming transactions in "pending" state when detected in mempool
- **FR-014**: System MUST transition pending transactions to "confirmed" state after mining

**Address Management**
- **FR-015**: System MUST generate a shielded-only Unified Address (no transparent receiver) as the default receive address
- **FR-016**: System MUST rotate to a fresh shielded address each time the Receive screen is opened
- **FR-017**: System MUST provide a separately accessible transparent address for legacy/exchange compatibility
- **FR-018**: System MUST clearly label the transparent address as a "compatibility" option with explanation of when to use it
- **FR-019**: System MUST provide one-click copy and large QR code for address sharing

**Keystone Hardware Wallet**
- **FR-020**: System MUST support importing Unified Full Viewing Key from Keystone for watch-only accounts
- **FR-021**: System MUST clearly distinguish watch-only accounts from spend-capable accounts in the UI
- **FR-022**: System MUST generate unsigned transactions displayable as large QR codes for Keystone scanning
- **FR-023**: System MUST support scanning animated QR codes from Keystone using webcam
- **FR-024**: System MUST provide microSD import/export as fallback for no-webcam scenarios
- **FR-025**: System MUST provide a dedicated full-screen signing window with step-by-step instructions
- **FR-026**: System MUST display "slow QR mode" option for reliability (frame rate and brightness guidance)
- **FR-027**: System MUST show verification checklist before broadcast: recipient, amount, fee, memo presence
- **FR-028**: System MUST NOT include hardware wallet branding or identifiers in QR payloads or exported files

**NEAR Intents (Swaps)**
- **FR-029**: System MUST support "Swap to ZEC" flow with source asset selection, quote review, and deposit QR
- **FR-030**: System MUST support "Swap from ZEC" flow with target asset and destination address input
- **FR-031**: System MUST use shielded ZEC by default for all swap operations
- **FR-032**: System MUST use ephemeral (non-reused) transparent addresses for any unavoidable transparent interactions
- **FR-033**: System MUST display swap entries in Activity with real-time status updates
- **FR-034**: System MUST support state machine: Draft, Awaiting deposit, Pending, Confirming, Completed, Refunded, Failed
- **FR-035**: System MUST display deadlines and countdown timers for time-sensitive swap actions
- **FR-036**: System MUST clearly communicate privacy tradeoffs for any transparent interactions in swap flows

**Tor Anonymization**
- **FR-037**: System MUST provide opt-in Tor toggle in settings, marked as Beta
- **FR-038**: System MUST display Tor status at all times: Off, Connecting, On, Error
- **FR-039**: System MUST route transaction submission, data fetching, and third-party API calls through Tor when enabled
- **FR-040**: System MUST fail-closed when Tor is enabled but fails (no silent fallback to direct connections)
- **FR-041**: System MUST provide actionable error with options to retry or disable Tor on failure

**Wallet Status Widget**
- **FR-042**: System MUST display status widget on Home screen summarizing wallet state
- **FR-043**: System MUST show backup status and prompt action if incomplete
- **FR-044**: System MUST show sync progress during restore with actionable guidance
- **FR-045**: System MUST show transparent funds presence and prompt "Shield now" action
- **FR-046**: System MUST show shielding-in-progress status when applicable
- **FR-047**: System MUST update status widget in real-time without requiring page refresh

**Network Selection and Custom Servers**
- **FR-048**: System MUST allow network selection (mainnet/testnet) at wallet creation
- **FR-049**: System MUST NOT allow changing network after wallet creation
- **FR-050**: System MUST use separate database directories for mainnet and testnet wallets
- **FR-051**: System MUST visually distinguish mainnet and testnet wallets (color coding, badges)
- **FR-052**: System MUST allow users to configure custom lightwalletd/Zaino server URLs
- **FR-053**: System MUST display security warning when configuring custom servers
- **FR-054**: System MUST test server connection before saving custom server configuration
- **FR-055**: System MUST validate that server network matches wallet network

### Non-Functional Requirements

**Observability**
- **NFR-001**: System MUST write application logs to local filesystem only
- **NFR-002**: System MUST NOT transmit any telemetry, crash reports, or usage data to external servers
- **NFR-003**: System MUST provide user-accessible log location for manual sharing during support requests
- **NFR-004**: System MUST include log rotation to prevent unbounded disk usage

**Accessibility**
- **NFR-005**: System MUST support full keyboard navigation for all primary workflows
- **NFR-006**: System MUST provide appropriate ARIA labels for screen reader compatibility
- **NFR-007**: System MUST maintain visible focus indicators during keyboard navigation
- **NFR-008**: System MUST support standard keyboard shortcuts (Tab, Enter, Escape, arrow keys)

**Security**
- **NFR-009**: System MUST encrypt spend-capable secret material at rest (seed phrase and any spending-capable keys) using a user-defined wallet password
- **NFR-010**: System MUST support optional "remember unlock" via OS keychain; wallet password/unlock material MUST NOT be stored in plaintext on disk
- **NFR-011**: System MUST default to "locked on restart" for spend-capable wallets and prompt for wallet password to unlock on app launch
- **NFR-012**: System MUST require manual wallet-password re-authentication for every spending attempt (send, shield, swap-from-ZEC); OS keychain "remember unlock" MUST NOT satisfy per-spend re-auth
- **NFR-013**: System MUST require manual wallet-password re-authentication to display the seed phrase after creation; OS keychain "remember unlock" MUST NOT satisfy seed-display re-auth
- **NFR-014**: System MUST encrypt transaction memo contents at rest using the wallet password; memo plaintext MUST NOT be written to disk
- **NFR-015**: System MUST encrypt the entire wallet database at rest using the wallet password; wallet transaction history, balances, addresses, and note metadata MUST NOT be readable without a successful unlock
- **NFR-016**: System MUST include a forward migration, a rollback strategy, and automated migration tests for every schema change (app metadata DB and wallet DB)

Implementation-oriented security and persistence guidance is in the plan: [Security & Persistence Design Notes (v1)](plan.md#security--persistence-design-notes-v1).

### Key Entities

- **Wallet**: The primary entity containing seed-derived keys, accounts, addresses, and transaction history. In v1, wallets are software (seed-backed); watch-only is modeled as account types created via UFVK import. Network (mainnet/testnet) is set at creation and immutable thereafter
- **Account**: A logical grouping within a wallet, supporting Orchard shielded pool. Each account has derived addresses and maintains balance state
- **Address**: Either a shielded-only Unified Address (default, Orchard receiver only) or a standalone transparent address (compatibility). Shielded addresses rotate on Receive screen access
- **Transaction**: An Orchard shielded transaction with sender, recipient, amount, optional memo, and lifecycle state (pending/confirmed)
- **Transparent UTXO**: A transparent fund that has been received but is not spendable until shielded. Tracked separately from spendable balance
- **Swap Intent**: A NEAR Intents operation with source/target assets, amounts, deadlines, state machine, and lifecycle tracking
- **Backup Status**: A durable flag tracking whether the user has verified their seed phrase backup
- **Tor State**: Current state of Tor connection: Off, Connecting, On, or Error
- **ServerConfig**: Configuration for lightwalletd/Zaino server including URL and network field. Must match wallet network

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can create a new wallet and see a receive address in under 60 seconds
- **SC-002**: Users can complete wallet restoration with known transaction history in under 10 minutes for typical wallet sizes
- **SC-003**: For 95% of outgoing shielded sends, the wallet receives a successful broadcast acknowledgment from the configured server within 10 seconds of user confirmation (assuming server connectivity)
- **SC-004**: Users can complete the full Keystone signing flow (create transaction, scan to device, sign, scan back, broadcast) in under 3 minutes
- **SC-005**: Wallet status widget correctly reflects current state within 2 seconds of any state change
- **SC-006**: No user can spend funds without completing backup verification (100% enforcement)
- **SC-007**: No transparent funds can be spent directly (100% enforcement of shield-first policy)
- **SC-008**: When Tor is enabled and fails, 100% of network operations fail (zero silent fallback)
- **SC-009**: Swap status updates appear in Activity within 5 seconds of state changes
- **SC-010**: Users can identify and initiate the correct action from the status widget on first attempt 90% of the time
- **SC-011**: Address rotation occurs on every Receive screen open (100% fresh address generation)
- **SC-012**: All Keystone signing flows complete without cable connection (100% air-gapped)

## Assumptions

- Users may restore the same seed on multiple devices; each device operates independently with no synchronization
- Multi-device conflict resolution relies on Zcash protocol-level nullifier rejection; no application-level sync is provided
- Users have internet connectivity for wallet operations (sync, send, swap)
- Keystone hardware wallet firmware supports Zcash Orchard and PCZT signing protocol
- NEAR Intents API is available and provides the required swap functionality
- NEAR Intents 1Click API is mainnet-only; swap features must be disabled for Testnet wallets
- CompactTxStreamer-compatible light client server is available for sync operations
- Tor integration uses embedded Arti-based client from zcash_client_backend
- User devices have sufficient storage for wallet database (estimated under 1GB for typical usage)
- Webcam access is available for QR scanning (with microSD fallback if not)
- Same seed phrase generates different addresses on mainnet vs testnet (due to BIP-44 coin_type)
- Default server is lwd.zec.pro with zec.rocks regional endpoints available

## Dependencies

- librustzcash ecosystem (zcash_client_backend, zcash_client_sqlite) for wallet engine
- CompactTxStreamer gRPC protocol for light client sync
- Zaino (Rust indexer, lightwalletd-compatible) as alternative to lightwalletd
- Keystone SDK for hardware wallet integration
- NEAR Intents 1Click API for swap operations
- Arti-based Tor client for anonymized networking
