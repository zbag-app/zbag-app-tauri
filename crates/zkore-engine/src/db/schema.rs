pub const INITIAL_SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS wallets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    directory_path TEXT NOT NULL,
    wallet_type TEXT NOT NULL,
    network TEXT NOT NULL,
    remember_unlock_enabled INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    last_opened_at INTEGER
);

CREATE TABLE IF NOT EXISTS accounts (
    wallet_id TEXT NOT NULL REFERENCES wallets(id),
    account_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (wallet_id, account_id)
);

CREATE TABLE IF NOT EXISTS wallet_encryption (
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

CREATE TABLE IF NOT EXISTS backup_status (
    wallet_id TEXT PRIMARY KEY REFERENCES wallets(id),
    backup_required INTEGER NOT NULL DEFAULT 1,
    backup_completed_at INTEGER,
    verification_method TEXT
);

CREATE TABLE IF NOT EXISTS servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    grpc_url TEXT NOT NULL,
    network TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    last_success_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS servers_one_default_per_network
ON servers(network)
WHERE is_default = 1;

CREATE TABLE IF NOT EXISTS tor_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'Off',
    last_error TEXT,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS swaps (
    id TEXT PRIMARY KEY,
    remote_id TEXT,
    wallet_id TEXT NOT NULL REFERENCES wallets(id),
    swap_type TEXT NOT NULL,
    input_asset TEXT NOT NULL,
    input_amount TEXT NOT NULL,
    output_asset TEXT NOT NULL,
    output_amount TEXT,
    deposit_address TEXT,
    deposit_memo TEXT,
    destination_address TEXT,
    refund_address TEXT,
    state TEXT NOT NULL DEFAULT 'Draft',
    deadline INTEGER,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS receive_rotation (
    account_id INTEGER NOT NULL,
    wallet_id TEXT NOT NULL REFERENCES wallets(id),
    diversifier_index INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (wallet_id, account_id)
);

CREATE TABLE IF NOT EXISTS _app_migrations (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);
"#;
