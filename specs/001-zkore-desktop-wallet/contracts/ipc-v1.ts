/**
 * Zkore Desktop Wallet - IPC Contract v1
 *
 * TypeScript type definitions for Tauri IPC commands and events.
 * These types MUST match the Rust definitions in zkore-core/src/ipc/v1/
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

// ============================================================================
// Wallet Types
// ============================================================================

export type WalletType = 'Software' | 'WatchOnly';
export type Network = 'Mainnet' | 'Testnet';
export type AccountType = 'Software' | 'WatchOnly' | 'HardwareSigner';

export interface WalletInfo {
  id: string;
  name: string;
  wallet_type: WalletType;
  network: Network;
  created_at: number;
  last_opened_at: number | null;
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
  orchard_spendable: Zatoshis;
  orchard_pending: Zatoshis;
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

export interface TransactionInfo {
  txid: string;
  tx_type: TransactionType;
  value: Zatoshis;
  fee: Zatoshis;
  memo_present: boolean;
  memo: string | null;
  status: TransactionStatus;
  mined_height: number | null;
  created_at: number;
  confirmed_at: number | null;
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
  | 'CatchingUp';

export interface SyncProgress {
  phase: SyncPhase;
  scan_frontier_height: number;
  wallet_tip_height: number;
  progress_percent: number;
  eta_seconds: number | null;
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
  destination_address: string | null;
  refund_address: string | null;
  state: SwapState;
  deadline: number | null;
  last_error: string | null;
  created_at: number;
  updated_at: number;
}

export interface SwapQuote {
  input_asset: string;
  input_amount: string;
  output_asset: string;
  output_amount: string;
  fee_amount: string;
  fee_asset: string;
  deadline: number;
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
  | { Error: { message: string } };
export type ShieldAction =
  | 'None'
  | { Available: { amount: Zatoshis } }
  | 'InProgress';
export type PrivacyPosture = 'Optimal' | 'NeedsAction';

export interface WalletStatus {
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
  /** Seed word indices requested for verification */
  indices: number[];
  /** Challenge expiry timestamp (unix seconds) */
  expires_at: number;
}

// ============================================================================
// Server Types
// ============================================================================

export interface ServerInfo {
  id: string;
  name: string;
  grpc_url: string;
  network: Network;
  is_default: boolean;
  last_success_at: number | null;
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

/** Create a new wallet */
export interface CreateWalletRequest extends VersionedPayload {
  name: string;
  network: Network;
}

/** Load an existing wallet */
export interface LoadWalletRequest extends VersionedPayload {
  wallet_id: string;
}

/** List all wallets */
export interface ListWalletsRequest extends VersionedPayload {}

/** Get wallet status for status widget */
export interface GetWalletStatusRequest extends VersionedPayload {
  wallet_id: string;
}

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
  recipient: string;
  amount: Zatoshis;
  memo: string | null;
}

/**
 * Confirm and broadcast a prepared send transaction.
 * The proposal_id references the transaction prepared in the backend.
 */
export interface ConfirmSendRequest extends VersionedPayload {
  /** Proposal ID from PrepareSendResponse */
  proposal_id: string;
}

/**
 * Cancel a prepared send transaction (optional, proposals expire automatically).
 */
export interface CancelSendRequest extends VersionedPayload {
  proposal_id: string;
}

/** Shield transparent funds */
export interface ShieldFundsRequest extends VersionedPayload {
  account_id: number;
  consolidate: boolean;
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
  /** Map of word index (0-23) to word */
  word_challenges: Record<number, string>;
}

/** Restore wallet from seed */
export interface RestoreWalletRequest extends VersionedPayload {
  name: string;
  network: Network;
  seed_phrase: string;
  /** Approximate date of first transaction (unix timestamp) */
  birthday_date: number | null;
}

/** Import UFVK for watch-only */
export interface ImportUfvkRequest extends VersionedPayload {
  wallet_id: string;
  ufvk: string;
  name: string;
}

/** Build unsigned signing request for Keystone */
export interface BuildSigningRequestRequest extends VersionedPayload {
  account_id: number;
  recipient: string;
  amount: Zatoshis;
  memo: string | null;
}

/** Finalize signed response from Keystone */
export interface FinalizeSigningRequest extends VersionedPayload {
  signed_payload: string;
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
}

/** Get swap status */
export interface GetSwapStatusRequest extends VersionedPayload {
  swap_id: string;
}

/** List swaps */
export interface ListSwapsRequest extends VersionedPayload {}

/** Set Tor enabled */
export interface SetTorEnabledRequest extends VersionedPayload {
  enabled: boolean;
}

/** Get current Tor state */
export interface GetTorStateRequest extends VersionedPayload {}

/** Add server */
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

/** List configured servers */
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
  /** Seed phrase words (24 words) - ONLY returned on create */
  seed_phrase: string[];
  /** Initial backend-generated backup challenge */
  backup_challenge: BackupChallenge;
}

export interface LoadWalletResponse extends VersionedPayload {
  wallet: WalletInfo;
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
  /** Proposal expiration timestamp (proposals auto-expire after ~5 minutes) */
  expires_at: number;
}

/** Transaction summary for user verification before confirming */
export interface TransactionSummary {
  recipient: string;
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

export interface ShieldFundsResponse extends VersionedPayload {
  txid: string;
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
  CREATE_WALLET: 'zkore_create_wallet',
  LOAD_WALLET: 'zkore_load_wallet',
  LIST_WALLETS: 'zkore_list_wallets',
  GET_WALLET_STATUS: 'zkore_get_wallet_status',

  // Address
  GET_RECEIVE_ADDRESS: 'zkore_get_receive_address',

  // Sync
  START_SYNC: 'zkore_start_sync',
  STOP_SYNC: 'zkore_stop_sync',
  GET_SYNC_PROGRESS: 'zkore_get_sync_progress',

  // Balance
  GET_BALANCE: 'zkore_get_balance',

  // Transactions
  LIST_TRANSACTIONS: 'zkore_list_transactions',
  PREPARE_SEND: 'zkore_prepare_send',
  CONFIRM_SEND: 'zkore_confirm_send',
  CANCEL_SEND: 'zkore_cancel_send',
  SHIELD_FUNDS: 'zkore_shield_funds',

  // Backup
  GET_BACKUP_CHALLENGE: 'zkore_get_backup_challenge',
  VERIFY_BACKUP: 'zkore_verify_backup',
  RESTORE_WALLET: 'zkore_restore_wallet',

  // Keystone
  IMPORT_UFVK: 'zkore_import_ufvk',
  BUILD_SIGNING_REQUEST: 'zkore_build_signing_request',
  FINALIZE_SIGNING: 'zkore_finalize_signing',

  // Swaps
  REQUEST_SWAP_QUOTE: 'zkore_request_swap_quote',
  START_SWAP: 'zkore_start_swap',
  GET_SWAP_STATUS: 'zkore_get_swap_status',
  LIST_SWAPS: 'zkore_list_swaps',

  // Tor
  SET_TOR_ENABLED: 'zkore_set_tor_enabled',
  GET_TOR_STATE: 'zkore_get_tor_state',

  // Servers
  ADD_SERVER: 'zkore_add_server',
  SET_DEFAULT_SERVER: 'zkore_set_default_server',
  TEST_SERVER: 'zkore_test_server',
  LIST_SERVERS: 'zkore_list_servers',

  // Logs
  GET_LOG_LOCATION: 'zkore_get_log_location',
} as const;

// ============================================================================
// Event Channels (for Tauri listen)
// ============================================================================

export const EventChannels = {
  SYNC: 'zkore://sync',
  BALANCE: 'zkore://balance',
  TRANSACTION: 'zkore://tx',
  SWAP: 'zkore://swap',
  TOR: 'zkore://tor',
  WALLET_STATUS: 'zkore://wallet-status',
} as const;
