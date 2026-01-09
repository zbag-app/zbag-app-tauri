use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use bip39::Mnemonic;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use rand::RngCore as _;
use rand::seq::SliceRandom as _;
use rusqlite::OpenFlags;
use secrecy::SecretVec;
use uuid::Uuid;
use zeroize::Zeroize;

use zkore_core::domain::{
    AccountInfo, AccountType, AddressInfo, AddressType, BackupAction, Balance, Network,
    PrivacyPosture, ShieldAction, SyncPhase, SyncProgress, SyncStatus, WalletInfo,
    WalletLockStatus, WalletStatus, WalletType,
};
use zkore_core::errors;

use crate::birthday;
use crate::db::wallet_encryption_meta::WalletEncryptionMeta;
use crate::db::{AppDb, account_meta, backup_meta, wallet_encryption_meta, wallet_meta};
use crate::encryption::{
    Dek, default_aead_params, default_kdf_params, generate_dek, unwrap_dek, wrap_dek,
};
use crate::error::ipc_err;
use crate::key_store::KeyStore;
use crate::reauth::{ReauthManager, SystemClock};
use crate::tx_service::{TxEventHandler, TxService};
use zcash_client_backend::data_api::{Account as _, WalletRead as _};
use zcash_protocol::consensus::Parameters as _;
use zkore_core::ipc::v1::commands::keystone::{
    BuildSigningRequestResponse, FinalizeSigningResponse,
};
use zkore_core::ipc::v1::commands::transaction::{
    ConfirmSendResponse, ListTransactionsResponse, PrepareSendResponse, ShieldFundsResponse,
};
use zkore_core::ipc::v1::commands::wallet::{BackupChallenge, ReauthPurpose};
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::events::WalletStatusEvent;

pub struct WalletManager {
    app_db: AppDb,
    key_store: Box<dyn KeyStore>,
    wallets_root: PathBuf,
    active_wallet: Option<ActiveWallet>,
    reauth: ReauthManager,
    tx_service: TxService<SystemClock>,
    backup_challenges: HashMap<Uuid, BackupChallengeState>,
    cached_balances: HashMap<(Uuid, u32), Balance>,
    cached_sync_status: HashMap<Uuid, SyncStatus>,
    sync_stop_requested: HashSet<Uuid>,
    last_emitted_status: HashMap<Uuid, WalletStatus>,
    on_wallet_status: Option<Arc<dyn Fn(WalletStatusEvent) + Send + Sync>>,
    wallet_db_force_validate_fail: bool,
}

#[derive(Debug)]
struct ActiveWallet {
    wallet: WalletInfo,
    wallet_dir: PathBuf,
    lock_status: WalletLockStatus,
    dek: Option<Dek>,
    wallet_db: Option<rusqlite::Connection>,
}

#[derive(Debug, Clone)]
pub struct CreateWalletResult {
    pub wallet: WalletInfo,
    pub seed_phrase: Vec<String>,
    pub backup_challenge: BackupChallenge,
}

#[derive(Debug, Clone)]
pub struct RestoreWalletResult {
    pub wallet: WalletInfo,
    pub birthday_height: u32,
}

#[derive(Debug, Clone)]
struct BackupChallengeState {
    challenge_id: String,
    indices: Vec<u8>,
    expires_at: i64,
    failed_attempts: u8,
}

impl WalletManager {
    pub fn new(app_db_path: PathBuf, key_store: Box<dyn KeyStore>) -> anyhow::Result<Self> {
        let wallets_root = default_wallets_root()?;
        Self::new_with_wallets_root(app_db_path, wallets_root, key_store)
    }

    pub fn new_with_wallets_root(
        app_db_path: PathBuf,
        wallets_root: PathBuf,
        key_store: Box<dyn KeyStore>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            app_db: AppDb::open(app_db_path)?,
            key_store,
            wallets_root,
            active_wallet: None,
            reauth: ReauthManager::new(SystemClock),
            tx_service: TxService::new(SystemClock),
            backup_challenges: HashMap::new(),
            cached_balances: HashMap::new(),
            cached_sync_status: HashMap::new(),
            sync_stop_requested: HashSet::new(),
            last_emitted_status: HashMap::new(),
            on_wallet_status: None,
            wallet_db_force_validate_fail: false,
        })
    }

    pub fn list_wallets(&self) -> anyhow::Result<Vec<WalletInfo>> {
        let wallets = wallet_meta::list_wallets(self.app_db.conn())?;
        Ok(wallets)
    }

    pub fn load_wallet(
        &mut self,
        wallet_id: Uuid,
    ) -> anyhow::Result<(WalletInfo, WalletLockStatus)> {
        let Some((wallet, directory_path_str)) =
            wallet_meta::get_wallet(self.app_db.conn(), wallet_id)?
        else {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        };

        let directory_path = PathBuf::from(directory_path_str);
        let expected_dir = self.wallet_dir(wallet.id, wallet.network);
        if directory_path != expected_dir {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet directory does not match network",
            ));
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        wallet_meta::update_last_opened_at(self.app_db.conn(), wallet_id, now_ms)?;

        if let Some(active) = self.active_wallet.as_mut()
            && active.wallet.id == wallet_id
            && active.lock_status == WalletLockStatus::Unlocked
            && active.dek.is_some()
            && active.wallet_db.is_some()
        {
            active.wallet = wallet.clone();
            return Ok((wallet, WalletLockStatus::Unlocked));
        }

        let mut lock_status = WalletLockStatus::Locked;
        let mut dek: Option<Dek> = None;
        let mut wallet_db: Option<rusqlite::Connection> = None;

        if wallet.remember_unlock_enabled
            && let Some(mut material) = self
                .key_store
                .load_keychain_unlock_material(wallet_id, wallet.network)?
        {
            if material.len() == 32 {
                let mut dek_bytes = [0u8; 32];
                dek_bytes.copy_from_slice(&material);
                let candidate_dek = Dek(dek_bytes);
                if let Ok(conn) = self.open_wallet_db_with_dek(
                    &directory_path,
                    wallet.network,
                    &candidate_dek,
                    None,
                    false,
                ) {
                    lock_status = WalletLockStatus::Unlocked;
                    dek = Some(candidate_dek);
                    wallet_db = Some(conn);
                }
            }
            material.zeroize();
        }

        self.active_wallet = Some(ActiveWallet {
            wallet: wallet.clone(),
            wallet_dir: directory_path.clone(),
            lock_status,
            dek,
            wallet_db,
        });

        self.tx_service
            .scan_queued_broadcasts(wallet.id, &directory_path)
            .context("failed to scan queued broadcasts")?;

        self.maybe_emit_wallet_status(wallet.id);

        Ok((wallet, lock_status))
    }

    pub fn create_wallet(
        &mut self,
        name: &str,
        network: Network,
        password: &str,
        remember_unlock: bool,
        birthday_height: Option<u32>,
    ) -> anyhow::Result<CreateWalletResult> {
        if name.trim().is_empty() {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet name must not be empty",
            ));
        }
        if name.chars().count() > 50 {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet name must be at most 50 characters",
            ));
        }
        if password.is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "password required"));
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut entropy = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy(&entropy).map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("failed to generate mnemonic: {e}"),
            )
        })?;
        entropy.zeroize();
        let seed_phrase: Vec<String> = mnemonic.words().map(|w| w.to_string()).collect();
        if seed_phrase.len() != 24 {
            return Err(ipc_err(errors::INTERNAL_ERROR, "mnemonic must be 24 words"));
        }

        let wallet = WalletInfo {
            id: Uuid::new_v4(),
            name: name.to_string(),
            wallet_type: WalletType::Software,
            network,
            remember_unlock_enabled: remember_unlock,
            created_at: now_ms,
            last_opened_at: None,
        };

        let wallet_dir = self.wallet_dir(wallet.id, network);
        std::fs::create_dir_all(&wallet_dir).with_context(|| {
            format!(
                "failed to create wallet directory: {}",
                wallet_dir.display()
            )
        })?;

        let wallet_dir_str = wallet_dir.to_string_lossy().to_string();
        wallet_meta::insert_wallet(self.app_db.conn(), &wallet, &wallet_dir_str)?;
        backup_meta::set_backup_required(self.app_db.conn(), wallet.id, true)?;

        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();
        let wrapped_dek_b64 = wrap_dek(
            wallet.id,
            wallet.network,
            password,
            &kdf.salt_b64,
            &aead.nonce_b64,
            &dek,
        )?;

        wallet_encryption_meta::insert_wallet_encryption(
            self.app_db.conn(),
            wallet.id,
            &WalletEncryptionMeta {
                kdf,
                aead,
                wrapped_dek_b64,
            },
        )?;

        let mut seed_bytes = mnemonic.to_seed_normalized("");
        let seed = SecretVec::new(seed_bytes.to_vec());
        seed_bytes.zeroize();

        let mut conn = self
            .open_wallet_db_with_dek(&wallet_dir, network, &dek, Some(seed), true)
            .context("failed to initialize encrypted wallet db")?;

        {
            use zcash_client_backend::data_api::chain::ChainState;
            use zcash_client_backend::data_api::{AccountBirthday, WalletWrite as _};
            use zcash_primitives::block::BlockHash;
            use zcash_protocol::consensus::NetworkUpgrade;
            use zcash_protocol::consensus::Parameters as _;

            let params = zcash_consensus_network(network);
            let sapling_activation = params
                .activation_height(NetworkUpgrade::Sapling)
                .unwrap_or(zcash_protocol::consensus::H0);

            // For new wallets: use provided birthday_height (near chain tip) if available,
            // otherwise fall back to Sapling activation (for tests or offline creation).
            // Ensure birthday is at least Sapling activation height.
            let birthday_height_u32 = birthday_height
                .unwrap_or(u32::from(sapling_activation))
                .max(u32::from(sapling_activation));

            let birthday = AccountBirthday::from_parts(
                ChainState::empty(
                    zcash_protocol::consensus::BlockHeight::from(
                        birthday_height_u32.saturating_sub(1),
                    ),
                    BlockHash([0; 32]),
                ),
                None,
            );

            let mut seed_bytes = mnemonic.to_seed_normalized("");
            let seed = SecretVec::new(seed_bytes.to_vec());
            seed_bytes.zeroize();

            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );
            let _ = wdb
                .create_account(&wallet.name, &seed, &birthday, None)
                .context("failed to create wallet account")?;

            wdb.update_chain_tip(birthday.height())
                .context("failed to set initial chain tip")?;
        }

        let mut mnemonic_phrase = seed_phrase.join(" ");
        let encrypted_mnemonic =
            encrypt_mnemonic(wallet.id, wallet.network, &dek, mnemonic_phrase.as_bytes())
                .context("failed to encrypt mnemonic")?;
        mnemonic_phrase.zeroize();

        self.key_store
            .store_encrypted_mnemonic(wallet.id, wallet.network, &encrypted_mnemonic)
            .context("failed to store encrypted mnemonic")?;

        account_meta::upsert_account(
            self.app_db.conn(),
            wallet.id,
            &AccountInfo {
                id: 0,
                name: wallet.name.clone(),
                account_type: AccountType::Software,
            },
            now_ms,
        )
        .context("failed to insert account metadata")?;

        if remember_unlock {
            self.key_store
                .store_keychain_unlock_material(wallet.id, network, &dek.0)
                .context("failed to store keychain unlock material")?;
        }

        let backup_challenge = self.issue_backup_challenge(wallet.id)?;

        self.active_wallet = Some(ActiveWallet {
            wallet: wallet.clone(),
            wallet_dir: wallet_dir.clone(),
            lock_status: WalletLockStatus::Unlocked,
            dek: Some(dek),
            wallet_db: Some(conn),
        });

        self.tx_service
            .scan_queued_broadcasts(wallet.id, &wallet_dir)
            .context("failed to scan queued broadcasts")?;

        self.maybe_emit_wallet_status(wallet.id);

        Ok(CreateWalletResult {
            wallet,
            seed_phrase,
            backup_challenge,
        })
    }

    pub fn restore_wallet(
        &mut self,
        name: &str,
        network: Network,
        password: &str,
        remember_unlock: bool,
        seed_phrase: &str,
        birthday_date_ms: Option<i64>,
    ) -> anyhow::Result<RestoreWalletResult> {
        if name.trim().is_empty() {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet name must not be empty",
            ));
        }
        if name.chars().count() > 50 {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet name must be at most 50 characters",
            ));
        }
        if password.is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "password required"));
        }

        let mut phrase = seed_phrase.trim().to_string();
        let mnemonic = Mnemonic::parse_in_normalized(bip39::Language::English, phrase.as_str())
            .map_err(|_e| ipc_err(errors::INVALID_SEED_PHRASE, "invalid seed phrase"))?;
        if mnemonic.words().count() != 24 {
            return Err(ipc_err(
                errors::INVALID_SEED_PHRASE,
                "seed phrase must be 24 words",
            ));
        }
        phrase.zeroize();

        let now_ms = chrono::Utc::now().timestamp_millis();

        use zcash_protocol::consensus::NetworkUpgrade;
        use zcash_protocol::consensus::Parameters as _;

        let sapling_activation = zcash_consensus_network(network)
            .activation_height(NetworkUpgrade::Sapling)
            .unwrap_or(zcash_protocol::consensus::H0);
        let sapling_activation_u32 = u32::from(sapling_activation);

        let birthday_height = birthday_date_ms
            .map(|ms| birthday::estimate_birthday_height(network, ms))
            .unwrap_or(sapling_activation_u32)
            .max(sapling_activation_u32);

        let wallet = WalletInfo {
            id: Uuid::new_v4(),
            name: name.to_string(),
            wallet_type: WalletType::Software,
            network,
            remember_unlock_enabled: remember_unlock,
            created_at: now_ms,
            last_opened_at: None,
        };

        let wallet_dir = self.wallet_dir(wallet.id, network);
        std::fs::create_dir_all(&wallet_dir).with_context(|| {
            format!(
                "failed to create wallet directory: {}",
                wallet_dir.display()
            )
        })?;

        let wallet_dir_str = wallet_dir.to_string_lossy().to_string();
        wallet_meta::insert_wallet(self.app_db.conn(), &wallet, &wallet_dir_str)?;
        backup_meta::mark_backup_complete(
            self.app_db.conn(),
            wallet.id,
            now_ms,
            "restore_seed_phrase",
        )?;

        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();
        let wrapped_dek_b64 = wrap_dek(
            wallet.id,
            wallet.network,
            password,
            &kdf.salt_b64,
            &aead.nonce_b64,
            &dek,
        )?;

        wallet_encryption_meta::insert_wallet_encryption(
            self.app_db.conn(),
            wallet.id,
            &WalletEncryptionMeta {
                kdf,
                aead,
                wrapped_dek_b64,
            },
        )?;

        let mut seed_bytes = mnemonic.to_seed_normalized("");
        let seed = SecretVec::new(seed_bytes.to_vec());
        seed_bytes.zeroize();

        let mut conn = self
            .open_wallet_db_with_dek(&wallet_dir, network, &dek, Some(seed), true)
            .context("failed to initialize encrypted wallet db")?;

        {
            use zcash_client_backend::data_api::chain::ChainState;
            use zcash_client_backend::data_api::{AccountBirthday, WalletWrite as _};
            use zcash_primitives::block::BlockHash;

            let params = zcash_consensus_network(network);
            let birthday_state = ChainState::empty(
                zcash_protocol::consensus::BlockHeight::from(birthday_height.saturating_sub(1)),
                BlockHash([0; 32]),
            );
            let birthday = AccountBirthday::from_parts(birthday_state, None);

            let mut seed_bytes = mnemonic.to_seed_normalized("");
            let seed = SecretVec::new(seed_bytes.to_vec());
            seed_bytes.zeroize();

            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let _ = wdb
                .create_account(&wallet.name, &seed, &birthday, None)
                .context("failed to create wallet account")?;

            wdb.update_chain_tip(birthday.height())
                .context("failed to set initial chain tip")?;
        }

        let mut normalized_phrase = mnemonic.words().collect::<Vec<_>>().join(" ");
        let encrypted_mnemonic = encrypt_mnemonic(
            wallet.id,
            wallet.network,
            &dek,
            normalized_phrase.as_bytes(),
        )
        .context("failed to encrypt mnemonic")?;
        normalized_phrase.zeroize();

        self.key_store
            .store_encrypted_mnemonic(wallet.id, wallet.network, &encrypted_mnemonic)
            .context("failed to store encrypted mnemonic")?;

        account_meta::upsert_account(
            self.app_db.conn(),
            wallet.id,
            &AccountInfo {
                id: 0,
                name: wallet.name.clone(),
                account_type: AccountType::Software,
            },
            now_ms,
        )
        .context("failed to insert account metadata")?;

        if remember_unlock {
            self.key_store
                .store_keychain_unlock_material(wallet.id, network, &dek.0)
                .context("failed to store keychain unlock material")?;
        }

        self.active_wallet = Some(ActiveWallet {
            wallet: wallet.clone(),
            wallet_dir: wallet_dir.clone(),
            lock_status: WalletLockStatus::Unlocked,
            dek: Some(dek),
            wallet_db: Some(conn),
        });

        self.tx_service
            .scan_queued_broadcasts(wallet.id, &wallet_dir)
            .context("failed to scan queued broadcasts")?;

        self.maybe_emit_wallet_status(wallet.id);

        Ok(RestoreWalletResult {
            wallet,
            birthday_height,
        })
    }

    /// Create a standalone Keystone hardware wallet from a UFVK.
    ///
    /// Unlike software wallets, this does NOT generate a mnemonic. The UFVK provides
    /// view-only access; spending requires the Keystone signing flow.
    pub fn create_keystone_wallet(
        &mut self,
        name: &str,
        network: Network,
        password: &str,
        remember_unlock: bool,
        ufvk: &str,
        birthday_height: Option<u32>,
        seed_fingerprint: Option<&str>,
        zip32_account_index: Option<u32>,
    ) -> anyhow::Result<(WalletInfo, AccountInfo)> {
        if name.trim().is_empty() {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet name must not be empty",
            ));
        }
        if name.chars().count() > 50 {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet name must be at most 50 characters",
            ));
        }
        if password.is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "password required"));
        }

        // Parse and validate UFVK
        let parsed = zkore_keystone::ufvk::parse_ufvk(ufvk)
            .map_err(|err| ipc_err(errors::INVALID_UFVK, err.to_string()))?;

        let expected_net = zcash_consensus_network(network).network_type();
        if parsed.network != expected_net {
            return Err(ipc_err(errors::INVALID_UFVK, "UFVK network mismatch"));
        }

        let now_ms = chrono::Utc::now().timestamp_millis();

        let wallet = WalletInfo {
            id: Uuid::new_v4(),
            name: name.to_string(),
            wallet_type: WalletType::WatchOnly,
            network,
            remember_unlock_enabled: remember_unlock,
            created_at: now_ms,
            last_opened_at: None,
        };

        let wallet_dir = self.wallet_dir(wallet.id, network);
        std::fs::create_dir_all(&wallet_dir).with_context(|| {
            format!(
                "failed to create wallet directory: {}",
                wallet_dir.display()
            )
        })?;

        let wallet_dir_str = wallet_dir.to_string_lossy().to_string();
        wallet_meta::insert_wallet(self.app_db.conn(), &wallet, &wallet_dir_str)?;

        // Watch-only wallets don't have a seed to back up, so mark backup complete immediately
        backup_meta::mark_backup_complete(self.app_db.conn(), wallet.id, now_ms, "keystone_only")?;

        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();
        let wrapped_dek_b64 = wrap_dek(
            wallet.id,
            wallet.network,
            password,
            &kdf.salt_b64,
            &aead.nonce_b64,
            &dek,
        )?;

        wallet_encryption_meta::insert_wallet_encryption(
            self.app_db.conn(),
            wallet.id,
            &WalletEncryptionMeta {
                kdf,
                aead,
                wrapped_dek_b64,
            },
        )?;

        // Open wallet db without seed (watch-only)
        let mut conn = self
            .open_wallet_db_with_dek(&wallet_dir, network, &dek, None, true)
            .context("failed to initialize encrypted wallet db")?;

        // Import UFVK as account_id=0
        let account = {
            use zcash_client_backend::data_api::chain::ChainState;
            #[allow(deprecated)]
            use zcash_client_backend::data_api::{
                AccountBirthday, AccountPurpose, WalletWrite as _, Zip32Derivation,
            };
            use zcash_primitives::block::BlockHash;
            use zcash_protocol::consensus::NetworkUpgrade;
            use zcash_protocol::consensus::Parameters as _;
            use zip32::fingerprint::SeedFingerprint;

            let params = zcash_consensus_network(network);
            let sapling_activation = params
                .activation_height(NetworkUpgrade::Sapling)
                .unwrap_or(zcash_protocol::consensus::H0);

            // Use provided birthday_height if available, otherwise default to Sapling activation
            // (slower but correct for existing Keystone wallets that may have transaction history)
            let birthday_height_u32 = birthday_height
                .unwrap_or(u32::from(sapling_activation))
                .max(u32::from(sapling_activation));

            let birthday = AccountBirthday::from_parts(
                ChainState::empty(
                    zcash_protocol::consensus::BlockHeight::from(
                        birthday_height_u32.saturating_sub(1),
                    ),
                    BlockHash([0; 32]),
                ),
                None,
            );

            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            // Build account purpose with derivation info for Keystone signing.
            // This enables PCZT to include zip32_derivation fields so Keystone knows which key to use.
            let account_purpose = match (seed_fingerprint, zip32_account_index) {
                (Some(fp_hex), Some(idx)) => {
                    let fp_bytes = hex::decode(fp_hex).map_err(|e| {
                        ipc_err(
                            errors::INVALID_REQUEST,
                            format!("invalid seed_fingerprint hex: {e}"),
                        )
                    })?;
                    if fp_bytes.len() != 32 {
                        return Err(ipc_err(
                            errors::INVALID_REQUEST,
                            "seed_fingerprint must be 32 bytes",
                        ));
                    }
                    let mut fp_arr = [0u8; 32];
                    fp_arr.copy_from_slice(&fp_bytes);
                    let seed_fp = SeedFingerprint::from_bytes(fp_arr);
                    let account_index = zip32::AccountId::try_from(idx).map_err(|_| {
                        ipc_err(errors::INVALID_REQUEST, "invalid zip32_account_index")
                    })?;
                    let derivation = Zip32Derivation::new(seed_fp, account_index);
                    AccountPurpose::Spending {
                        derivation: Some(derivation),
                    }
                }
                _ => {
                    // Fallback: import without derivation info (signing may not work)
                    AccountPurpose::Spending { derivation: None }
                }
            };

            // Use account_id=0 directly for the first account
            let key_source = crate::account_key_source::key_source_for_account_id(0);
            let _ = wdb
                .import_account_ufvk(
                    name,
                    &parsed.ufvk,
                    &birthday,
                    account_purpose,
                    Some(&key_source),
                )
                .context("failed to import UFVK into wallet db")?;

            wdb.update_chain_tip(birthday.height())
                .context("failed to set initial chain tip")?;

            AccountInfo {
                id: 0,
                name: name.to_string(),
                account_type: AccountType::HardwareSigner,
            }
        };

        account_meta::upsert_account(self.app_db.conn(), wallet.id, &account, now_ms)
            .context("failed to insert account metadata")?;

        if remember_unlock {
            self.key_store
                .store_keychain_unlock_material(wallet.id, network, &dek.0)
                .context("failed to store keychain unlock material")?;
        }

        self.active_wallet = Some(ActiveWallet {
            wallet: wallet.clone(),
            wallet_dir: wallet_dir.clone(),
            lock_status: WalletLockStatus::Unlocked,
            dek: Some(dek),
            wallet_db: Some(conn),
        });

        self.tx_service
            .scan_queued_broadcasts(wallet.id, &wallet_dir)
            .context("failed to scan queued broadcasts")?;

        self.maybe_emit_wallet_status(wallet.id);

        Ok((wallet, account))
    }

    pub fn unlock_wallet(
        &mut self,
        wallet_id: Uuid,
        password: &str,
        remember_unlock: bool,
    ) -> anyhow::Result<WalletLockStatus> {
        let Some((wallet, directory_path_str)) =
            wallet_meta::get_wallet(self.app_db.conn(), wallet_id)?
        else {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        };
        let directory_path = PathBuf::from(directory_path_str);
        let expected_dir = self.wallet_dir(wallet.id, wallet.network);
        if directory_path != expected_dir {
            return Err(ipc_err(
                errors::INVALID_REQUEST,
                "wallet directory does not match network",
            ));
        }

        let Some(meta) =
            wallet_encryption_meta::get_wallet_encryption(self.app_db.conn(), wallet_id)?
        else {
            return Err(ipc_err(
                errors::INTERNAL_ERROR,
                "wallet encryption metadata not found",
            ));
        };

        let dek = unwrap_dek(
            wallet_id,
            wallet.network,
            password,
            &meta.kdf.salt_b64,
            &meta.aead.nonce_b64,
            &meta.wrapped_dek_b64,
        )
        .map_err(|_e| ipc_err(errors::INVALID_WALLET_PASSWORD, "invalid wallet password"))?;

        let conn = self
            .open_wallet_db_with_dek(&directory_path, wallet.network, &dek, None, false)
            .context("failed to open wallet db")?;

        wallet_meta::set_remember_unlock_enabled(self.app_db.conn(), wallet_id, remember_unlock)?;
        if remember_unlock {
            self.key_store
                .store_keychain_unlock_material(wallet_id, wallet.network, &dek.0)
                .context("failed to store keychain unlock material")?;
        } else if let Err(e) = self
            .key_store
            .delete_keychain_unlock_material(wallet_id, wallet.network)
        {
            tracing::warn!(
                wallet_id = %wallet_id,
                error = ?e,
                "failed to delete keychain entry - may require manual cleanup in OS keychain"
            );
        }

        self.active_wallet = Some(ActiveWallet {
            wallet: wallet.clone(),
            wallet_dir: directory_path.clone(),
            lock_status: WalletLockStatus::Unlocked,
            dek: Some(dek),
            wallet_db: Some(conn),
        });

        self.tx_service
            .scan_queued_broadcasts(wallet.id, &directory_path)
            .context("failed to scan queued broadcasts")?;

        self.maybe_emit_wallet_status(wallet.id);

        Ok(WalletLockStatus::Unlocked)
    }

    pub fn lock_wallet(&mut self, wallet_id: Uuid) -> anyhow::Result<WalletLockStatus> {
        let Some(active) = self.active_wallet.as_mut() else {
            return Ok(WalletLockStatus::Locked);
        };
        if active.wallet.id != wallet_id {
            return Ok(active.lock_status);
        }

        active.wallet_db = None;
        active.dek = None;
        active.lock_status = WalletLockStatus::Locked;
        self.maybe_emit_wallet_status(wallet_id);
        Ok(WalletLockStatus::Locked)
    }

    /// Logout from the active wallet completely.
    /// This differs from lock_wallet() in that it:
    /// 1. Clears all cached state for the wallet (balances, sync status, proposals)
    /// 2. Sets active_wallet to None (not just locked)
    /// 3. Clears backup challenges
    ///
    /// Caller MUST stop sync before calling this method.
    pub fn logout_wallet(&mut self, wallet_id: Uuid) -> anyhow::Result<()> {
        // Verify wallet_id matches active wallet
        let Some(active) = self.active_wallet.as_ref() else {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "no active wallet"));
        };
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not active"));
        }

        // Clear wallet_db connection and DEK
        if let Some(active) = self.active_wallet.as_mut() {
            active.wallet_db = None;
            active.dek = None;
        }

        // Clear all cached state for this wallet
        self.cached_balances.retain(|(wid, _), _| *wid != wallet_id);
        self.cached_sync_status.remove(&wallet_id);
        self.sync_stop_requested.remove(&wallet_id);
        self.last_emitted_status.remove(&wallet_id);
        self.backup_challenges.remove(&wallet_id);
        self.tx_service.clear_proposals_for_wallet(wallet_id);

        // Deactivate the wallet
        self.active_wallet = None;
        Ok(())
    }

    pub fn reauth_wallet(
        &mut self,
        wallet_id: Uuid,
        password: &str,
        purpose: ReauthPurpose,
    ) -> anyhow::Result<(String, std::time::SystemTime)> {
        if password.is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "password required"));
        }

        let Some((wallet, _directory_path_str)) =
            wallet_meta::get_wallet(self.app_db.conn(), wallet_id)?
        else {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        };

        let Some(meta) =
            wallet_encryption_meta::get_wallet_encryption(self.app_db.conn(), wallet_id)?
        else {
            return Err(ipc_err(
                errors::INTERNAL_ERROR,
                "wallet encryption metadata not found",
            ));
        };

        // Verify password by unwrapping the DEK; do NOT consult keychain unlock material.
        let _dek = unwrap_dek(
            wallet_id,
            wallet.network,
            password,
            &meta.kdf.salt_b64,
            &meta.aead.nonce_b64,
            &meta.wrapped_dek_b64,
        )
        .map_err(|_e| ipc_err(errors::INVALID_WALLET_PASSWORD, "invalid wallet password"))?;

        let (token, expires_at) = self.reauth.issue(wallet_id, purpose);
        Ok((token, expires_at))
    }

    pub fn view_seed_phrase(
        &mut self,
        wallet_id: Uuid,
        reauth_token: &str,
    ) -> anyhow::Result<Vec<String>> {
        // Check for watch-only wallet early (before consuming reauth token)
        if let Some(active) = self.active_wallet.as_ref()
            && active.wallet.id == wallet_id
            && active.wallet.wallet_type == WalletType::WatchOnly
        {
            return Err(ipc_err(
                errors::WATCH_ONLY_NO_SEED,
                "cannot view seed phrase for watch-only wallet",
            ));
        }

        match self.reauth.validate_and_consume(
            reauth_token,
            wallet_id,
            ReauthPurpose::ViewSeedPhrase,
        ) {
            Ok(()) => {}
            Err(crate::reauth::ReauthError::Invalid) => {
                return Err(ipc_err(
                    errors::REAUTH_TOKEN_INVALID,
                    "reauth token invalid",
                ));
            }
            Err(crate::reauth::ReauthError::Expired) => {
                return Err(ipc_err(
                    errors::REAUTH_TOKEN_EXPIRED,
                    "reauth token expired",
                ));
            }
        }

        let (wallet, dek) = self.require_unlocked_wallet_snapshot(wallet_id)?;
        let phrase = self
            .decrypt_mnemonic(wallet.id, wallet.network, &dek)
            .context("failed to decrypt mnemonic")?;
        Ok(phrase.split_whitespace().map(|w| w.to_string()).collect())
    }

    pub fn consume_reauth_token(
        &mut self,
        wallet_id: Uuid,
        token: &str,
        purpose: ReauthPurpose,
    ) -> anyhow::Result<()> {
        match self.reauth.validate_and_consume(token, wallet_id, purpose) {
            Ok(()) => Ok(()),
            Err(crate::reauth::ReauthError::Invalid) => Err(ipc_err(
                errors::REAUTH_TOKEN_INVALID,
                "reauth token invalid",
            )),
            Err(crate::reauth::ReauthError::Expired) => Err(ipc_err(
                errors::REAUTH_TOKEN_EXPIRED,
                "reauth token expired",
            )),
        }
    }

    pub(crate) fn derive_unified_spending_key(
        &mut self,
        wallet_id: Uuid,
        account_id: u32,
    ) -> anyhow::Result<zcash_client_backend::keys::UnifiedSpendingKey> {
        let (wallet, dek) = self.require_unlocked_wallet_snapshot(wallet_id)?;

        let mut phrase = self
            .decrypt_mnemonic(wallet.id, wallet.network, &dek)
            .context("failed to decrypt mnemonic")?;

        let mnemonic = Mnemonic::parse_in_normalized(bip39::Language::English, phrase.trim())
            .map_err(|e| {
                ipc_err(
                    errors::INTERNAL_ERROR,
                    format!("invalid stored mnemonic: {e}"),
                )
            })?;
        phrase.zeroize();

        let mut seed_bytes = mnemonic.to_seed_normalized("");
        let account = zip32::AccountId::try_from(account_id)
            .map_err(|_| ipc_err(errors::INVALID_REQUEST, "invalid account_id"))?;
        let params = zcash_consensus_network(wallet.network);
        let usk = zcash_client_backend::keys::UnifiedSpendingKey::from_seed(
            &params,
            &seed_bytes,
            account,
        )
        .map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("failed to derive spending key: {e}"),
            )
        })?;
        seed_bytes.zeroize();

        Ok(usk)
    }

    pub fn get_backup_challenge(&mut self, wallet_id: Uuid) -> anyhow::Result<BackupChallenge> {
        // Check for watch-only wallet (no seed to back up)
        if let Some(active) = self.active_wallet.as_ref()
            && active.wallet.id == wallet_id
            && active.wallet.wallet_type == WalletType::WatchOnly
        {
            return Err(ipc_err(
                errors::WATCH_ONLY_NO_BACKUP,
                "no backup required for watch-only wallet",
            ));
        }
        self.issue_backup_challenge(wallet_id)
    }

    pub fn unlocked_wallet_dek(&self, wallet_id: Uuid) -> anyhow::Result<Dek> {
        let Some(active) = self.active_wallet.as_ref() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        }
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(dek) = active.dek.as_ref() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        Ok(Dek(dek.0))
    }

    pub fn verify_backup(
        &mut self,
        wallet_id: Uuid,
        challenge_id: &str,
        word_challenges: &HashMap<u8, String>,
    ) -> anyhow::Result<()> {
        // Check for watch-only wallet (no seed to back up)
        if let Some(active) = self.active_wallet.as_ref()
            && active.wallet.id == wallet_id
            && active.wallet.wallet_type == WalletType::WatchOnly
        {
            return Err(ipc_err(
                errors::WATCH_ONLY_NO_BACKUP,
                "no backup required for watch-only wallet",
            ));
        }

        let now_ms = chrono::Utc::now().timestamp_millis();

        let indices = {
            let Some(state) = self.backup_challenges.get(&wallet_id) else {
                return Err(ipc_err(
                    errors::BACKUP_CHALLENGE_INVALID,
                    "backup challenge invalid",
                ));
            };

            if state.challenge_id != challenge_id {
                return Err(ipc_err(
                    errors::BACKUP_CHALLENGE_INVALID,
                    "backup challenge invalid",
                ));
            }

            if now_ms > state.expires_at {
                self.backup_challenges.remove(&wallet_id);
                return Err(ipc_err(
                    errors::BACKUP_CHALLENGE_EXPIRED,
                    "backup challenge expired",
                ));
            }

            if state.failed_attempts >= 5 {
                self.backup_challenges.remove(&wallet_id);
                return Err(ipc_err(
                    errors::BACKUP_CHALLENGE_TOO_MANY_ATTEMPTS,
                    "too many failed attempts",
                ));
            }

            state.indices.clone()
        };

        let (wallet, dek) = self.require_unlocked_wallet_snapshot(wallet_id)?;
        let phrase = self
            .decrypt_mnemonic(wallet.id, wallet.network, &dek)
            .context("failed to decrypt mnemonic")?;
        let words: Vec<&str> = phrase.split_whitespace().collect();
        if words.len() != 24 {
            return Err(ipc_err(errors::INTERNAL_ERROR, "invalid stored mnemonic"));
        }

        let mut ok = true;
        for index in &indices {
            let expected = words
                .get((*index as usize).saturating_sub(1))
                .copied()
                .unwrap_or_default();
            let actual = word_challenges
                .get(index)
                .map(|w| w.trim().to_lowercase())
                .unwrap_or_default();
            if expected != actual {
                ok = false;
            }
        }

        if ok {
            backup_meta::mark_backup_complete(self.app_db.conn(), wallet_id, now_ms, "challenge")?;
            self.backup_challenges.remove(&wallet_id);
            self.maybe_emit_wallet_status(wallet_id);
            return Ok(());
        }

        let Some(state) = self.backup_challenges.get_mut(&wallet_id) else {
            return Err(ipc_err(
                errors::BACKUP_CHALLENGE_INVALID,
                "backup challenge invalid",
            ));
        };

        state.failed_attempts = state.failed_attempts.saturating_add(1);
        if state.failed_attempts >= 5 {
            self.backup_challenges.remove(&wallet_id);
            return Err(ipc_err(
                errors::BACKUP_CHALLENGE_TOO_MANY_ATTEMPTS,
                "too many failed attempts",
            ));
        }
        Err(ipc_err(
            errors::BACKUP_CHALLENGE_INVALID,
            "backup challenge invalid",
        ))
    }

    pub fn compute_wallet_status(&mut self, wallet_id: Uuid) -> anyhow::Result<WalletStatus> {
        let Some((wallet, _dir)) = wallet_meta::get_wallet(self.app_db.conn(), wallet_id)? else {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        };

        let lock_status = match self.active_wallet.as_ref() {
            Some(active) if active.wallet.id == wallet_id => active.lock_status,
            _ => WalletLockStatus::Locked,
        };

        let backup_required =
            backup_meta::get_backup_required(self.app_db.conn(), wallet.id)?.unwrap_or(true);

        let backup_status = if backup_required {
            BackupAction::Required
        } else {
            BackupAction::Complete
        };

        let sync_status = self
            .cached_sync_status
            .get(&wallet_id)
            .cloned()
            .unwrap_or_else(|| {
                tracing::debug!("sync status unknown, defaulting to Synced");
                SyncStatus::Synced
            });

        let transparent_total_zat =
            self.cached_transparent_total_zat(wallet_id)
                .unwrap_or_else(|| {
                    match self.transparent_total_from_wallet_db(wallet_id) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(wallet_id = %wallet_id, error = ?e, "failed to get transparent balance, defaulting to 0");
                            0
                        }
                    }
                });

        let shield_status = if transparent_total_zat > 0 {
            ShieldAction::Available {
                amount: transparent_total_zat.to_string(),
            }
        } else {
            ShieldAction::None
        };

        let privacy_posture = if backup_required || transparent_total_zat > 0 {
            PrivacyPosture::NeedsAction
        } else {
            PrivacyPosture::Optimal
        };

        Ok(WalletStatus {
            lock_status,
            backup_status,
            sync_status,
            shield_status,
            privacy_posture,
        })
    }

    pub fn set_wallet_status_handler(
        &mut self,
        handler: Arc<dyn Fn(WalletStatusEvent) + Send + Sync>,
    ) {
        self.on_wallet_status = Some(handler);
    }

    pub fn observe_sync_progress(&mut self, wallet_id: Uuid, progress: SyncProgress) {
        let prev = self.cached_sync_status.get(&wallet_id).cloned();

        let next = match progress.phase {
            SyncPhase::Idle => {
                if progress.progress_percent >= 100 {
                    self.sync_stop_requested.remove(&wallet_id);
                    SyncStatus::Synced
                } else if self.sync_stop_requested.remove(&wallet_id) {
                    SyncStatus::Synced
                } else if matches!(prev, Some(SyncStatus::Syncing { .. })) {
                    SyncStatus::Error {
                        message: "sync failed".to_string(),
                    }
                } else {
                    SyncStatus::Synced
                }
            }
            SyncPhase::CatchingUp => {
                if progress.wallet_tip_height > 0
                    && progress.scan_frontier_height >= progress.wallet_tip_height
                {
                    self.sync_stop_requested.remove(&wallet_id);
                    SyncStatus::Synced
                } else {
                    SyncStatus::Syncing {
                        progress_percent: progress.progress_percent,
                    }
                }
            }
            _ => SyncStatus::Syncing {
                progress_percent: progress.progress_percent,
            },
        };

        self.cached_sync_status.insert(wallet_id, next);
        self.maybe_emit_wallet_status(wallet_id);
    }

    pub fn observe_sync_stop_requested(&mut self, wallet_id: Uuid) {
        self.sync_stop_requested.insert(wallet_id);
    }

    pub fn observe_balance_changed(&mut self, wallet_id: Uuid, account_id: u32, balance: Balance) {
        self.cached_balances
            .insert((wallet_id, account_id), balance);
        self.maybe_emit_wallet_status(wallet_id);
    }

    fn cached_transparent_total_zat(&self, wallet_id: Uuid) -> Option<u64> {
        let mut total: u64 = 0;
        let mut seen = false;

        for ((wid, _account_id), bal) in &self.cached_balances {
            if *wid != wallet_id {
                continue;
            }
            seen = true;
            let value: u64 = bal.transparent_total.parse().unwrap_or(0);
            total = total.saturating_add(value);
        }

        if seen { Some(total) } else { None }
    }

    fn transparent_total_from_wallet_db(&mut self, wallet_id: Uuid) -> anyhow::Result<u64> {
        let (wallet, conn) = match self.require_unlocked_wallet_db(wallet_id) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::debug!(error = ?e, "wallet not unlocked, returning 0");
                return Ok(0);
            }
        };

        #[allow(deprecated)]
        use zcash_client_backend::data_api::WalletRead as _;

        let params = zcash_consensus_network(wallet.network);
        let wdb = zcash_client_sqlite::WalletDb::from_connection(
            conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let summary = wdb
            .get_wallet_summary(
                zcash_client_backend::data_api::wallet::ConfirmationsPolicy::default(),
            )
            .context("failed to compute wallet summary")?;

        let Some(summary) = summary else {
            return Ok(0);
        };

        let mut total: u64 = 0;
        for balance in summary.account_balances().values() {
            total = total.saturating_add(balance.unshielded_balance().total().into_u64());
        }

        Ok(total)
    }

    fn maybe_emit_wallet_status(&mut self, wallet_id: Uuid) {
        let status = match self.compute_wallet_status(wallet_id) {
            Ok(status) => status,
            Err(e) => {
                tracing::debug!(wallet_id = %wallet_id, error = ?e, "failed to compute wallet status");
                return;
            }
        };

        let changed = match self.last_emitted_status.get(&wallet_id) {
            Some(prev) => prev != &status,
            None => true,
        };
        if !changed {
            return;
        }

        self.last_emitted_status.insert(wallet_id, status.clone());

        let Some(handler) = self.on_wallet_status.as_ref() else {
            return;
        };

        handler(WalletStatusEvent {
            schema_version: SCHEMA_VERSION,
            event: "wallet.status".to_string(),
            status,
        });
    }

    pub fn list_wallet_db_account_ids(&mut self, wallet_id: Uuid) -> anyhow::Result<Vec<u32>> {
        let (wallet, conn) = self.require_unlocked_wallet_db(wallet_id)?;
        let params = zcash_consensus_network(wallet.network);
        let wdb = zcash_client_sqlite::WalletDb::from_connection(
            conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );
        let account_uuids = wdb
            .get_account_ids()
            .context("failed to list wallet accounts")?;
        let mut account_indices = Vec::with_capacity(account_uuids.len());
        for account_uuid in account_uuids {
            let Some(account) = wdb
                .get_account(account_uuid)
                .context("failed to load wallet account")?
            else {
                continue;
            };

            // Check key_source first (software wallets, Zkore-tagged imports including HardwareSigner)
            if let Some(key_source) = account.source().key_source()
                && let Some(account_id) =
                    crate::account_key_source::parse_account_id_from_key_source(key_source)
            {
                account_indices.push(account_id);
            }
            // Then check key_derivation (hardware wallets with ZIP-32 derivation, only if no key_source)
            else if let Some(derivation) = account.source().key_derivation() {
                let account_index: u32 = derivation.account_index().into();
                account_indices.push(account_index);
            }
        }
        account_indices.sort_unstable();
        account_indices.dedup();
        Ok(account_indices)
    }

    pub fn import_ufvk(
        &mut self,
        wallet_id: Uuid,
        ufvk: &str,
        name: &str,
        seed_fingerprint: Option<&str>,
        zip32_account_index: Option<u32>,
    ) -> anyhow::Result<AccountInfo> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "account name required"));
        }

        let (wallet, conn) = self.require_unlocked_wallet_db(wallet_id)?;

        let parsed = zkore_keystone::ufvk::parse_ufvk(ufvk)
            .map_err(|err| ipc_err(errors::INVALID_UFVK, err.to_string()))?;

        let expected_net = zcash_consensus_network(wallet.network).network_type();
        if parsed.network != expected_net {
            return Err(ipc_err(errors::INVALID_UFVK, "UFVK network mismatch"));
        }

        #[allow(deprecated)]
        use zcash_client_backend::data_api::Account as _;
        #[allow(deprecated)]
        use zcash_client_backend::data_api::WalletRead as _;

        let params = zcash_consensus_network(wallet.network);
        let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
            conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let mut used_ids = std::collections::HashSet::<u32>::new();
        for account_uuid in wdb
            .get_account_ids()
            .context("failed to list wallet accounts")?
        {
            let Some(account) = wdb
                .get_account(account_uuid)
                .context("failed to load wallet account")?
            else {
                continue;
            };

            // Collect BOTH key_source and key_derivation IDs to avoid any collision.
            // An account may have both (e.g., Keystone accounts with key_source="zkore:N"
            // AND key_derivation.account_index=M where N != M).
            if let Some(key_source) = account.source().key_source()
                && let Some(id) =
                    crate::account_key_source::parse_account_id_from_key_source(key_source)
            {
                used_ids.insert(id);
            }
            if let Some(derivation) = account.source().key_derivation() {
                used_ids.insert(derivation.account_index().into());
            }
        }

        let next_account_id = used_ids
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        use zcash_client_backend::data_api::chain::ChainState;
        #[allow(deprecated)]
        use zcash_client_backend::data_api::{
            AccountBirthday, AccountPurpose, WalletWrite as _, Zip32Derivation,
        };
        use zcash_primitives::block::BlockHash;
        use zcash_protocol::consensus::NetworkUpgrade;
        use zcash_protocol::consensus::Parameters as _;
        use zip32::fingerprint::SeedFingerprint;

        let sapling_activation = params
            .activation_height(NetworkUpgrade::Sapling)
            .unwrap_or(zcash_protocol::consensus::H0);
        let birthday = AccountBirthday::from_parts(
            ChainState::empty(sapling_activation.saturating_sub(1), BlockHash([0; 32])),
            None,
        );

        // Build account purpose with derivation info for Keystone signing.
        // This enables PCZT to include zip32_derivation fields so Keystone knows which key to use.
        let account_purpose = match (seed_fingerprint, zip32_account_index) {
            (Some(fp_hex), Some(idx)) => {
                let fp_bytes = hex::decode(fp_hex).map_err(|e| {
                    ipc_err(
                        errors::INVALID_REQUEST,
                        format!("invalid seed_fingerprint hex: {e}"),
                    )
                })?;
                if fp_bytes.len() != 32 {
                    return Err(ipc_err(
                        errors::INVALID_REQUEST,
                        "seed_fingerprint must be 32 bytes",
                    ));
                }
                let mut fp_arr = [0u8; 32];
                fp_arr.copy_from_slice(&fp_bytes);
                let seed_fp = SeedFingerprint::from_bytes(fp_arr);
                let account_index = zip32::AccountId::try_from(idx)
                    .map_err(|_| ipc_err(errors::INVALID_REQUEST, "invalid zip32_account_index"))?;
                let derivation = Zip32Derivation::new(seed_fp, account_index);
                AccountPurpose::Spending {
                    derivation: Some(derivation),
                }
            }
            _ => {
                // Fallback: import without derivation info (signing may not work)
                AccountPurpose::Spending { derivation: None }
            }
        };

        let key_source = crate::account_key_source::key_source_for_account_id(next_account_id);
        let _ = wdb
            .import_account_ufvk(
                name,
                &parsed.ufvk,
                &birthday,
                account_purpose,
                Some(&key_source),
            )
            .context("failed to import UFVK into wallet db")?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let account = AccountInfo {
            id: next_account_id,
            name: name.to_string(),
            account_type: AccountType::HardwareSigner,
        };
        account_meta::upsert_account(self.app_db.conn(), wallet_id, &account, now_ms)
            .context("failed to insert account metadata")?;

        Ok(account)
    }

    pub fn get_receive_address(
        &mut self,
        account_id: u32,
        address_type: AddressType,
    ) -> anyhow::Result<AddressInfo> {
        let WalletManager {
            app_db,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        crate::address_service::get_receive_address(
            app_db.conn(),
            active.wallet.id,
            conn,
            active.wallet.network,
            account_id,
            address_type,
        )
    }

    pub fn get_balance(&mut self, account_id: u32) -> anyhow::Result<Balance> {
        let (wallet, conn) = self.require_active_unlocked_wallet_db()?;
        crate::balance::get_balance(conn, wallet.network, account_id)
    }

    pub fn prepare_send(
        &mut self,
        account_id: u32,
        recipient: &str,
        amount_zat: &str,
        memo: Option<&str>,
        allow_transparent_recipient: bool,
    ) -> anyhow::Result<PrepareSendResponse> {
        let WalletManager {
            app_db,
            tx_service,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        tx_service.prepare_send(
            app_db,
            active.wallet.id,
            active.wallet.network,
            conn,
            account_id,
            recipient,
            amount_zat,
            memo,
            allow_transparent_recipient,
        )
    }

    pub fn build_signing_request(
        &mut self,
        account_id: u32,
        recipient: &str,
        amount_zat: &str,
        memo: Option<&str>,
        allow_transparent_recipient: bool,
    ) -> anyhow::Result<BuildSigningRequestResponse> {
        let WalletManager {
            app_db,
            tx_service,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        tx_service.build_signing_request(
            app_db,
            active.wallet.id,
            active.wallet.network,
            conn,
            account_id,
            recipient,
            amount_zat,
            memo,
            allow_transparent_recipient,
        )
    }

    pub fn finalize_signing(
        &mut self,
        signing_request_id: &str,
        signed_payload: &str,
        reauth_token: &str,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<FinalizeSigningResponse> {
        let Some(active) = self.active_wallet.as_ref() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        let wallet_id = active.wallet.id;
        let wallet_network = active.wallet.network;
        let wallet_dir = active.wallet_dir.clone();
        let wallet_dek = self.unlocked_wallet_dek(wallet_id)?;

        self.consume_reauth_token(wallet_id, reauth_token, ReauthPurpose::Spend)?;

        let grpc_url = crate::server_resolver::resolve_grpc_url(&self.app_db, wallet_network)
            .context("failed to resolve active lightwalletd endpoint")?;

        let WalletManager {
            app_db,
            tx_service,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        tx_service.finalize_signing(
            app_db,
            wallet_id,
            wallet_network,
            &wallet_dir,
            &wallet_dek,
            conn,
            &grpc_url,
            signing_request_id,
            signed_payload,
            on_tx_changed,
        )
    }

    pub fn confirm_send(
        &mut self,
        proposal_id: &str,
        reauth_token: &str,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<ConfirmSendResponse> {
        let Some(active) = self.active_wallet.as_ref() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        // Watch-only wallets cannot use confirm_send (no seed for signing)
        // Use the hardware signing flow (build_signing_request + finalize_signing) instead
        if active.wallet.wallet_type == WalletType::WatchOnly {
            return Err(ipc_err(
                errors::WATCH_ONLY_CANNOT_SPEND,
                "cannot confirm send for watch-only wallet; use hardware signing flow",
            ));
        }

        let wallet_id = active.wallet.id;
        let wallet_network = active.wallet.network;
        let wallet_dir = active.wallet_dir.clone();
        let wallet_dek = self.unlocked_wallet_dek(wallet_id)?;

        let proposal_account_id = self
            .tx_service
            .proposal_account_id(proposal_id)
            .ok_or_else(|| ipc_err(errors::PROPOSAL_NOT_FOUND, "proposal not found"))?;

        let spending_key = self.derive_unified_spending_key(wallet_id, proposal_account_id)?;

        self.consume_reauth_token(wallet_id, reauth_token, ReauthPurpose::Spend)?;

        let grpc_url = crate::server_resolver::resolve_grpc_url(&self.app_db, wallet_network)
            .context("failed to resolve active lightwalletd endpoint")?;

        let WalletManager {
            app_db,
            tx_service,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        tx_service.confirm_send(
            app_db,
            wallet_id,
            wallet_network,
            &wallet_dir,
            &wallet_dek,
            conn,
            &grpc_url,
            proposal_id,
            spending_key,
            on_tx_changed,
        )
    }

    pub fn cancel_send(&mut self, proposal_id: &str) -> bool {
        self.tx_service.cancel_send(proposal_id)
    }

    pub fn shield_funds(
        &mut self,
        account_id: u32,
        consolidate: bool,
        reauth_token: &str,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<ShieldFundsResponse> {
        let Some(active) = self.active_wallet.as_ref() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        // Shielding is not supported for watch-only wallets
        // (no Keystone signing flow exists for shielding transactions)
        if active.wallet.wallet_type == WalletType::WatchOnly {
            return Err(ipc_err(
                errors::WATCH_ONLY_CANNOT_SHIELD,
                "shielding not supported for watch-only wallet",
            ));
        }

        let wallet_id = active.wallet.id;
        let wallet_network = active.wallet.network;
        let wallet_dir = active.wallet_dir.clone();
        let wallet_dek = self.unlocked_wallet_dek(wallet_id)?;

        self.consume_reauth_token(wallet_id, reauth_token, ReauthPurpose::Spend)?;

        let spending_key = self.derive_unified_spending_key(wallet_id, account_id)?;

        let grpc_url = crate::server_resolver::resolve_grpc_url(&self.app_db, wallet_network)
            .context("failed to resolve active lightwalletd endpoint")?;

        let WalletManager {
            app_db,
            tx_service,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        let resp = tx_service.shield_funds(
            app_db,
            wallet_id,
            wallet_network,
            &wallet_dir,
            &wallet_dek,
            conn,
            &grpc_url,
            account_id,
            consolidate,
            spending_key,
            on_tx_changed,
        )?;
        self.maybe_emit_wallet_status(wallet_id);
        Ok(resp)
    }

    pub fn retry_broadcast(
        &mut self,
        txid: &str,
        reauth_token: &str,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<String> {
        let Some(active_wallet_id) = self.active_wallet.as_ref().map(|w| w.wallet.id) else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        self.consume_reauth_token(active_wallet_id, reauth_token, ReauthPurpose::Spend)?;
        let (wallet_id, wallet_network, wallet_dir) = {
            let Some(active) = self.active_wallet.as_ref() else {
                return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
            };
            (
                active.wallet.id,
                active.wallet.network,
                active.wallet_dir.clone(),
            )
        };

        let wallet_dek = self.unlocked_wallet_dek(wallet_id)?;

        let grpc_url = crate::server_resolver::resolve_grpc_url(&self.app_db, wallet_network)
            .context("failed to resolve active lightwalletd endpoint")?;

        let WalletManager {
            app_db,
            tx_service,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        tx_service.retry_broadcast(
            app_db,
            wallet_id,
            wallet_network,
            &wallet_dir,
            &wallet_dek,
            conn,
            &grpc_url,
            txid,
            on_tx_changed,
        )
    }

    pub fn list_transactions(
        &mut self,
        account_id: u32,
        limit: u32,
        offset: u32,
    ) -> anyhow::Result<ListTransactionsResponse> {
        let (wallet_id, wallet_network, wallet_dir) = {
            let Some(active) = self.active_wallet.as_ref() else {
                return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
            };
            (
                active.wallet.id,
                active.wallet.network,
                active.wallet_dir.clone(),
            )
        };

        let WalletManager {
            tx_service,
            active_wallet,
            ..
        } = self;

        let Some(active) = active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };

        tx_service.list_transactions(
            wallet_id,
            wallet_network,
            &wallet_dir,
            conn,
            account_id,
            limit,
            offset,
        )
    }

    pub fn key_store(&self) -> &dyn KeyStore {
        self.key_store.as_ref()
    }

    pub fn set_tor_manager(&mut self, tor_manager: std::sync::Arc<zkore_tor::TorManager>) {
        self.tx_service.set_tor_manager(tor_manager);
    }

    pub fn app_db(&self) -> &AppDb {
        &self.app_db
    }

    pub fn app_db_mut(&mut self) -> &mut AppDb {
        &mut self.app_db
    }

    pub fn active_wallet_info(&self) -> Option<WalletInfo> {
        self.active_wallet.as_ref().map(|w| w.wallet.clone())
    }

    pub fn ensure_server_network_matches_active_wallet(
        &self,
        server_network: Network,
    ) -> anyhow::Result<()> {
        let Some(active) = self.active_wallet.as_ref() else {
            return Ok(());
        };
        if active.wallet.network != server_network {
            return Err(ipc_err(errors::INVALID_REQUEST, "server network mismatch"));
        }
        Ok(())
    }

    pub fn wallets_root(&self) -> &Path {
        &self.wallets_root
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn __set_wallet_db_force_validate_fail(&mut self, enabled: bool) {
        self.wallet_db_force_validate_fail = enabled;
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn __set_backup_challenge_expires_at(
        &mut self,
        wallet_id: Uuid,
        expires_at_ms: i64,
    ) -> bool {
        let Some(state) = self.backup_challenges.get_mut(&wallet_id) else {
            return false;
        };
        state.expires_at = expires_at_ms;
        true
    }

    fn wallet_dir(&self, wallet_id: Uuid, network: Network) -> PathBuf {
        self.wallets_root
            .join(network_dir_name(network))
            .join(wallet_id.to_string())
    }

    fn wallet_db_path(&self, wallet_dir: &Path) -> PathBuf {
        wallet_dir.join("wallet.sqlite")
    }

    fn wallet_db_snapshot_path(&self, wallet_db_path: &Path) -> PathBuf {
        let mut snapshot = wallet_db_path.as_os_str().to_os_string();
        snapshot.push(".pre_migration");
        PathBuf::from(snapshot)
    }

    fn open_wallet_db_with_dek(
        &self,
        wallet_dir: &Path,
        network: Network,
        dek: &Dek,
        seed: Option<SecretVec<u8>>,
        create_if_missing: bool,
    ) -> anyhow::Result<rusqlite::Connection> {
        let wallet_db_path = self.wallet_db_path(wallet_dir);
        let existed = wallet_db_path.exists();
        let snapshot_path = self.wallet_db_snapshot_path(&wallet_db_path);

        if existed {
            std::fs::copy(&wallet_db_path, &snapshot_path).with_context(|| {
                format!(
                    "failed to create pre-migration snapshot: {} -> {}",
                    wallet_db_path.display(),
                    snapshot_path.display()
                )
            })?;
        }

        let migrate_result = (|| {
            let mut conn =
                self.open_sqlcipher_connection(&wallet_db_path, dek, create_if_missing)?;

            rusqlite::vtab::array::load_module(&conn)
                .context("failed to load sqlite array module")?;

            let params = zcash_consensus_network(network);
            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            zcash_client_sqlite::wallet::init::init_wallet_db(&mut wdb, seed)
                .context("wallet db migration failed")?;

            self.validate_wallet_db(&conn)?;

            Ok::<_, anyhow::Error>(conn)
        })();

        match migrate_result {
            Ok(conn) => {
                if existed {
                    let _ = std::fs::remove_file(&snapshot_path);
                }
                Ok(conn)
            }
            Err(err) => {
                if existed {
                    let restore_result = std::fs::copy(&snapshot_path, &wallet_db_path)
                        .with_context(|| {
                            format!(
                                "failed to restore pre-migration snapshot: {} -> {}",
                                snapshot_path.display(),
                                wallet_db_path.display()
                            )
                        });
                    let _ = std::fs::remove_file(&snapshot_path);
                    if let Err(restore_err) = restore_result {
                        anyhow::bail!("{err}\n{restore_err}");
                    }
                }
                Err(err)
            }
        }
    }

    fn validate_wallet_db(&self, conn: &rusqlite::Connection) -> anyhow::Result<()> {
        let _: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
            .context("wallet db validation query failed")?;

        if self.wallet_db_force_validate_fail {
            anyhow::bail!("wallet db validation forced failure");
        }

        Ok(())
    }

    fn open_sqlcipher_connection(
        &self,
        wallet_db_path: &Path,
        dek: &Dek,
        create_if_missing: bool,
    ) -> anyhow::Result<rusqlite::Connection> {
        let flags = if create_if_missing {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };

        let conn = rusqlite::Connection::open_with_flags(wallet_db_path, flags)
            .with_context(|| format!("failed to open wallet db: {}", wallet_db_path.display()))?;

        let mut dek_hex = dek.0.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let mut pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");
        conn.execute_batch(&pragma)
            .context("failed to apply wallet db encryption key")?;

        dek_hex.zeroize();
        pragma.zeroize();

        // Force an early read to detect an incorrect key.
        let _: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
            .context("wallet db is not readable (incorrect key or corrupted db)")?;

        Ok(conn)
    }

    fn issue_backup_challenge(&mut self, wallet_id: Uuid) -> anyhow::Result<BackupChallenge> {
        // Ensure wallet exists.
        let Some((_wallet, _dir)) = wallet_meta::get_wallet(self.app_db.conn(), wallet_id)? else {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        };

        let mut pool: Vec<u8> = (1u8..=24u8).collect();
        pool.shuffle(&mut rand::thread_rng());
        let mut indices: Vec<u8> = pool.into_iter().take(4).collect();
        indices.sort_unstable();

        let challenge_id = Uuid::new_v4().to_string();
        let expires_at = chrono::Utc::now().timestamp_millis() + 10 * 60 * 1000;

        self.backup_challenges.insert(
            wallet_id,
            BackupChallengeState {
                challenge_id: challenge_id.clone(),
                indices: indices.clone(),
                expires_at,
                failed_attempts: 0,
            },
        );

        Ok(BackupChallenge {
            challenge_id,
            indices,
            expires_at,
        })
    }

    fn require_unlocked_wallet_snapshot(
        &self,
        wallet_id: Uuid,
    ) -> anyhow::Result<(WalletInfo, Dek)> {
        let Some(active) = self.active_wallet.as_ref() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        }
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(dek) = active.dek.as_ref() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        Ok((active.wallet.clone(), Dek(dek.0)))
    }

    fn require_unlocked_wallet_db(
        &mut self,
        wallet_id: Uuid,
    ) -> anyhow::Result<(WalletInfo, &mut rusqlite::Connection)> {
        let Some(active) = self.active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.wallet.id != wallet_id {
            return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
        }
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        Ok((active.wallet.clone(), conn))
    }

    pub(crate) fn require_active_unlocked_wallet_db(
        &mut self,
    ) -> anyhow::Result<(WalletInfo, &mut rusqlite::Connection)> {
        let Some(active) = self.active_wallet.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        if active.lock_status != WalletLockStatus::Unlocked {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }
        let Some(conn) = active.wallet_db.as_mut() else {
            return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        };
        Ok((active.wallet.clone(), conn))
    }

    fn decrypt_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
        dek: &Dek,
    ) -> anyhow::Result<String> {
        let Some(bytes) = self
            .key_store
            .load_encrypted_mnemonic(wallet_id, network)
            .context("failed to read encrypted mnemonic")?
        else {
            return Err(ipc_err(errors::INTERNAL_ERROR, "mnemonic not found"));
        };

        let plaintext = decrypt_mnemonic(wallet_id, network, dek, &bytes)
            .context("failed to decrypt mnemonic")?;
        String::from_utf8(plaintext).context("mnemonic plaintext is not valid UTF-8")
    }
}

fn default_wallets_root() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".zkore").join("wallets"))
}

fn network_dir_name(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
    }
}

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}

fn mnemonic_aad(wallet_id: Uuid, network: Network) -> String {
    format!(
        "wallet_id={wallet_id};network={network:?};purpose=mnemonic;scheme=xchacha20poly1305;version=1"
    )
}

fn encrypt_mnemonic(
    wallet_id: Uuid,
    network: Network,
    dek: &Dek,
    plaintext: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(&dek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce: &XNonce = XNonce::from_slice(&nonce_bytes);

    let aad = mnemonic_aad(wallet_id, network);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to encrypt mnemonic: {e}"))?;

    let mut out = Vec::with_capacity(1 + nonce_bytes.len() + ciphertext.len());
    out.push(1u8); // version
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn decrypt_mnemonic(
    wallet_id: Uuid,
    network: Network,
    dek: &Dek,
    bytes: &[u8],
) -> anyhow::Result<Vec<u8>> {
    if bytes.len() < 1 + 24 + 16 {
        anyhow::bail!("encrypted mnemonic is too short");
    }
    let version = bytes[0];
    if version != 1 {
        anyhow::bail!("unsupported encrypted mnemonic version: {version}");
    }

    let nonce = &bytes[1..1 + 24];
    let ciphertext = &bytes[1 + 24..];

    let cipher = XChaCha20Poly1305::new_from_slice(&dek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;
    let aad = mnemonic_aad(wallet_id, network);
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to decrypt mnemonic: {e}"))
}

/// Safety margin for birthday height below chain tip.
/// New wallets use a birthday 100 blocks below the chain tip to avoid
/// edge cases where the tip might be reorged.
const NEW_WALLET_BIRTHDAY_MARGIN: u32 = 100;

/// Fetch the current chain tip height for use as a new wallet birthday.
///
/// Returns the chain tip height minus a safety margin, or `None` if the chain tip
/// cannot be fetched (e.g., no network connection).
///
/// For new wallets, this allows skipping the scan of the entire blockchain history
/// since a new wallet cannot have any funds before its creation.
pub async fn fetch_birthday_height_for_new_wallet(
    grpc_url: &str,
    tor_manager: Option<std::sync::Arc<zkore_tor::TorManager>>,
) -> Option<u32> {
    let client = match tor_manager {
        Some(tor) => zkore_network::grpc_client::GrpcClient::new_with_tor(grpc_url, tor),
        None => zkore_network::grpc_client::GrpcClient::new(grpc_url),
    };

    match client.get_latest_block().await {
        Ok((height, _hash)) => {
            let tip_height = u32::from(height);
            let birthday = tip_height.saturating_sub(NEW_WALLET_BIRTHDAY_MARGIN);
            tracing::debug!(
                chain_tip = tip_height,
                birthday = birthday,
                "fetched chain tip for new wallet birthday"
            );
            Some(birthday)
        }
        Err(err) => {
            tracing::warn!(
                error = ?err,
                "failed to fetch chain tip for new wallet birthday, will use Sapling activation"
            );
            None
        }
    }
}
