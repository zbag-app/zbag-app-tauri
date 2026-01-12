// Error codes are part of the stable IPC contract and must match
// `specs/001-zstash-desktop-wallet/contracts/ipc-v1.ts`.

// Wallet errors
pub const WALLET_NOT_FOUND: &str = "E1001";
pub const WALLET_LOCKED: &str = "E1002";
pub const WALLET_ALREADY_EXISTS: &str = "E1003";
pub const INVALID_SEED_PHRASE: &str = "E1004";
pub const BACKUP_REQUIRED: &str = "E1005";
pub const BACKUP_CHALLENGE_INVALID: &str = "E1006";
pub const BACKUP_CHALLENGE_EXPIRED: &str = "E1007";
pub const INVALID_WALLET_PASSWORD: &str = "E1008";
pub const REAUTH_REQUIRED: &str = "E1009";
pub const REAUTH_TOKEN_INVALID: &str = "E1010";
pub const REAUTH_TOKEN_EXPIRED: &str = "E1011";
pub const BACKUP_CHALLENGE_TOO_MANY_ATTEMPTS: &str = "E1012";

// Account errors
pub const ACCOUNT_NOT_FOUND: &str = "E2001";
pub const WATCH_ONLY_CANNOT_SPEND: &str = "E2002";

// Transaction errors
pub const INSUFFICIENT_FUNDS: &str = "E3001";
pub const INVALID_RECIPIENT: &str = "E3002";
pub const TRANSPARENT_SPEND_BLOCKED: &str = "E3003";
pub const TRANSACTION_FAILED: &str = "E3004";
pub const MEMO_TOO_LONG: &str = "E3005";
pub const PROPOSAL_NOT_FOUND: &str = "E3006";
pub const PROPOSAL_EXPIRED: &str = "E3007";
pub const QUEUED_BROADCAST_NOT_FOUND: &str = "E3008";
pub const QUEUED_BROADCAST_EXPIRED: &str = "E3009";
pub const PRIVACY_ACK_REQUIRED: &str = "E3010";
pub const MEMO_NOT_ALLOWED: &str = "E3011";

// Sync errors
pub const SYNC_IN_PROGRESS: &str = "E4001";
pub const SERVER_UNAVAILABLE: &str = "E4002";
pub const SYNC_FAILED: &str = "E4003";
// Detailed sync errors
pub const SYNC_CHAIN_TIP_FAILED: &str = "E4004";
pub const SYNC_CACHE_INIT_FAILED: &str = "E4005";
pub const SYNC_DB_INIT_FAILED: &str = "E4006";
pub const SYNC_WALLET_DB_FAILED: &str = "E4007";
pub const SYNC_CHAIN_UPDATE_FAILED: &str = "E4008";
pub const SYNC_SCAN_FAILED: &str = "E4009";
pub const SYNC_TREE_STATE_FAILED: &str = "E4010";
pub const SYNC_BLOCK_CACHE_FAILED: &str = "E4011";

// Keystone errors
pub const INVALID_UFVK: &str = "E5001";
pub const INVALID_PCZT: &str = "E5002";
pub const SIGNING_FAILED: &str = "E5003";

// Swap errors
pub const QUOTE_EXPIRED: &str = "E6001";
pub const SWAP_FAILED: &str = "E6002";
pub const INVALID_ASSET: &str = "E6003";
pub const SWAP_UNSUPPORTED_NETWORK: &str = "E6004";

// Tor errors
pub const TOR_NOT_READY: &str = "E7001";
pub const TOR_CONNECTION_FAILED: &str = "E7002";

// Watch-only wallet errors
pub const WATCH_ONLY_NO_SEED: &str = "E9010";
pub const WATCH_ONLY_NO_BACKUP: &str = "E9011";
pub const WATCH_ONLY_CANNOT_SHIELD: &str = "E9012";

// General errors
pub const INVALID_REQUEST: &str = "E9001";
pub const INTERNAL_ERROR: &str = "E9002";
pub const SCHEMA_VERSION_MISMATCH: &str = "E9003";
