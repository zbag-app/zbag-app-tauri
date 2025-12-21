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
  diversifier_index: number;
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

export type SwapType = 'ToZec' | 'FromZec' | 'Pay';
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

export type BackupAction = { Required: null } | { Complete: null };
export type SyncStatus =
  | { Synced: null }
  | { Syncing: { progress_percent: number } }
  | { Error: { message: string } };
export type ShieldAction =
  | { None: null }
  | { Available: { amount: Zatoshis } }
  | { InProgress: null };
export type PrivacyPosture = 'Optimal' | 'NeedsAction';

export interface WalletStatus {
  backup_status: BackupAction;
  sync_status: SyncStatus;
  shield_status: ShieldAction;
  privacy_posture: PrivacyPosture;
}

// ============================================================================
// Server Types
// ============================================================================

export interface ServerInfo {
  id: string;
  name: string;
  grpc_url: string;
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

/** Get fresh shielded receive address */
export interface GetReceiveAddressRequest extends VersionedPayload {
  account_id: number;
  address_type: AddressType;
}

/** Start wallet sync */
export interface StartSyncRequest extends VersionedPayload {
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

/** Build send transaction */
export interface BuildSendRequest extends VersionedPayload {
  account_id: number;
  recipient: string;
  amount: Zatoshis;
  memo: string | null;
}

/** Submit transaction */
export interface SubmitTransactionRequest extends VersionedPayload {
  /** Raw transaction bytes, base64 encoded */
  tx_bytes: string;
}

/** Shield transparent funds */
export interface ShieldFundsRequest extends VersionedPayload {
  account_id: number;
  consolidate: boolean;
}

/** Verify backup words */
export interface VerifyBackupRequest extends VersionedPayload {
  wallet_id: string;
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

/** Set Tor enabled */
export interface SetTorEnabledRequest extends VersionedPayload {
  enabled: boolean;
}

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

// ============================================================================
// Command Responses
// ============================================================================

export interface CreateWalletResponse extends VersionedPayload {
  wallet: WalletInfo;
  /** Seed phrase words (24 words) - ONLY returned on create */
  seed_phrase: string[];
  /** Word indices for backup verification challenge */
  backup_challenge_indices: number[];
}

export interface LoadWalletResponse extends VersionedPayload {
  wallet: WalletInfo;
  accounts: AccountInfo[];
}

export interface GetReceiveAddressResponse extends VersionedPayload {
  address: AddressInfo;
}

export interface StartSyncResponse extends VersionedPayload {
  started: boolean;
}

export interface GetBalanceResponse extends VersionedPayload {
  balance: Balance;
}

export interface ListTransactionsResponse extends VersionedPayload {
  transactions: TransactionInfo[];
  total_count: number;
}

export interface BuildSendResponse extends VersionedPayload {
  /** Raw transaction bytes, base64 encoded */
  tx_bytes: string;
  /** Fee for the transaction */
  fee: Zatoshis;
}

export interface SubmitTransactionResponse extends VersionedPayload {
  txid: string;
}

export interface ShieldFundsResponse extends VersionedPayload {
  txid: string;
  fee: Zatoshis;
}

export interface VerifyBackupResponse extends VersionedPayload {
  verified: boolean;
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

  // Account errors
  ACCOUNT_NOT_FOUND: 'E2001',
  WATCH_ONLY_CANNOT_SPEND: 'E2002',

  // Transaction errors
  INSUFFICIENT_FUNDS: 'E3001',
  INVALID_RECIPIENT: 'E3002',
  TRANSPARENT_SPEND_BLOCKED: 'E3003',
  TRANSACTION_FAILED: 'E3004',
  MEMO_TOO_LONG: 'E3005',

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
  BUILD_SEND: 'zkore_build_send',
  SUBMIT_TRANSACTION: 'zkore_submit_transaction',
  SHIELD_FUNDS: 'zkore_shield_funds',

  // Backup
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
