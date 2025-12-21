# Feature Specification: Zkore Desktop Wallet

**Feature Branch**: `001-zkore-desktop-wallet`
**Status**: Draft
**Input**: Desktop-first shielded Zcash wallet with Orchard-only transactions, Keystone hardware wallet support, NEAR Intents DEX integration, and Tor anonymization

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create New Wallet and Receive Funds (Priority: P1)

A new user downloads Zkore Desktop and creates a wallet. They can immediately receive shielded ZEC without being forced to complete backup first, but the app persistently reminds them to back up until they verify their seed phrase. They can only spend after backup verification.

**Why this priority**: This is the foundational user journey. Without wallet creation, no other functionality is accessible. Fast onboarding reduces friction while backup gating protects users from fund loss.

**Independent Test**: Can be fully tested by creating a wallet, receiving testnet ZEC to the shielded address, verifying backup, and confirming the balance is spendable after backup completion.

**Acceptance Scenarios**:

1. **Given** a user launches Zkore for the first time, **When** they select "Create Wallet", **Then** a new wallet is created in under 60 seconds and a shielded receive address is displayed
2. **Given** a user has created a wallet but not backed up, **When** they view the home screen, **Then** a persistent backup reminder is visible and cannot be dismissed
3. **Given** a user has created a wallet but not backed up, **When** they attempt to send funds, **Then** they are blocked and prompted to complete backup verification first
4. **Given** a user is verifying backup, **When** they correctly re-enter specific seed words as requested, **Then** backup is marked complete and spending is enabled

---

### User Story 2 - Send Shielded Transaction with Memo (Priority: P1)

A user with backed-up wallet and shielded funds sends ZEC to another shielded address with an optional memo. The transaction is constructed using Orchard only.

**Why this priority**: Core wallet functionality. Sending is as essential as receiving for a functional wallet.

**Independent Test**: Can be fully tested by sending testnet ZEC from a funded wallet to a shielded address, with and without memo, and verifying the transaction appears in both sender and recipient activity.

**Acceptance Scenarios**:

1. **Given** a user has shielded funds and completed backup, **When** they enter a valid shielded address and amount, **Then** they can send the transaction successfully
2. **Given** a user is composing a send, **When** they add an optional memo, **Then** the memo is included in the shielded transaction
3. **Given** a transaction is broadcast, **When** it enters the mempool, **Then** it appears as "pending" in Activity and transitions to "confirmed" after mining

---

### User Story 3 - Shield Transparent Funds (Priority: P1)

A user who has received ZEC to a transparent address (for exchange/legacy wallet compatibility) must shield those funds before they can spend them. The wallet provides a one-click "Shield and Consolidate" action.

**Why this priority**: Critical for privacy enforcement. Transparent funds being unspendable directly is a core privacy guarantee from the constitution.

**Independent Test**: Can be fully tested by receiving testnet ZEC to the compatibility transparent address, verifying it shows as "unspendable", clicking "Shield Now", and confirming the funds become shielded and spendable.

**Acceptance Scenarios**:

1. **Given** a user has transparent funds, **When** they view their balance, **Then** transparent funds are displayed separately and marked as "not spendable until shielded"
2. **Given** a user has transparent funds, **When** they attempt to include them in a send, **Then** the wallet prevents this and prompts them to shield first
3. **Given** a user clicks "Shield Now", **When** the shielding transaction completes, **Then** the funds appear as shielded and spendable

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

**Independent Test**: Can be fully tested by initiating a testnet/sandbox swap flow, verifying the quote display, and tracking status updates in Activity.

**Acceptance Scenarios**:

1. **Given** a user initiates "Swap to ZEC", **When** they select source asset and amount, **Then** a quote with fees and deadlines is displayed
2. **Given** a quote is accepted, **When** the user views the deposit QR, **Then** they can pay from an external wallet
3. **Given** a swap is in progress, **When** the user views Activity, **Then** the swap status auto-updates through all states: Awaiting deposit, Pending, Confirming, Completed/Refunded/Failed

---

### User Story 9 - Swap From ZEC via NEAR Intents (Priority: P3)

A user wants to convert shielded ZEC to another cryptocurrency. They select target asset and destination address, review the quote, and execute. The swap uses shielded ZEC by default.

**Why this priority**: Off-ramp functionality is valuable but not core to wallet operations.

**Independent Test**: Can be fully tested by initiating an off-ramp flow with testnet ZEC, verifying the shielded spend, and tracking completion.

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

### Edge Cases

- What happens when a user enters an invalid or checksum-failed seed phrase during restore?
  - The wallet must validate the seed phrase and display a clear error before proceeding
- How does the system handle network disconnection during a transaction broadcast?
  - The transaction is saved locally, and the user is prompted to retry when connectivity resumes
- What happens when Keystone QR scanning fails repeatedly?
  - The wallet offers "slow QR mode" with frame rate/brightness tips and microSD fallback
- How does the wallet handle if a swap deadline expires?
  - The swap transitions to "Refunded" or "Failed" state with clear explanation and any refund outcome surfaced
- What happens when transparent funds are received while Tor is enabled?
  - The funds are received and displayed as unspendable until shielded; Tor applies to network operations, not on-chain receiving
- How does the system handle insufficient shielded balance for shielding fee?
  - The wallet explains that shielding requires a fee and suggests acquiring minimal additional ZEC

## Requirements *(mandatory)*

### Functional Requirements

**Wallet Creation & Restoration**
- **FR-001**: System MUST create a new wallet with seed phrase generation in under 60 seconds
- **FR-002**: System MUST allow receiving funds before backup is completed
- **FR-003**: System MUST display a persistent, undismissable backup reminder until backup is verified
- **FR-004**: System MUST block all spending until backup verification is complete (re-entering specific seed words)
- **FR-005**: System MUST support wallet restoration from BIP-39 seed phrase with word autocomplete and paste support
- **FR-006**: System MUST accept an optional approximate first-transaction date during restore to reduce scan time
- **FR-007**: System MUST display distinct restore phases, progress percentage, and estimated time remaining
- **FR-008**: System MUST support spend-before-sync for funds discovered during an ongoing restore

**Shielded Transactions (Orchard Only)**
- **FR-009**: System MUST construct all spends using only Orchard shielded funds
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

**NEAR Intents (Swaps and Pay)**
- **FR-029**: System MUST support "Swap to ZEC" flow with source asset selection, quote review, and deposit QR
- **FR-030**: System MUST support "Swap from ZEC" flow with target asset and destination address input
- **FR-031**: System MUST use shielded ZEC by default for all swap operations
- **FR-032**: System MUST use ephemeral (non-reused) transparent addresses for any unavoidable transparent interactions
- **FR-033**: System MUST display swap/pay entries in Activity with real-time status updates
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

### Key Entities

- **Wallet**: The primary entity containing seed-derived keys, accounts, addresses, and transaction history. Can be spend-capable (full keys) or watch-only (viewing key only)
- **Account**: A logical grouping within a wallet, supporting Orchard shielded pool. Each account has derived addresses and maintains balance state
- **Address**: Either a shielded-only Unified Address (default), a full Unified Address with transparent receiver (not default), or a standalone transparent address (compatibility). Addresses rotate on receive screen access
- **Transaction**: An Orchard shielded transaction with sender, recipient, amount, optional memo, and lifecycle state (pending/confirmed)
- **Transparent UTXO**: A transparent fund that has been received but is not spendable until shielded. Tracked separately from spendable balance
- **Swap Intent**: A NEAR Intents operation with source/target assets, amounts, deadlines, state machine, and lifecycle tracking
- **Backup Status**: A durable flag tracking whether the user has verified their seed phrase backup
- **Tor State**: Current state of Tor connection: Off, Connecting, On, or Error

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can create a new wallet and see a receive address in under 60 seconds
- **SC-002**: Users can complete wallet restoration with known transaction history in under 10 minutes for typical wallet sizes
- **SC-003**: 95% of shielded transactions confirm within the expected block time after broadcast
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

- Users have internet connectivity for wallet operations (sync, send, swap)
- Keystone hardware wallet firmware supports Zcash Orchard and PCZT signing protocol
- NEAR Intents API is available and provides the required swap functionality
- CompactTxStreamer-compatible light client server is available for sync operations
- Tor integration uses embedded Arti-based client from zcash_client_backend
- User devices have sufficient storage for wallet database (estimated under 1GB for typical usage)
- Webcam access is available for QR scanning (with microSD fallback if not)

## Dependencies

- librustzcash ecosystem (zcash_client_backend, zcash_client_sqlite) for wallet engine
- CompactTxStreamer gRPC protocol for light client sync
- Keystone SDK for hardware wallet integration
- NEAR Intents 1Click API for swap operations
- Arti-based Tor client for anonymized networking
