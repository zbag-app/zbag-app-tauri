/**
 * zSTASH Desktop Wallet - IPC Contract v1
 *
 * TypeScript type definitions for Tauri IPC commands and events.
 * These types MUST match the Rust definitions in zstash-core/src/ipc/v1/
 *
 * @version 1
 */

// ============================================================================
// Common Types
// ============================================================================

/** Schema version for all IPC payloads */
export const SCHEMA_VERSION = 1;

/** Base payload with version */
export interface VersionedPayload {
  schema_version: number;
}

/** Standard error response */
export interface IpcError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

/** Result type for IPC commands */
export type IpcResult<T> = { ok: T } | { err: IpcError };

/**
 * Unix epoch timestamp in milliseconds (UTC).
 *
 * NOTE: All timestamp fields in this IPC contract use milliseconds unless explicitly stated otherwise.
 */
export type UnixTimestampMs = number;

// ============================================================================
// Wallet Types
// ============================================================================

// In v1, wallets are always `Software`; watch-only behavior is modeled at the account level.
// `WatchOnly` wallet type is reserved for future use and MUST NOT be created in v1.
export type WalletType = 'Software' | 'WatchOnly';
export type Network = 'Mainnet' | 'Testnet';

// In v1, Keystone UFVK import creates `HardwareSigner` accounts (watch-only; spend via signing flow).
// `WatchOnly` account type is reserved for future generic viewing-key accounts and MUST NOT be created in v1.
export type AccountType = 'Software' | 'WatchOnly' | 'HardwareSigner';
export type WalletLockStatus = 'Locked' | 'Unlocked';

export interface WalletInfo {
  id: string;
  name: string;
  wallet_type: WalletType;
  network: Network;
  /**
   * DISABLED: Keychain-based auto-unlock is disabled due to security concerns (issue #45).
   * This field is vestigial and always false. Retained for schema compatibility.
   */
  remember_unlock_enabled: boolean;
  created_at: UnixTimestampMs;
  last_opened_at: UnixTimestampMs | null;
}

export interface AccountInfo {
  id: number;
  name: string;
  account_type: AccountType;
}

// ============================================================================
// Balance Types
// ============================================================================

/** Amount in zatoshis (1 ZEC = 100,000,000 zatoshis) */
export type Zatoshis = string;

export interface Balance {
  shielded_spendable: Zatoshis;
  shielded_pending: Zatoshis;
  transparent_total: Zatoshis;
  total: Zatoshis;
}

// ============================================================================
// Address Types
// ============================================================================

export type AddressType = 'ShieldedOnly' | 'Transparent';

export interface AddressInfo {
  encoded: string;
  address_type: AddressType;
  /** Diversifier index as string to avoid JS number overflow for u64 values */
  diversifier_index: string;
}

// ============================================================================
// Transaction Types
// ============================================================================

export type TransactionType = 'Send' | 'Receive' | 'Shield' | 'Consolidate';
export type TransactionStatus = 'Pending' | 'Confirmed' | 'Expired' | 'Failed';
export type RecipientKind = 'Orchard' | 'Sapling' | 'Transparent';

export interface TransactionInfo {
  txid: string;
  /** Wallet-local account index (ZIP-32) */
  account_id: number;
  tx_type: TransactionType;
  value: Zatoshis;
  fee: Zatoshis;
  memo_present: boolean;
  memo: string | null;
  status: TransactionStatus;
  /** Last error message for failed/queued broadcasts (user-safe, redacted) */
  last_error: string | null;
  /** True if user can retry broadcasting this tx (i.e., signed bytes were queued after a broadcast failure) */
  can_retry_broadcast: boolean;
  mined_height: number | null;
  created_at: UnixTimestampMs;
  confirmed_at: UnixTimestampMs | null;
}

// ============================================================================
// Sync Types
// ============================================================================

export type SyncPhase =
  | 'Idle'
  | 'Preparing'
  | 'Downloading'
  | 'Scanning'
  | 'Enhancing'
  | 'CatchingUp'
  /** Network is unreachable; sync is retrying with exponential backoff. */
  | 'Offline'
  /** Sync encountered a local error (DB, scan); retrying with backoff. */
  | 'Error';

export interface SyncProgress {
  phase: SyncPhase;
  scan_frontier_height: number;
  wallet_tip_height: number;
  progress_percent: number;
  eta_seconds: number | null;
  /** Seconds until the next retry attempt (populated when phase is Offline or Error). */
  retry_in_seconds?: number;
  /** User-safe, high-level error message (populated when phase is Error). */
  error_message?: string;
}

// ============================================================================
// Swap Types
// ============================================================================

export type SwapType = 'ToZec' | 'FromZec';
export type SwapState =
  | 'Draft'
  | 'AwaitingDeposit'
  | 'Pending'
  | 'Confirming'
  | 'Completed'
  | 'Refunded'
  | 'Failed';

export interface SwapInfo {
  id: string;
  remote_id: string | null;
  swap_type: SwapType;
  input_asset: string;
  input_amount: string;
  output_asset: string;
  output_amount: string | null;
  deposit_address: string | null;
  deposit_memo: string | null;
  destination_address: string | null;
  refund_address: string | null;
  state: SwapState;
  deadline: UnixTimestampMs | null;
  last_error: string | null;
  created_at: UnixTimestampMs;
  updated_at: UnixTimestampMs;
}

export interface SwapQuote {
  input_asset: string;
  input_amount: string;
  output_asset: string;
  output_amount: string;
  fee_amount: string;
  fee_asset: string;
  deadline: UnixTimestampMs;
  rate: string;
}

// ============================================================================
// Tor Types
// ============================================================================

export type TorStatus = 'Off' | 'Connecting' | 'On' | 'Error';

export interface TorState {
  enabled: boolean;
  status: TorStatus;
  last_error: string | null;
}

// ============================================================================
// Wallet Status Types
// ============================================================================

/** NOTE: Rust serde serializes unit enum variants as strings (e.g. "Required"). */
export type BackupAction = 'Required' | 'Complete';
export type SyncStatus =
  | 'Synced'
  | { Syncing: { progress_percent: number } }
  /** Network unreachable; retrying with exponential backoff. Cached funds remain visible. */
  | { Offline: { retry_in_seconds: number } }
  | { Error: { message: string } };
export type ShieldAction =
  | 'None'
  | { Available: { amount: Zatoshis } }
  | 'InProgress';
export type PrivacyPosture = 'Optimal' | 'NeedsAction';

export interface WalletStatus {
  lock_status: WalletLockStatus;
  backup_status: BackupAction;
  sync_status: SyncStatus;
  shield_status: ShieldAction;
  privacy_posture: PrivacyPosture;
}

// ============================================================================
// Backup Types
// ============================================================================

export interface BackupChallenge {
  /** Opaque challenge identifier */
  challenge_id: string;
  /** Seed word indices requested for verification (exactly 4; 1..=24, 1-based word numbers) */
  indices: number[];
  /** Challenge expiry timestamp */
  expires_at: UnixTimestampMs;
}

// ============================================================================
// Re-auth Types
// ============================================================================

export type ReauthPurpose = 'Spend' | 'ViewSeedPhrase';

// ============================================================================
// Server Types
// ============================================================================

export interface ServerInfo {
  id: string;
  name: string;
  grpc_url: string;
  network: Network;
  is_default: boolean;
  last_success_at: UnixTimestampMs | null;
}

// ============================================================================
// Keystone Types
// ============================================================================

export interface SigningRequest {
  /** Base64-encoded PCZT payload */
  pczt_payload: string;
  /** QR frames for animated display */
  qr_frames: string[];
  /** Transaction summary for verification */
  summary: SigningSummary;
}

export interface SigningSummary {
  recipient: string;
  recipient_kind: RecipientKind;
  amount: Zatoshis;
  fee: Zatoshis;
  memo_present: boolean;
  tx_type: TransactionType;
}

export interface SignedResponse {
  /** Base64-encoded signed PCZT payload */
  signed_payload: string;
}

// ============================================================================
// Command Requests
// ============================================================================

/**
 * Create a new wallet.
 *
 * On success, this sets the created wallet as the active wallet (equivalent to `LoadWallet`).
 */
export interface CreateWalletRequest extends VersionedPayload {
  name: string;
  network: Network;
  /** Wallet password used to encrypt spend-capable secrets and wallet DB at rest */
  password: string;
  /**
   * DISABLED: Keychain-based auto-unlock is disabled due to security concerns (issue #45).
   * This parameter is ignored; always treated as false. Retained for schema compatibility.
   */
  remember_unlock: boolean;
}

/**
 * Load an existing wallet and set it as the active wallet for account-scoped requests/events.
 *
 * NOTE: Keychain-based auto-unlock is disabled due to security concerns (issue #45).
 * Wallets always load in Locked state and require password entry via UnlockWallet.
 *
 * Accounts are available only when the encrypted wallet DB is unlocked:
 * - If `lock_status` is `Locked`, `LoadWalletResponse.accounts` MUST be an empty array.
 * - After a successful `UnlockWallet`, the UI SHOULD call `LoadWallet` again to obtain `accounts`.
 */
export interface LoadWalletRequest extends VersionedPayload {
  wallet_id: string;
}

/** List all wallets */
export interface ListWalletsRequest extends VersionedPayload {}

/** Get wallet status for status widget */
export interface GetWalletStatusRequest extends VersionedPayload {
  wallet_id: string;
}

/** Unlock an encrypted wallet DB (read-only operations still require unlock) */
export interface UnlockWalletRequest extends VersionedPayload {
  wallet_id: string;
  password: string;
  /**
   * DISABLED: Keychain-based auto-unlock is disabled due to security concerns (issue #45).
   * This parameter is ignored; always treated as false. Retained for schema compatibility.
   */
  remember_unlock: boolean;
}

/** Lock a wallet (drops decrypted material from memory) */
export interface LockWalletRequest extends VersionedPayload {
  wallet_id: string;
}

/**
 * Re-authenticate for a sensitive action (per-action; OS keychain must not satisfy).
 * Returns a short-lived token that can be used to authorize exactly one action.
 */
export interface ReauthWalletRequest extends VersionedPayload {
  wallet_id: string;
  password: string;
  purpose: ReauthPurpose;
}

/** View seed phrase (requires re-auth token with purpose ViewSeedPhrase) */
export interface ViewSeedPhraseRequest extends VersionedPayload {
  wallet_id: string;
  reauth_token: string;
}

/**
 * Account-scoped requests (those with account_id) operate on the currently active wallet.
 *
 * The active wallet is set by:
 * - `LoadWallet`
 * - `CreateWallet` (on success)
 * - `RestoreWallet` (on success)
 *
 * account_id values are wallet-local to the active wallet.
 */
/** Get fresh shielded receive address */
export interface GetReceiveAddressRequest extends VersionedPayload {
  account_id: number;
  address_type: AddressType;
}

/** Start wallet sync */
export interface StartSyncRequest extends VersionedPayload {
  wallet_id: string;
}

/** Stop wallet sync */
export interface StopSyncRequest extends VersionedPayload {
  wallet_id: string;
}

/** Get current sync progress snapshot */
export interface GetSyncProgressRequest extends VersionedPayload {
  wallet_id: string;
}

/** Get wallet balance */
export interface GetBalanceRequest extends VersionedPayload {
  account_id: number;
}

/** List transactions */
export interface ListTransactionsRequest extends VersionedPayload {
  account_id: number;
  limit: number;
  offset: number;
}

/**
 * Prepare a send transaction (software wallet flow).
 * Returns a proposal that can be confirmed. Transaction bytes stay in backend.
 */
export interface PrepareSendRequest extends VersionedPayload {
  account_id: number;
  /** Recipient address (UA, Sapling, Orchard, or Transparent) */
  recipient: string;
  amount: Zatoshis;
  memo: string | null;
  /**
   * Required acknowledgement for privacy downgrades.
   * Must be true when the recipient resolves to a transparent receiver (t-addr or UA with only transparent receiver).
   */
  allow_transparent_recipient: boolean;
}

/**
 * Confirm and broadcast a prepared send transaction.
 * The proposal_id references the transaction prepared in the backend.
 */
export interface ConfirmSendRequest extends VersionedPayload {
  /** Proposal ID from PrepareSendResponse */
  proposal_id: string;
  /** Re-auth token (purpose: Spend) */
  reauth_token: string;
}

/**
 * Cancel a prepared send transaction (optional, proposals expire automatically).
 */
export interface CancelSendRequest extends VersionedPayload {
  proposal_id: string;
}

/**
 * Retry broadcasting a previously-signed transaction that was queued after a broadcast failure.
 * Requires explicit user action and manual re-auth.
 */
export interface RetryBroadcastRequest extends VersionedPayload {
  txid: string;
  /** Re-auth token (purpose: Spend) */
  reauth_token: string;
}

/** Shield transparent funds */
export interface ShieldFundsRequest extends VersionedPayload {
  account_id: number;
  consolidate: boolean;
  /** Re-auth token (purpose: Spend) */
  reauth_token: string;
}

/** Get a fresh backend-generated backup challenge */
export interface GetBackupChallengeRequest extends VersionedPayload {
  wallet_id: string;
}

/** Verify backup words */
export interface VerifyBackupRequest extends VersionedPayload {
  wallet_id: string;
  /** Challenge ID issued by the backend (prevents UI-controlled verification) */
  challenge_id: string;
  /** Map of word index (1-24) to word */
  word_challenges: Record<number, string>;
}

/**
 * Restore wallet from seed.
 *
 * On success, this sets the restored wallet as the active wallet (equivalent to `LoadWallet`).
 */
export interface RestoreWalletRequest extends VersionedPayload {
  name: string;
  network: Network;
  /** Wallet password used to encrypt spend-capable secrets and wallet DB at rest */
  password: string;
  /**
   * DISABLED: Keychain-based auto-unlock is disabled due to security concerns (issue #45).
   * This parameter is ignored; always treated as false. Retained for schema compatibility.
   */
  remember_unlock: boolean;
  seed_phrase: string;
  /** Approximate date of first transaction (unix timestamp, ms) */
  birthday_date: UnixTimestampMs | null;
}

/** Import UFVK (Keystone) to create a HardwareSigner account (watch-only; spends via signing flow) within an existing software wallet */
export interface ImportUfvkRequest extends VersionedPayload {
  wallet_id: string;
  ufvk: string;
  name: string;
  /** 32-byte seed fingerprint as hex string (from Keystone QR) */
  seed_fingerprint: string | null;
  /** ZIP-32 account index (from Keystone QR) */
  zip32_account_index: number | null;
}

/** Build unsigned signing request for Keystone */
export interface BuildSigningRequestRequest extends VersionedPayload {
  account_id: number;
  recipient: string;
  amount: Zatoshis;
  memo: string | null;
  /**
   * Required acknowledgement for privacy downgrades.
   * Must be true when the recipient resolves to a transparent receiver (t-addr or UA with only transparent receiver).
   */
  allow_transparent_recipient: boolean;
}

/** Finalize signed response from Keystone */
export interface FinalizeSigningRequest extends VersionedPayload {
  signed_payload: string;
  /** Re-auth token (purpose: Spend) */
  reauth_token: string;
}

/** Request swap quote */
export interface RequestSwapQuoteRequest extends VersionedPayload {
  swap_type: SwapType;
  input_asset: string;
  input_amount: string;
  output_asset: string;
  destination_address: string | null;
  refund_address: string | null;
}

/** Start swap from quote */
export interface StartSwapRequest extends VersionedPayload {
  quote_id: string;
  /**
   * Required acknowledgement for privacy downgrades.
   * Must be true when the swap requires transparent interaction (for example, a transparent recipient,
   * transparent refund path, or generating/using a transparent address).
   */
  allow_transparent_interaction: boolean;
  /**
   * Re-auth token (purpose: Spend).
   * Required when the quote results in a ZEC spend (e.g. swap-from-ZEC).
   */
  reauth_token: string | null;
}

/** Get swap status */
export interface GetSwapStatusRequest extends VersionedPayload {
  swap_id: string;
}

/**
 * List swaps for the currently active wallet only.
 *
 * Note: SwapInfo does not include a wallet_id, so this call is wallet-scoped.
 */
export interface ListSwapsRequest extends VersionedPayload {}

/** Set Tor enabled */
export interface SetTorEnabledRequest extends VersionedPayload {
  enabled: boolean;
}

/** Get current Tor state */
export interface GetTorStateRequest extends VersionedPayload {}

/**
 * Add a custom lightwalletd server configuration.
 *
 * The backend MUST probe `grpc_url` to determine and persist the server's `network` (see ServerInfo.network).
 * Probing MUST also validate required server capabilities for v1, including CompactTxStreamer mempool support for FR-013 (`GetMempoolStream` MUST be supported; reject `UNIMPLEMENTED`).
 * If probing fails, the request MUST fail rather than guessing.
 */
export interface AddServerRequest extends VersionedPayload {
  name: string;
  grpc_url: string;
}

/** Set default server */
export interface SetDefaultServerRequest extends VersionedPayload {
  server_id: string;
}

/** Test server connection */
export interface TestServerRequest extends VersionedPayload {
  server_id: string;
}

/**
 * List all configured servers (Mainnet + Testnet).
 *
 * UI SHOULD filter by the active wallet's network when presenting selectable servers.
 */
export interface ListServersRequest extends VersionedPayload {}

/** Get log file location for support */
export interface GetLogLocationRequest extends VersionedPayload {}

export interface GetLogLocationResponse extends VersionedPayload {
  /** Directory containing log files */
  log_directory: string;
  /** Current log file path */
  current_log_file: string;
}

// ============================================================================
// Command Responses
// ============================================================================

export interface CreateWalletResponse extends VersionedPayload {
  wallet: WalletInfo;
  /** Seed phrase words (24 words) - returned on create (and via ViewSeedPhrase) */
  seed_phrase: string[];
  /** Initial backend-generated backup challenge */
  backup_challenge: BackupChallenge;
}

export interface UnlockWalletResponse extends VersionedPayload {
  unlocked: boolean;
}

export interface LockWalletResponse extends VersionedPayload {
  locked: boolean;
}

export interface ReauthWalletResponse extends VersionedPayload {
  reauth_token: string;
  /** Token expiry timestamp (v1: expires_at = issued_at + 120 seconds) */
  expires_at: UnixTimestampMs;
}

export interface ViewSeedPhraseResponse extends VersionedPayload {
  seed_phrase: string[];
}

export interface LoadWalletResponse extends VersionedPayload {
  wallet: WalletInfo;
  lock_status: WalletLockStatus;

  /**
   * Accounts are available only when the wallet DB is unlocked.
   * If `lock_status` is `Locked`, this MUST be an empty array.
   * After a successful `UnlockWallet`, the UI SHOULD call `LoadWallet` again to obtain `accounts`.
   */
  accounts: AccountInfo[];
}

export interface ListWalletsResponse extends VersionedPayload {
  wallets: WalletInfo[];
}

export interface GetReceiveAddressResponse extends VersionedPayload {
  address: AddressInfo;
}

export interface StartSyncResponse extends VersionedPayload {
  started: boolean;
}

export interface StopSyncResponse extends VersionedPayload {
  stopped: boolean;
}

export interface GetSyncProgressResponse extends VersionedPayload {
  progress: SyncProgress;
}

export interface GetBalanceResponse extends VersionedPayload {
  balance: Balance;
}

export interface ListTransactionsResponse extends VersionedPayload {
  transactions: TransactionInfo[];
  total_count: number;
}

/**
 * Response from PrepareSend. Contains proposal details for user review.
 * Transaction bytes are held in backend memory, not sent to UI.
 */
export interface PrepareSendResponse extends VersionedPayload {
  /** Unique proposal identifier (backend-side reference) */
  proposal_id: string;
  /** Fee for the transaction */
  fee: Zatoshis;
  /** Summary for user verification */
  summary: TransactionSummary;
  /** Proposal expiration timestamp (v1: proposals auto-expire after 10 minutes) */
  expires_at: UnixTimestampMs;
}

/** Transaction summary for user verification before confirming */
export interface TransactionSummary {
  recipient: string;
  recipient_kind: RecipientKind;
  amount: Zatoshis;
  fee: Zatoshis;
  memo_present: boolean;
  total_spend: Zatoshis;
}

export interface ConfirmSendResponse extends VersionedPayload {
  txid: string;
}

export interface CancelSendResponse extends VersionedPayload {
  cancelled: boolean;
}

export interface RetryBroadcastResponse extends VersionedPayload {
  txid: string;
}

export interface ShieldFundsResponse extends VersionedPayload {
  /**
   * Txid of the shielding transaction created by this call.
   *
   * Note: Shielding may batch into multiple transactions when the transparent input
   * set is too large to fit in a single transaction; in that case this `txid`
   * refers to the first transaction in the batch. Additional shielding txids are
   * observable via `tx.changed` events and `ListTransactions`.
   */
  txid: string;
  /** Fee (zatoshis) for the transaction identified by `txid` */
  fee: Zatoshis;
}

export interface VerifyBackupResponse extends VersionedPayload {
  verified: boolean;
}

export interface GetBackupChallengeResponse extends VersionedPayload {
  challenge: BackupChallenge;
}

export interface RestoreWalletResponse extends VersionedPayload {
  wallet: WalletInfo;
  birthday_height: number;
}

export interface ImportUfvkResponse extends VersionedPayload {
  account: AccountInfo;
}

export interface BuildSigningRequestResponse extends VersionedPayload {
  signing_request: SigningRequest;
}

export interface FinalizeSigningResponse extends VersionedPayload {
  txid: string;
}

export interface RequestSwapQuoteResponse extends VersionedPayload {
  quote_id: string;
  quote: SwapQuote;
}

export interface StartSwapResponse extends VersionedPayload {
  swap: SwapInfo;
}

export interface GetSwapStatusResponse extends VersionedPayload {
  swap: SwapInfo;
}

export interface SetTorEnabledResponse extends VersionedPayload {
  state: TorState;
}

export interface AddServerResponse extends VersionedPayload {
  server: ServerInfo;
}

export interface SetDefaultServerResponse extends VersionedPayload {
  success: boolean;
}

export interface TestServerResponse extends VersionedPayload {
  success: boolean;
  latency_ms: number | null;
  error: string | null;
}

export interface GetWalletStatusResponse extends VersionedPayload {
  status: WalletStatus;
}

export interface GetTorStateResponse extends VersionedPayload {
  state: TorState;
}

export interface ListServersResponse extends VersionedPayload {
  servers: ServerInfo[];
}

export interface ListSwapsResponse extends VersionedPayload {
  swaps: SwapInfo[];
}

// ============================================================================
// Events
// ============================================================================

/**
 * Events are emitted for the currently active wallet.
 *
 * The active wallet is set by:
 * - `LoadWallet`
 * - `CreateWallet` (on success)
 * - `RestoreWallet` (on success)
 *
 * Any account_id values are wallet-local to the active wallet.
 */
/** Sync progress update */
export interface SyncProgressEvent extends VersionedPayload {
  event: 'sync.progress';
  progress: SyncProgress;
}

/** Balance changed */
export interface BalanceChangedEvent extends VersionedPayload {
  event: 'balance.changed';
  account_id: number;
  balance: Balance;
}

/** Transaction state changed */
export interface TransactionChangedEvent extends VersionedPayload {
  event: 'tx.changed';
  transaction: TransactionInfo;
}

/** Swap state changed */
export interface SwapChangedEvent extends VersionedPayload {
  event: 'swap.changed';
  swap: SwapInfo;
}

/** Tor status changed */
export interface TorStatusEvent extends VersionedPayload {
  event: 'tor.status';
  state: TorState;
}

/** Wallet status changed */
export interface WalletStatusEvent extends VersionedPayload {
  event: 'wallet.status';
  status: WalletStatus;
}

export type IpcEvent =
  | SyncProgressEvent
  | BalanceChangedEvent
  | TransactionChangedEvent
  | SwapChangedEvent
  | TorStatusEvent
  | WalletStatusEvent;

// ============================================================================
// Error Codes
// ============================================================================

export const ErrorCodes = {
  // Wallet errors
  WALLET_NOT_FOUND: 'E1001',
  WALLET_LOCKED: 'E1002',
  WALLET_ALREADY_EXISTS: 'E1003',
  INVALID_SEED_PHRASE: 'E1004',
  BACKUP_REQUIRED: 'E1005',
  BACKUP_CHALLENGE_INVALID: 'E1006',
  BACKUP_CHALLENGE_EXPIRED: 'E1007',
  INVALID_WALLET_PASSWORD: 'E1008',
  REAUTH_REQUIRED: 'E1009',
  REAUTH_TOKEN_INVALID: 'E1010',
  REAUTH_TOKEN_EXPIRED: 'E1011',
  BACKUP_CHALLENGE_TOO_MANY_ATTEMPTS: 'E1012',

  // Account errors
  ACCOUNT_NOT_FOUND: 'E2001',
  WATCH_ONLY_CANNOT_SPEND: 'E2002',

  // Transaction errors
  INSUFFICIENT_FUNDS: 'E3001',
  INVALID_RECIPIENT: 'E3002',
  TRANSPARENT_SPEND_BLOCKED: 'E3003',
  TRANSACTION_FAILED: 'E3004',
  MEMO_TOO_LONG: 'E3005',
  PROPOSAL_NOT_FOUND: 'E3006',
  PROPOSAL_EXPIRED: 'E3007',
  QUEUED_BROADCAST_NOT_FOUND: 'E3008',
  QUEUED_BROADCAST_EXPIRED: 'E3009',
  PRIVACY_ACK_REQUIRED: 'E3010',
  MEMO_NOT_ALLOWED: 'E3011',
  JOB_NOT_FOUND: 'E3012',

  // Sync errors
  SYNC_IN_PROGRESS: 'E4001',
  SERVER_UNAVAILABLE: 'E4002',
  SYNC_FAILED: 'E4003',

  // Keystone errors
  INVALID_UFVK: 'E5001',
  INVALID_PCZT: 'E5002',
  SIGNING_FAILED: 'E5003',

  // Swap errors
  QUOTE_EXPIRED: 'E6001',
  SWAP_FAILED: 'E6002',
  INVALID_ASSET: 'E6003',
  SWAP_UNSUPPORTED_NETWORK: 'E6004',

  // Tor errors
  TOR_NOT_READY: 'E7001',
  TOR_CONNECTION_FAILED: 'E7002',

  // General errors
  INVALID_REQUEST: 'E9001',
  INTERNAL_ERROR: 'E9002',
  SCHEMA_VERSION_MISMATCH: 'E9003',
} as const;

// ============================================================================
// Command Names (for Tauri invoke)
// ============================================================================

export const Commands = {
  // Wallet
  CREATE_WALLET: 'zstash_create_wallet',
  LOAD_WALLET: 'zstash_load_wallet',
  LIST_WALLETS: 'zstash_list_wallets',
  GET_WALLET_STATUS: 'zstash_get_wallet_status',
  UNLOCK_WALLET: 'zstash_unlock_wallet',
  LOCK_WALLET: 'zstash_lock_wallet',
  REAUTH_WALLET: 'zstash_reauth_wallet',
  VIEW_SEED_PHRASE: 'zstash_view_seed_phrase',

  // Address
  GET_RECEIVE_ADDRESS: 'zstash_get_receive_address',

  // Sync
  START_SYNC: 'zstash_start_sync',
  STOP_SYNC: 'zstash_stop_sync',
  GET_SYNC_PROGRESS: 'zstash_get_sync_progress',

  // Balance
  GET_BALANCE: 'zstash_get_balance',

  // Transactions
  LIST_TRANSACTIONS: 'zstash_list_transactions',
  PREPARE_SEND: 'zstash_prepare_send',
  CONFIRM_SEND: 'zstash_confirm_send',
  CANCEL_SEND: 'zstash_cancel_send',
  RETRY_BROADCAST: 'zstash_retry_broadcast',
  SHIELD_FUNDS: 'zstash_shield_funds',

  // Backup
  GET_BACKUP_CHALLENGE: 'zstash_get_backup_challenge',
  VERIFY_BACKUP: 'zstash_verify_backup',
  RESTORE_WALLET: 'zstash_restore_wallet',

  // Keystone
  IMPORT_UFVK: 'zstash_import_ufvk',
  BUILD_SIGNING_REQUEST: 'zstash_build_signing_request',
  FINALIZE_SIGNING: 'zstash_finalize_signing',

  // Swaps
  REQUEST_SWAP_QUOTE: 'zstash_request_swap_quote',
  START_SWAP: 'zstash_start_swap',
  GET_SWAP_STATUS: 'zstash_get_swap_status',
  LIST_SWAPS: 'zstash_list_swaps',

  // Tor
  SET_TOR_ENABLED: 'zstash_set_tor_enabled',
  GET_TOR_STATE: 'zstash_get_tor_state',

  // Servers
  ADD_SERVER: 'zstash_add_server',
  SET_DEFAULT_SERVER: 'zstash_set_default_server',
  TEST_SERVER: 'zstash_test_server',
  LIST_SERVERS: 'zstash_list_servers',

  // Logs
  GET_LOG_LOCATION: 'zstash_get_log_location',
} as const;

// ============================================================================
// Event Channels (for Tauri listen)
// ============================================================================

export const EventChannels = {
  SYNC: 'zstash://sync',
  BALANCE: 'zstash://balance',
  TRANSACTION: 'zstash://tx',
  SWAP: 'zstash://swap',
  TOR: 'zstash://tor',
  WALLET_STATUS: 'zstash://wallet-status',
} as const;
