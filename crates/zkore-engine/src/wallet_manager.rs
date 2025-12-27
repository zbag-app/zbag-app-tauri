use std::path::{Path, PathBuf};

use anyhow::Context as _;
use rusqlite::OpenFlags;
use uuid::Uuid;
use zeroize::Zeroize;

use zkore_core::domain::{Network, WalletInfo, WalletLockStatus, WalletType};

use crate::db::wallet_encryption_meta::WalletEncryptionMeta;
use crate::db::{backup_meta, wallet_encryption_meta, wallet_meta, AppDb};
use crate::encryption::{Dek, default_aead_params, default_kdf_params, generate_dek, unwrap_dek, wrap_dek};
use crate::key_store::KeyStore;
use crate::reauth::{ReauthManager, SystemClock};
use zkore_core::ipc::v1::commands::wallet::ReauthPurpose;

pub struct WalletManager {
    app_db: AppDb,
    key_store: Box<dyn KeyStore>,
    wallets_root: PathBuf,
    active_wallet: Option<ActiveWallet>,
    reauth: ReauthManager,
    wallet_db_force_validate_fail: bool,
}

#[derive(Debug)]
struct ActiveWallet {
    wallet: WalletInfo,
    lock_status: WalletLockStatus,
    dek: Option<Dek>,
    wallet_db: Option<rusqlite::Connection>,
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
            wallet_db_force_validate_fail: false,
        })
    }

    pub fn list_wallets(&self) -> anyhow::Result<Vec<WalletInfo>> {
        let wallets = wallet_meta::list_wallets(self.app_db.conn())?;
        Ok(wallets)
    }

    pub fn load_wallet(&mut self, wallet_id: Uuid) -> anyhow::Result<(WalletInfo, WalletLockStatus)> {
        let Some((wallet, directory_path_str)) = wallet_meta::get_wallet(self.app_db.conn(), wallet_id)?
        else {
            anyhow::bail!("wallet not found");
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        wallet_meta::update_last_opened_at(self.app_db.conn(), wallet_id, now_ms)?;

        let directory_path = PathBuf::from(directory_path_str);
        let mut lock_status = WalletLockStatus::Locked;
        let mut dek: Option<Dek> = None;
        let mut wallet_db: Option<rusqlite::Connection> = None;

        if wallet.remember_unlock_enabled {
            if let Some(mut material) =
                self.key_store.load_keychain_unlock_material(wallet_id, wallet.network)?
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
        }

        self.active_wallet = Some(ActiveWallet {
            wallet: wallet.clone(),
            lock_status,
            dek,
            wallet_db,
        });

        Ok((wallet, lock_status))
    }

    pub fn create_wallet(
        &mut self,
        name: &str,
        network: Network,
        password: &str,
        remember_unlock: bool,
    ) -> anyhow::Result<WalletInfo> {
        if name.trim().is_empty() {
            anyhow::bail!("wallet name must not be empty");
        }
        if name.chars().count() > 50 {
            anyhow::bail!("wallet name must be at most 50 characters");
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
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
        let wrapped_dek_b64 =
            wrap_dek(wallet.id, wallet.network, password, &kdf.salt_b64, &aead.nonce_b64, &dek)?;

        wallet_encryption_meta::insert_wallet_encryption(
            self.app_db.conn(),
            wallet.id,
            &WalletEncryptionMeta {
                kdf,
                aead,
                wrapped_dek_b64,
            },
        )?;

        self.open_wallet_db_with_dek(&wallet_dir, network, &dek, None, true)
            .context("failed to initialize encrypted wallet db")?;

        if remember_unlock {
            self.key_store
                .store_keychain_unlock_material(wallet.id, network, &dek.0)
                .context("failed to store keychain unlock material")?;
        }

        Ok(wallet)
    }

    pub fn unlock_wallet(
        &mut self,
        wallet_id: Uuid,
        password: &str,
        remember_unlock: bool,
    ) -> anyhow::Result<WalletLockStatus> {
        let Some((wallet, directory_path_str)) = wallet_meta::get_wallet(self.app_db.conn(), wallet_id)?
        else {
            anyhow::bail!("wallet not found");
        };
        let directory_path = PathBuf::from(directory_path_str);

        let Some(meta) = wallet_encryption_meta::get_wallet_encryption(self.app_db.conn(), wallet_id)?
        else {
            anyhow::bail!("wallet encryption metadata not found");
        };

        let dek = unwrap_dek(
            wallet_id,
            wallet.network,
            password,
            &meta.kdf.salt_b64,
            &meta.aead.nonce_b64,
            &meta.wrapped_dek_b64,
        )
        .context("failed to unwrap wallet DEK")?;

        let conn = self
            .open_wallet_db_with_dek(&directory_path, wallet.network, &dek, None, false)
            .context("failed to open wallet db")?;

        wallet_meta::set_remember_unlock_enabled(self.app_db.conn(), wallet_id, remember_unlock)?;
        if remember_unlock {
            self.key_store
                .store_keychain_unlock_material(wallet_id, wallet.network, &dek.0)
                .context("failed to store keychain unlock material")?;
        } else {
            self.key_store
                .delete_keychain_unlock_material(wallet_id, wallet.network)
                .ok();
        }

        self.active_wallet = Some(ActiveWallet {
            wallet: wallet.clone(),
            lock_status: WalletLockStatus::Unlocked,
            dek: Some(dek),
            wallet_db: Some(conn),
        });

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
        Ok(WalletLockStatus::Locked)
    }

    pub fn reauth_wallet(
        &mut self,
        wallet_id: Uuid,
        password: &str,
        purpose: ReauthPurpose,
    ) -> anyhow::Result<(String, std::time::SystemTime)> {
        if password.is_empty() {
            anyhow::bail!("password required");
        }

        let Some((wallet, _directory_path_str)) =
            wallet_meta::get_wallet(self.app_db.conn(), wallet_id)?
        else {
            anyhow::bail!("wallet not found");
        };

        let Some(meta) =
            wallet_encryption_meta::get_wallet_encryption(self.app_db.conn(), wallet_id)?
        else {
            anyhow::bail!("wallet encryption metadata not found");
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
        .context("invalid wallet password")?;

        let (token, expires_at) = self.reauth.issue(wallet_id, purpose);
        Ok((token, expires_at))
    }

    pub fn key_store(&self) -> &dyn KeyStore {
        self.key_store.as_ref()
    }

    pub fn app_db(&self) -> &AppDb {
        &self.app_db
    }

    pub fn wallets_root(&self) -> &Path {
        &self.wallets_root
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn __set_wallet_db_force_validate_fail(&mut self, enabled: bool) {
        self.wallet_db_force_validate_fail = enabled;
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
        seed: Option<secrecy::SecretVec<u8>>,
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
            let mut conn = self.open_sqlcipher_connection(&wallet_db_path, dek, create_if_missing)?;

            rusqlite::vtab::array::load_module(&conn).context("failed to load sqlite array module")?;

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
                    let restore_result =
                        std::fs::copy(&snapshot_path, &wallet_db_path).with_context(|| {
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
