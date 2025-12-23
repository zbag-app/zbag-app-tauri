# Data Model: Zkore Desktop Wallet

**Branch**: `001-zkore-desktop-wallet`
**Status**: Complete
**Purpose**: Define entities, relationships, validation rules, and state transitions

## Entity Definitions

### Wallet

The root entity containing seed-derived keys and accounts.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| id | UUID | Unique wallet identifier | Auto-generated |
| name | String | User-defined wallet name | 1-50 chars, non-empty |
| directory_path | String | Filesystem path to wallet data (network-specific: ~/.zkore/wallets/{network}/{wallet-id}/) | Valid path, writable |
| wallet_type | WalletType | Software or WatchOnly | Enum value |
| network | Network | Mainnet or Testnet (IMMUTABLE after creation) | Enum value |
| created_at | Timestamp | Creation timestamp | Auto-set |
| last_opened_at | Timestamp | Last access timestamp | Updated on open |

**WalletType Enum**:
- `Software` - Full spending capability with seed
- `WatchOnly` - View-only from imported UFVK

**Network Enum**:
- `Mainnet` - Production Zcash network (addresses start with u1, zs, t1/t3)
- `Testnet` - Test network (addresses start with utest, ztestsapling, tm)

**Relationships**:
- Has many `Account` (1:N)
- Has one `BackupStatus` (1:1)

**Notes**:
- Seed phrase NEVER stored in app metadata DB
- Network field is IMMUTABLE after wallet creation

### Key Storage Architecture

This section clarifies the separation of viewing keys and spending capability, following `zcash_client_backend`'s design.

**Encrypted wallet DB (zcash_client_sqlite-backed):**
- Unified Full Viewing Keys (UFVKs) per account
- Scanned wallet state (notes, witnesses, transactions)
- Address derivation metadata
- Encrypted at rest with the wallet password; not readable without successful unlock

**Spending capability stored separately:**
- `zcash_client_backend` does NOT store spending keys - they must be supplied when creating transactions
- Spending keys derived on-demand from mnemonic when transaction construction is needed
- Mnemonic storage options (choose one per deployment):
  1. **OS Keychain** (preferred): macOS Keychain, Windows Credential Manager, Linux Secret Service
  2. **Encrypted file**: User-password-protected file in wallet directory
  3. **Memory-only mode**: Mnemonic kept in memory only, user must re-enter on each app launch

**Key derivation flow:**
1. User unlocks wallet (provides password or OS unlocks keychain)
2. Backend retrieves mnemonic from secure store
3. Backend derives spending keys as needed for transaction construction
4. Spending keys held in memory only for duration of operation
5. Spending keys zeroized after use

**Lock/unlock semantics:**
- `locked`: wallet DB not decrypted/open; mnemonic not accessible; spending operations blocked
- `unlocked`: wallet DB decrypted/open; read-only wallet operations allowed, but spending still requires per-action re-authentication
- `reauthenticated`: short-lived, per-action authorization granted after manual wallet-password entry (required for send/shield/swap-from-ZEC and "View seed phrase"; OS keychain must not satisfy)
- WatchOnly wallets still require unlock for the encrypted wallet DB, but have no spending capability

---

### Account

A logical grouping within a wallet for Orchard shielded operations.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| id | u32 | Account index (ZIP-32) | 0-based, sequential |
| wallet_id | UUID | Parent wallet reference | FK to Wallet |
| account_type | AccountType | Spending capability | Enum value |
| name | String | User-defined account name | 1-30 chars |
| diversifier_index | u64 | Next address diversifier | >= 0 |
| created_at | Timestamp | Creation timestamp | Auto-set |

**AccountType Enum**:
- `Software` - Full keys, can spend
- `WatchOnly` - Viewing key only, cannot spend
- `HardwareSigner` - Watch-only with Keystone signing capability

**Relationships**:
- Belongs to `Wallet` (N:1)
- Has many `Address` (1:N, derived)
- Has many `Transaction` (1:N)
- Has one `Balance` (1:1, computed)

**Constraints**:
- Account 0 always exists after wallet creation
- WatchOnly wallets MUST NOT contain Software accounts (no spending keys)
- WatchOnly wallets MAY contain WatchOnly or HardwareSigner accounts

---

### Address

Derived addresses for receiving funds.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| account_id | u32 | Parent account reference | FK to Account |
| diversifier_index | u64 | Address derivation index (use string in IPC for JS safety) | >= 0 |
| address_type | AddressType | Shielded or Transparent | Enum value |
| encoded | String | Bech32m/Base58 encoded address | Valid encoding |
| created_at | Timestamp | Generation timestamp | Auto-set |

**AddressType Enum**:
- `ShieldedOnly` - Unified Address with Orchard receiver only (DEFAULT)
- `Transparent` - Compatibility transparent address (separate)

**Validation Rules**:
- ShieldedOnly addresses MUST NOT include transparent receiver
- Transparent addresses displayed separately with "compatibility" label
- Each Receive screen open generates new diversifier_index

**State Transitions**:
```
[New Request] -> Generated -> Used (received funds)
```

---

### Transaction

An Orchard shielded transaction record.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| txid | TxId | Transaction hash | 32 bytes |
| account_id | u32 | Associated account | FK to Account |
| tx_type | TransactionType | Send, Receive, Shield | Enum value |
| value | Amount | ZEC amount (zatoshis) | Non-negative |
| fee | Amount | Transaction fee | Non-negative |
| memo_present | bool | Whether memo exists | - |
| memo | Option<String> | Decrypted memo content (in-memory only; encrypted at rest) | Max 512 bytes |
| status | TransactionStatus | Lifecycle state | Enum value |
| mined_height | Option<BlockHeight> | Block height if confirmed | > 0 when set |
| created_at | Timestamp | Detection/creation time | Auto-set |
| confirmed_at | Option<Timestamp> | Confirmation time | Set on confirm |

**TransactionType Enum**:
- `Send` - Outgoing shielded payment
- `Receive` - Incoming shielded payment
- `Shield` - Transparent to shielded conversion
- `Consolidate` - Orchard note consolidation

**TransactionStatus Enum**:
- `Pending` - Detected in mempool or just broadcast
- `Confirmed` - Mined in a block
- `Expired` - Expired without confirmation
- `Failed` - Failed to broadcast

**State Transitions**:
```
[Created] -> Pending -> Confirmed
                    \-> Expired
                    \-> Failed
```

**Validation Rules**:
- Only Orchard transactions can be created (no transparent spends)
- Memo redacted in logs (constitution requirement)
- Memo plaintext MUST NOT be written to disk; encryption-at-rest must cover memo contents

---

### TransparentUTXO

A transparent fund that must be shielded before spending.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| outpoint | OutPoint | TxId + output index | Unique |
| account_id | u32 | Associated account | FK to Account |
| value | Amount | ZEC amount (zatoshis) | > 0 |
| address | String | Transparent address | Valid t-addr |
| mined_height | BlockHeight | Block height received | > 0 |
| is_spent | bool | Shielded or not | - |
| shielding_txid | Option<TxId> | If shielded, the shield tx | FK to Transaction |

**Constraints**:
- CANNOT be spent directly per constitution (Principle II)
- MUST be shielded before becoming spendable
- Displayed as "not spendable until shielded" in UI

**State Transitions**:
```
[Received] -> Unspent -> Shielded
```

---

### SwapIntent

A NEAR Intents swap operation.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| id | UUID | Local swap identifier | Auto-generated |
| wallet_id | UUID | Parent wallet reference | FK to Wallet |
| remote_id | Option<String> | 1Click intent ID | From API response |
| swap_type | SwapType | ToZec, FromZec | Enum value |
| input_asset | String | Source asset symbol | Non-empty |
| input_amount | String | Source amount | Decimal string |
| output_asset | String | Target asset symbol | Non-empty |
| output_amount | Option<String> | Target amount if known | Decimal string |
| deposit_address | Option<String> | Where to send deposit | Valid address |
| destination_address | Option<String> | Where to receive output | Valid address |
| refund_address | Option<String> | Refund address if failed | Valid address |
| state | SwapState | Current state | Enum value |
| deadline | Option<Timestamp> | Expiration for deposit | Future time |
| last_error | Option<String> | Error message if failed | - |
| created_at | Timestamp | Creation timestamp | Auto-set |
| updated_at | Timestamp | Last state update | Auto-updated |

**SwapType Enum**:
- `ToZec` - External asset -> Shielded ZEC
- `FromZec` - Shielded ZEC -> External asset

**SwapState Enum**:
- `Draft` - Quote received, not started
- `AwaitingDeposit` - Waiting for user to send funds
- `Pending` - Deposit detected, processing
- `Confirming` - Swap executing, awaiting confirmations
- `Completed` - Successfully completed
- `Refunded` - Failed, funds returned
- `Failed` - Failed, may require action

**State Transitions**:
```
[Quote] -> Draft -> AwaitingDeposit -> Pending -> Confirming -> Completed
                                               \            \-> Refunded
                                                \-> Failed
```

**Privacy Constraints**:
- FromZec swaps MUST use shielded ZEC
- If transparent deposit required, use ephemeral address (never reused)
- Display privacy tradeoff warnings in UI

---

### BackupStatus

Tracks seed phrase backup state per wallet.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| wallet_id | UUID | Associated wallet | FK to Wallet |
| backup_required | bool | Whether backup needed | Default true |
| backup_completed_at | Option<Timestamp> | When verified | Set on verify |
| verification_method | Option<String> | How verified | e.g., "word_challenge" |

**Constraints**:
- All spending blocked while `backup_required = true`
- Cannot be unset once `backup_required = false`

**State Transitions**:
```
[Created] -> Required -> Verified (spending enabled)
```

---

### TorState

Current Tor connection state.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| enabled | bool | User preference | - |
| status | TorStatus | Current state | Enum value |
| last_error | Option<String> | Error if failed | - |
| updated_at | Timestamp | Last state change | Auto-updated |

**TorStatus Enum**:
- `Off` - Tor disabled
- `Connecting` - Establishing circuit
- `On` - Connected and healthy
- `Error` - Failed, requires action

**Fail-Closed Behavior**:
- When `enabled = true` and `status != On`, block sensitive operations
- Never silently fallback to direct connections

---

### ServerConfig

Light client server configuration.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| id | UUID | Server identifier | Auto-generated |
| name | String | Display name | 1-50 chars |
| grpc_url | String | gRPC endpoint URL | Valid URL |
| network | Network | Mainnet or Testnet | Enum value, must match wallet network |
| is_default | bool | Whether selected | Only one default per network |
| last_success_at | Option<Timestamp> | Last successful connection | - |
| created_at | Timestamp | When added | Auto-set |

**Default Servers**:
- Mainnet: https://lwd.zec.pro (default), https://zec.rocks (regional: https://na.zec.rocks, https://eu.zec.rocks, https://sa.zec.rocks)
- Testnet: https://lwd.testnet.zec.pro (default)

**Validation Rules**:
- Server network MUST match wallet network when connecting
- Only servers matching the wallet's network are available for selection

---

### SyncProgress

Current synchronization state (computed, not persisted).

| Field | Type | Description |
|-------|------|-------------|
| phase | SyncPhase | Current sync phase |
| scan_frontier_height | BlockHeight | How far scanned |
| wallet_tip_height | BlockHeight | Server chain tip |
| progress_percent | f32 | Overall progress (0-100) |
| eta_seconds | Option<u64> | Estimated time remaining |

**SyncPhase Enum**:
- `Idle` - Not syncing
- `Preparing` - Initializing sync
- `Downloading` - Fetching compact blocks
- `Scanning` - Decrypting transactions
- `Enhancing` - Fetching transaction details
- `CatchingUp` - Near tip, real-time updates

---

### Balance (Computed)

Wallet balance breakdown (computed, not persisted).

| Field | Type | Description |
|-------|------|-------------|
| account_id | u32 | Account reference |
| orchard_spendable | Amount | Immediately spendable |
| orchard_pending | Amount | Not yet spendable |
| transparent_total | Amount | Must shield to spend |
| total | Amount | All funds |

**Display Rules**:
- `orchard_spendable` shown as "Available"
- `orchard_pending` shown as "Pending" during restore
- `transparent_total` shown as "Needs Shielding" with action button

---

### WalletStatus (Computed)

Aggregated wallet status for status widget.

| Field | Type | Description |
|-------|------|-------------|
| lock_status | LockStatus | Whether wallet DB is locked/unlocked |
| backup_status | BackupAction | Backup state + action |
| sync_status | SyncStatus | Sync state + progress |
| shield_status | ShieldAction | Transparent funds state |
| privacy_posture | PrivacyPosture | Overall privacy state |

**LockStatus Enum**:
- `Locked` - Wallet DB locked; prompt user to unlock
- `Unlocked` - Wallet DB unlocked; normal operations available (spending still requires per-action re-auth)

**BackupAction Enum**:
- `Required` - Must backup, action: "Back up now"
- `Complete` - Backup done

**SyncStatus Enum**:
- `Synced` - Up to date
- `Syncing(progress)` - In progress with percentage
- `Error(message)` - Failed with error

**ShieldAction Enum**:
- `None` - No transparent funds
- `Available(amount)` - Has transparent, action: "Shield now"
- `InProgress` - Shielding transaction pending

**PrivacyPosture Enum**:
- `Optimal` - All shielded, backed up
- `NeedsAction` - Requires shielding or backup

---

## Relationships Diagram

```
Wallet (1) ─────── (*) Account (1) ─────── (*) Transaction
   │                     │
   │                     ├─────── (*) TransparentUTXO
   │                     │
   │                     └─────── (*) Address
   │
   └─── (1) BackupStatus

SwapIntent ─────── (references) ─────── Wallet

ServerConfig ─────── (selected) ─────── Wallet

TorState ─────── (global singleton)
```

## Database Schema Overview

### Wallet DB (zcash_client_sqlite)
- Managed by librustzcash
- Contains: accounts, addresses, transactions, notes, witnesses
- Migrations handled by library
- Encrypted at rest with the wallet password; not readable without successful unlock
- Directory structure: ~/.zkore/wallets/mainnet/{wallet-id}/ and ~/.zkore/wallets/testnet/{wallet-id}/

### App Metadata DB (custom SQLite)
```sql
-- Wallet metadata
CREATE TABLE wallets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    directory_path TEXT NOT NULL,  -- Network-specific path: ~/.zkore/wallets/{network}/{wallet-id}/
    wallet_type TEXT NOT NULL,
    network TEXT NOT NULL,  -- IMMUTABLE after creation
    remember_unlock_enabled INTEGER NOT NULL DEFAULT 0, -- OS keychain-backed auto-unlock (must not satisfy per-action re-auth)
    created_at INTEGER NOT NULL,
    last_opened_at INTEGER
);

-- Wallet encryption metadata (per-wallet)
CREATE TABLE wallet_encryption (
    wallet_id TEXT PRIMARY KEY REFERENCES wallets(id),
    kdf_algorithm TEXT NOT NULL,
    kdf_version INTEGER NOT NULL,
    kdf_memory_mib INTEGER NOT NULL,
    kdf_iterations INTEGER NOT NULL,
    kdf_parallelism INTEGER NOT NULL,
    kdf_salt TEXT NOT NULL,
    wrapped_dek TEXT NOT NULL,
    aead_scheme TEXT NOT NULL,
    aead_version INTEGER NOT NULL,
    aead_nonce TEXT
);

-- Backup state
CREATE TABLE backup_status (
    wallet_id TEXT PRIMARY KEY REFERENCES wallets(id),
    backup_required INTEGER NOT NULL DEFAULT 1,
    backup_completed_at INTEGER,
    verification_method TEXT
);

-- Server configuration
CREATE TABLE servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    grpc_url TEXT NOT NULL,
    network TEXT NOT NULL,  -- Mainnet or Testnet, must match wallet network
    is_default INTEGER NOT NULL DEFAULT 0, -- Default server per network
    last_success_at INTEGER,
    created_at INTEGER NOT NULL
);

-- Ensure one default server per network
CREATE UNIQUE INDEX servers_one_default_per_network
ON servers(network)
WHERE is_default = 1;

-- Tor settings (singleton)
CREATE TABLE tor_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'Off',
    last_error TEXT,
    updated_at INTEGER NOT NULL
);

-- Swap records
CREATE TABLE swaps (
    id TEXT PRIMARY KEY,
    remote_id TEXT,
    wallet_id TEXT NOT NULL REFERENCES wallets(id),
    swap_type TEXT NOT NULL,
    input_asset TEXT NOT NULL,
    input_amount TEXT NOT NULL,
    output_asset TEXT NOT NULL,
    output_amount TEXT,
    deposit_address TEXT,
    destination_address TEXT,
    refund_address TEXT,
    state TEXT NOT NULL DEFAULT 'Draft',
    deadline INTEGER,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Address rotation tracking
CREATE TABLE receive_rotation (
    account_id INTEGER NOT NULL,
    wallet_id TEXT NOT NULL REFERENCES wallets(id),
    diversifier_index INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (wallet_id, account_id)
);

-- Migration tracking
CREATE TABLE _app_migrations (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);
```

## Validation Summary

| Entity | Critical Validations |
|--------|---------------------|
| Wallet | Valid path, enum values |
| Account | Sequential index, type matches wallet |
| Address | Shielded-only default, transparent separate |
| Transaction | Orchard only, memo redacted in logs |
| TransparentUTXO | Cannot be spent directly |
| SwapIntent | Shielded ZEC for FromZec, ephemeral transparent |
| BackupStatus | Blocks spending when required |
| TorState | Fail-closed when enabled but not On |
