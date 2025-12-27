//! Transaction proposal creation, signing, and broadcast (US2+).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use rand::RngCore as _;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use zkore_core::domain::{Network, RecipientKind, TransactionInfo, TransactionStatus, TransactionType};
use zkore_core::errors;
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::commands::transaction::{
    ConfirmSendResponse, ListTransactionsResponse, PrepareSendResponse, TransactionSummary,
};
use zkore_core::ipc::v1::events::TransactionChangedEvent;

use crate::db::AppDb;
use crate::encryption::Dek;
use crate::error::ipc_err;
use crate::reauth::Clock;

const PROPOSAL_TTL: Duration = Duration::from_secs(5 * 60);
const QUEUED_BROADCAST_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub type TxEventHandler = Arc<dyn Fn(TransactionChangedEvent) + Send + Sync>;

#[derive(Debug)]
pub struct TxService<C: Clock> {
    clock: C,
    proposals: HashMap<String, ProposalRecord>,
    queued_broadcasts: HashMap<Uuid, HashMap<String, QueuedBroadcastEntry>>,
}

#[derive(Debug)]
struct ProposalRecord {
    wallet_id: Uuid,
    account_id: u32,
    expires_at: SystemTime,
    proposal: zcash_client_backend::proposal::Proposal<
        zcash_client_backend::fees::StandardFeeRule,
        zcash_client_sqlite::ReceivedNoteId,
    >,
    summary: TransactionSummary,
}

#[derive(Debug, Clone)]
struct QueuedBroadcastEntry {
    txid: String,
    created_at: SystemTime,
    last_error: Option<String>,
    bin_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedBroadcastMeta {
    created_at_ms: i64,
    last_error: Option<String>,
}

impl<C: Clock> TxService<C> {
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            proposals: HashMap::new(),
            queued_broadcasts: HashMap::new(),
        }
    }

    pub fn proposal_account_id(&self, proposal_id: &str) -> Option<u32> {
        self.proposals.get(proposal_id).map(|r| r.account_id)
    }

    pub fn scan_queued_broadcasts(&mut self, wallet_id: Uuid, wallet_dir: &Path) -> anyhow::Result<()> {
        let queue_dir = queued_broadcasts_dir(wallet_dir);
        if !queue_dir.exists() {
            self.queued_broadcasts.remove(&wallet_id);
            return Ok(());
        }

        let now = self.clock.now();
        let mut entries: HashMap<String, QueuedBroadcastEntry> = HashMap::new();

        for dir_entry in std::fs::read_dir(&queue_dir)
            .with_context(|| format!("failed to read queued broadcasts dir: {}", queue_dir.display()))?
        {
            let dir_entry = dir_entry?;
            let path = dir_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let txid = match path.file_stem().and_then(|s| s.to_str()) {
                Some(stem) if !stem.is_empty() => stem.to_string(),
                _ => continue,
            };

            let meta_bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            let meta: QueuedBroadcastMeta = match serde_json::from_slice(&meta_bytes) {
                Ok(meta) => meta,
                Err(_) => {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
            };

            let created_at = UNIX_EPOCH + Duration::from_millis(meta.created_at_ms.max(0) as u64);
            let bin_path = queued_broadcast_bin_path(&queue_dir, &txid);

            if !bin_path.exists() {
                let _ = std::fs::remove_file(&path);
                continue;
            }

            if now
                .duration_since(created_at)
                .unwrap_or(Duration::ZERO)
                > QUEUED_BROADCAST_RETENTION
            {
                let _ = std::fs::remove_file(&bin_path);
                let _ = std::fs::remove_file(&path);
                continue;
            }

            entries.insert(
                txid.clone(),
                QueuedBroadcastEntry {
                    txid,
                    created_at,
                    last_error: meta.last_error,
                    bin_path,
                },
            );
        }

        self.queued_broadcasts.insert(wallet_id, entries);
        Ok(())
    }

    pub fn prepare_send(
        &mut self,
        app_db: &AppDb,
        wallet_id: Uuid,
        network: Network,
        wallet_db_conn: &mut Connection,
        account_id: u32,
        recipient: &str,
        amount_zat: &str,
        memo: Option<&str>,
        allow_transparent_recipient: bool,
    ) -> anyhow::Result<PrepareSendResponse> {
        ensure_spend_allowed(app_db, wallet_id, account_id)?;

        let amount = parse_zatoshis(amount_zat)?;
        if amount == zcash_protocol::value::Zatoshis::ZERO {
            return Err(ipc_err(errors::INVALID_REQUEST, "amount must be > 0"));
        }

        let (recipient_addr, recipient_kind) = parse_recipient(network, recipient)?;
        enforce_privacy_and_memo_rules(recipient_kind, memo, allow_transparent_recipient)?;

        let memo_bytes = memo
            .map(|m| {
                zcash_protocol::memo::MemoBytes::from_bytes(m.as_bytes())
                    .map_err(|_| ipc_err(errors::MEMO_TOO_LONG, "memo too long"))
            })
            .transpose()?;

        let params = zcash_consensus_network(network);
        let account_uuid = resolve_wallet_account_uuid(wallet_db_conn, network, account_id)
            .context("failed to resolve wallet account")?;

        let fee_rule = zcash_client_backend::fees::StandardFeeRule::Zip317;
        let confirmations_policy =
            zcash_client_backend::data_api::wallet::ConfirmationsPolicy::default();

        let proposal = {
            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut *wallet_db_conn,
                params.clone(),
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            zcash_client_backend::data_api::wallet::propose_standard_transfer_to_address::<
                _,
                _,
                zcash_client_sqlite::error::SqliteClientError,
            >(
                &mut wdb,
                &params,
                fee_rule,
                account_uuid,
                confirmations_policy,
                &recipient_addr,
                amount,
                memo_bytes,
                None,
                zcash_protocol::ShieldedProtocol::Orchard,
            )
        };

        let proposal = match proposal {
            Ok(p) => p,
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("Insufficient balance") || err_str.contains("Insufficient funds")
                {
                    let balance = crate::balance::get_balance(wallet_db_conn, network, account_id)
                        .unwrap_or(zkore_core::domain::Balance {
                            shielded_spendable: "0".to_string(),
                            shielded_pending: "0".to_string(),
                            transparent_total: "0".to_string(),
                            total: "0".to_string(),
                        });
                    let shielded_spendable = balance.shielded_spendable.parse::<u64>().unwrap_or(0);
                    let transparent_total = balance.transparent_total.parse::<u64>().unwrap_or(0);
                    let total = balance.total.parse::<u64>().unwrap_or(0);
                    let amount_u64 = u64::from(amount);

                    if shielded_spendable < amount_u64 && total >= amount_u64 && transparent_total > 0 {
                        return Err(ipc_err(
                            errors::TRANSPARENT_SPEND_BLOCKED,
                            "shielded funds are insufficient; shield transparent funds first",
                        ));
                    }

                    return Err(ipc_err(errors::INSUFFICIENT_FUNDS, "insufficient funds"));
                }

                return Err(ipc_err(
                    errors::TRANSACTION_FAILED,
                    format!("failed to propose transaction: {err}"),
                ));
            }
        };

        if proposal
            .steps()
            .iter()
            .any(|step| !step.transparent_inputs().is_empty())
        {
            return Err(ipc_err(
                errors::TRANSPARENT_SPEND_BLOCKED,
                "transparent inputs are not allowed; shield funds before sending",
            ));
        }

        let fee = proposal_total_fee(&proposal)?;
        let fee_str = u64::from(fee).to_string();
        let amount_str = u64::from(amount).to_string();
        let total_spend_str = (u64::from(amount) + u64::from(fee)).to_string();

        let summary = TransactionSummary {
            recipient: recipient.to_string(),
            recipient_kind,
            amount: amount_str,
            fee: fee_str.clone(),
            memo_present: memo.is_some(),
            total_spend: total_spend_str,
        };

        let proposal_id = Uuid::new_v4().to_string();
        let now = self.clock.now();
        let expires_at = now + PROPOSAL_TTL;

        self.proposals.insert(
            proposal_id.clone(),
            ProposalRecord {
                wallet_id,
                account_id,
                expires_at,
                proposal,
                summary: summary.clone(),
            },
        );

        Ok(PrepareSendResponse {
            schema_version: SCHEMA_VERSION,
            proposal_id,
            fee: fee_str,
            summary,
            expires_at: to_unix_ms(expires_at)?,
        })
    }

    pub fn cancel_send(&mut self, proposal_id: &str) -> bool {
        self.proposals.remove(proposal_id).is_some()
    }

    pub fn confirm_send(
        &mut self,
        app_db: &AppDb,
        wallet_id: Uuid,
        network: Network,
        wallet_dir: &Path,
        wallet_dek: &Dek,
        wallet_db_conn: &mut Connection,
        grpc_url: &str,
        proposal_id: &str,
        spending_key: zcash_client_backend::keys::UnifiedSpendingKey,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<ConfirmSendResponse> {
        let now = self.clock.now();
        let record = self
            .proposals
            .remove(proposal_id)
            .ok_or_else(|| ipc_err(errors::PROPOSAL_NOT_FOUND, "proposal not found"))?;

        if record.wallet_id != wallet_id {
            return Err(ipc_err(errors::PROPOSAL_NOT_FOUND, "proposal not found"));
        }
        if now > record.expires_at {
            return Err(ipc_err(errors::PROPOSAL_EXPIRED, "proposal expired"));
        }

        ensure_spend_allowed(app_db, wallet_id, record.account_id)?;

        if record
            .proposal
            .steps()
            .iter()
            .any(|step| !step.transparent_inputs().is_empty())
        {
            return Err(ipc_err(
                errors::TRANSPARENT_SPEND_BLOCKED,
                "transparent inputs are not allowed for sends",
            ));
        }

        let params = zcash_consensus_network(network);

        let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
            wallet_db_conn,
            params.clone(),
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let spending_keys =
            zcash_client_backend::data_api::wallet::SpendingKeys::from_unified_spending_key(
                spending_key,
            );
        let prover = zcash_proofs::prover::LocalTxProver::bundled();

        let txids = zcash_client_backend::data_api::wallet::create_proposed_transactions::<
            _,
            _,
            std::convert::Infallible,
            _,
            std::convert::Infallible,
            _,
        >(
            &mut wdb,
            &params,
            &prover,
            &prover,
            &spending_keys,
            zcash_client_backend::wallet::OvkPolicy::Sender,
            &record.proposal,
        )
        .map_err(|e| ipc_err(errors::TRANSACTION_FAILED, format!("failed to build tx: {e}")))?;

        let mut broadcast_errors: HashMap<String, String> = HashMap::new();

        #[allow(deprecated)]
        use zcash_client_backend::data_api::WalletRead as _;

        for txid in txids.iter() {
            let tx = wdb
                .get_transaction(*txid)
                .map_err(|e| ipc_err(errors::TRANSACTION_FAILED, format!("failed to load tx: {e}")))?
                .ok_or_else(|| ipc_err(errors::TRANSACTION_FAILED, "tx bytes unavailable"))?;

            let mut tx_bytes = Vec::new();
            tx.write(&mut tx_bytes).map_err(|e| {
                ipc_err(errors::TRANSACTION_FAILED, format!("failed to serialize tx: {e}"))
            })?;

            if let Err(err) = send_transaction_bytes(grpc_url, &tx_bytes) {
                let err_msg = format!("{err:#}");
                broadcast_errors.insert(txid.to_string(), err_msg.clone());
                queue_broadcast(
                    &self.clock,
                    wallet_id,
                    wallet_dir,
                    wallet_dek,
                    txid.to_string(),
                    &tx_bytes,
                    Some(err_msg),
                )?;
            } else {
                delete_queued_broadcast(wallet_dir, txid.to_string());
            }
        }

        self.scan_queued_broadcasts(wallet_id, wallet_dir)?;

        let primary_txid = txids[0].to_string();
        let now_ms = chrono::Utc::now().timestamp_millis();
        let queued = self
            .queued_broadcasts
            .get(&wallet_id)
            .and_then(|m| m.get(&primary_txid))
            .cloned();

        let (status, last_error, can_retry_broadcast) = match queued {
            Some(entry) => (
                TransactionStatus::Failed,
                entry.last_error.clone(),
                true,
            ),
            None => (TransactionStatus::Pending, None, false),
        };

        if let Some(handler) = on_tx_changed.as_ref() {
            handler(TransactionChangedEvent {
                schema_version: SCHEMA_VERSION,
                event: "tx.changed".to_string(),
                transaction: TransactionInfo {
                    txid: primary_txid.clone(),
                    account_id: record.account_id,
                    tx_type: TransactionType::Send,
                    value: record.summary.amount.clone(),
                    fee: record.summary.fee.clone(),
                    memo_present: record.summary.memo_present,
                    memo: None,
                    status,
                    last_error,
                    can_retry_broadcast,
                    mined_height: None,
                    created_at: now_ms,
                    confirmed_at: None,
                },
            });
        }

        if broadcast_errors.contains_key(&primary_txid) {
            // Broadcast failure is communicated via TransactionInfo status + queued metadata.
        }

        Ok(ConfirmSendResponse {
            schema_version: SCHEMA_VERSION,
            txid: primary_txid,
        })
    }

    pub fn retry_broadcast(
        &mut self,
        app_db: &AppDb,
        wallet_id: Uuid,
        network: Network,
        wallet_dir: &Path,
        wallet_dek: &Dek,
        wallet_db_conn: &mut Connection,
        grpc_url: &str,
        txid: &str,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<String> {
        self.scan_queued_broadcasts(wallet_id, wallet_dir)?;

        let account_id: Option<i64> = wallet_db_conn
            .query_row(
                "SELECT sent_from_account FROM v_tx_sent WHERE txid = ?1 LIMIT 1",
                [txid],
                |row| row.get(0),
            )
            .optional()?;
        let Some(account_id) = account_id else {
            return Err(ipc_err(
                errors::QUEUED_BROADCAST_NOT_FOUND,
                "queued broadcast not found",
            ));
        };
        let account_id_u32 = account_id.max(0) as u32;

        ensure_spend_allowed(app_db, wallet_id, account_id_u32)?;

        let entry = self
            .queued_broadcasts
            .get(&wallet_id)
            .and_then(|map| map.get(txid))
            .cloned()
            .ok_or_else(|| ipc_err(errors::QUEUED_BROADCAST_NOT_FOUND, "queued broadcast not found"))?;

        let tx_bytes = decrypt_queued_tx_bytes(wallet_id, wallet_dek, &entry.bin_path)?;

        match send_transaction_bytes(grpc_url, &tx_bytes) {
            Ok(()) => {
                delete_queued_broadcast(wallet_dir, txid.to_string());
            }
            Err(err) => {
                let err_msg = format!("{err:#}");
                update_queued_broadcast_error(&self.clock, wallet_dir, txid, Some(err_msg))?;
            }
        }

        self.scan_queued_broadcasts(wallet_id, wallet_dir)?;

        if let Some(handler) = on_tx_changed.as_ref() {
            let list = self.list_transactions(
                wallet_id,
                network,
                wallet_dir,
                wallet_db_conn,
                account_id_u32,
                200,
                0,
            )?;
            if let Some(info) = list.transactions.into_iter().find(|t| t.txid == txid) {
                handler(TransactionChangedEvent {
                    schema_version: SCHEMA_VERSION,
                    event: "tx.changed".to_string(),
                    transaction: info,
                });
            }
        }

        Ok(txid.to_string())
    }

    pub fn list_transactions(
        &mut self,
        wallet_id: Uuid,
        network: Network,
        wallet_dir: &Path,
        wallet_db_conn: &mut Connection,
        account_id: u32,
        limit: u32,
        offset: u32,
    ) -> anyhow::Result<ListTransactionsResponse> {
        self.scan_queued_broadcasts(wallet_id, wallet_dir)?;

        let total_count: i64 = wallet_db_conn.query_row(
            "SELECT COUNT(*) FROM (\n             SELECT id_tx FROM v_tx_sent WHERE sent_from_account = ?1\n             UNION ALL\n             SELECT id_tx FROM v_tx_received WHERE received_by_account = ?1\n             )",
            [account_id as i64],
            |row| row.get(0),
        )?;
        let total_count = total_count.max(0) as u32;

        let chain_height = current_chain_height(wallet_db_conn, network).unwrap_or(0);

        let mut stmt = wallet_db_conn.prepare(
            "SELECT txid, mined_height, expiry_height, fee_paid, value, memo_present, tx_type, block_time\n             FROM (\n               SELECT s.txid AS txid,\n                      s.mined_height AS mined_height,\n                      s.expiry_height AS expiry_height,\n                      t.fee_paid AS fee_paid,\n                      s.sent_total AS value,\n                      s.memo_count > 0 AS memo_present,\n                      'Send' AS tx_type,\n                      s.block_time AS block_time\n               FROM v_tx_sent s\n               JOIN v_transactions t ON t.id_tx = s.id_tx\n               WHERE s.sent_from_account = ?1\n\n               UNION ALL\n\n               SELECT r.txid AS txid,\n                      r.mined_height AS mined_height,\n                      r.expiry_height AS expiry_height,\n                      t.fee_paid AS fee_paid,\n                      r.received_total AS value,\n                      r.memo_count > 0 AS memo_present,\n                      'Receive' AS tx_type,\n                      r.block_time AS block_time\n               FROM v_tx_received r\n               JOIN v_transactions t ON t.id_tx = r.id_tx\n               WHERE r.received_by_account = ?1\n             )\n             ORDER BY COALESCE(block_time, 0) DESC, txid DESC\n             LIMIT ?2 OFFSET ?3",
        )?;

        let mut rows = stmt.query(params![account_id as i64, limit as i64, offset as i64])?;
        let mut transactions = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        while let Some(row) = rows.next()? {
            let txid: String = row.get(0)?;
            if !seen.insert(txid.clone()) {
                continue;
            }
            let mined_height: Option<i64> = row.get(1)?;
            let expiry_height: Option<i64> = row.get(2)?;
            let fee_paid: Option<i64> = row.get(3)?;
            let value: i64 = row.get(4)?;
            let memo_present: bool = row.get(5)?;
            let tx_type_str: String = row.get(6)?;
            let block_time: Option<i64> = row.get(7)?;

            let tx_type = match tx_type_str.as_str() {
                "Send" => TransactionType::Send,
                "Receive" => TransactionType::Receive,
                "Shield" => TransactionType::Shield,
                "Consolidate" => TransactionType::Consolidate,
                _ => TransactionType::Receive,
            };

            let mined_height_u32 = mined_height.and_then(|h| u32::try_from(h).ok());
            let expiry_height_u32 = expiry_height.and_then(|h| u32::try_from(h).ok());
            let confirmed_at = block_time.map(|t| t.saturating_mul(1000));

            let mut status = if mined_height_u32.is_some() {
                TransactionStatus::Confirmed
            } else {
                TransactionStatus::Pending
            };

            if mined_height_u32.is_none() {
                if let Some(expiry) = expiry_height_u32 {
                    if chain_height > expiry {
                        status = TransactionStatus::Expired;
                        delete_queued_broadcast(wallet_dir, txid.clone());
                    }
                }
            }

            let queue_entry = self
                .queued_broadcasts
                .get(&wallet_id)
                .and_then(|map| map.get(&txid));

            let (last_error, can_retry_broadcast) = match (status, queue_entry) {
                (TransactionStatus::Confirmed, _) => (None, false),
                (TransactionStatus::Expired, _) => (None, false),
                (TransactionStatus::Pending, Some(entry)) => {
                    status = TransactionStatus::Failed;
                    (entry.last_error.clone(), true)
                }
                (TransactionStatus::Failed, Some(entry)) => (entry.last_error.clone(), true),
                _ => (None, false),
            };

            transactions.push(TransactionInfo {
                txid,
                account_id,
                tx_type,
                value: value.max(0).to_string(),
                fee: fee_paid.unwrap_or(0).max(0).to_string(),
                memo_present,
                memo: None,
                status,
                last_error,
                can_retry_broadcast,
                mined_height: mined_height_u32,
                created_at: confirmed_at.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                confirmed_at,
            });
        }

        Ok(ListTransactionsResponse {
            schema_version: SCHEMA_VERSION,
            transactions,
            total_count,
        })
    }
}

fn ensure_spend_allowed(app_db: &AppDb, wallet_id: Uuid, account_id: u32) -> anyhow::Result<()> {
    let backup_required =
        crate::db::backup_meta::get_backup_required(app_db.conn(), wallet_id)?.unwrap_or(true);
    if backup_required {
        return Err(ipc_err(errors::BACKUP_REQUIRED, "backup required"));
    }

    let account_type: Option<String> = app_db
        .conn()
        .query_row(
            "SELECT account_type FROM accounts WHERE wallet_id = ?1 AND account_id = ?2",
            params![wallet_id.to_string(), account_id as i64],
            |row| row.get(0),
        )
        .optional()?;

    let Some(account_type) = account_type else {
        return Err(ipc_err(errors::ACCOUNT_NOT_FOUND, "account not found"));
    };

    if account_type == "HardwareSigner" || account_type == "WatchOnly" {
        return Err(ipc_err(
            errors::WATCH_ONLY_CANNOT_SPEND,
            "watch-only accounts cannot spend",
        ));
    }

    Ok(())
}

fn parse_zatoshis(value: &str) -> anyhow::Result<zcash_protocol::value::Zatoshis> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ipc_err(errors::INVALID_REQUEST, "amount required"));
    }
    let parsed: u64 = trimmed
        .parse()
        .map_err(|_| ipc_err(errors::INVALID_REQUEST, "invalid amount"))?;
    zcash_protocol::value::Zatoshis::from_u64(parsed)
        .map_err(|_| ipc_err(errors::INVALID_REQUEST, "invalid amount"))
}

fn parse_recipient(
    network: Network,
    recipient: &str,
) -> anyhow::Result<(zcash_client_backend::address::Address, RecipientKind)> {
    let params = zcash_consensus_network(network);

    let addr = zcash_client_backend::address::Address::decode(&params, recipient)
        .ok_or_else(|| ipc_err(errors::INVALID_RECIPIENT, "invalid recipient"))?;

    let kind = match &addr {
        zcash_client_backend::address::Address::Unified(ua) => {
            if ua.has_orchard() {
                RecipientKind::Orchard
            } else if ua.has_sapling() {
                RecipientKind::Sapling
            } else if ua.has_transparent() {
                RecipientKind::Transparent
            } else {
                return Err(ipc_err(errors::INVALID_RECIPIENT, "invalid recipient"));
            }
        }
        zcash_client_backend::address::Address::Sapling(_) => RecipientKind::Sapling,
        zcash_client_backend::address::Address::Transparent(_) => RecipientKind::Transparent,
        zcash_client_backend::address::Address::Tex(_) => RecipientKind::Transparent,
    };

    Ok((addr, kind))
}

fn enforce_privacy_and_memo_rules(
    recipient_kind: RecipientKind,
    memo: Option<&str>,
    allow_transparent_recipient: bool,
) -> anyhow::Result<()> {
    if recipient_kind == RecipientKind::Transparent {
        if memo.is_some() {
            return Err(ipc_err(
                errors::MEMO_NOT_ALLOWED,
                "memos are not allowed for transparent recipients",
            ));
        }
        if !allow_transparent_recipient {
            return Err(ipc_err(
                errors::PRIVACY_ACK_REQUIRED,
                "transparent recipient requires privacy acknowledgement",
            ));
        }
    }

    if let Some(memo) = memo {
        if memo.as_bytes().len() > 512 {
            return Err(ipc_err(errors::MEMO_TOO_LONG, "memo too long"));
        }
    }

    Ok(())
}

fn proposal_total_fee(
    proposal: &zcash_client_backend::proposal::Proposal<
        zcash_client_backend::fees::StandardFeeRule,
        zcash_client_sqlite::ReceivedNoteId,
    >,
) -> anyhow::Result<zcash_protocol::value::Zatoshis> {
    let mut total: u64 = 0;
    for step in proposal.steps().iter() {
        total = total.saturating_add(u64::from(step.balance().fee_required()));
    }
    zcash_protocol::value::Zatoshis::from_u64(total)
        .map_err(|_| ipc_err(errors::INTERNAL_ERROR, "fee out of range"))
}

fn resolve_wallet_account_uuid(
    conn: &mut Connection,
    network: Network,
    account_id: u32,
) -> anyhow::Result<zcash_client_sqlite::AccountUuid> {
    #[allow(deprecated)]
    use zcash_client_backend::data_api::{Account as _, WalletRead as _};

    let params = zcash_consensus_network(network);
    let wdb = zcash_client_sqlite::WalletDb::from_connection(
        conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    for account_uuid in wdb.get_account_ids().context("failed to list wallet accounts")? {
        let Some(account) = wdb.get_account(account_uuid).context("failed to load account")? else {
            continue;
        };
        let Some(derivation) = account.source().key_derivation() else {
            continue;
        };
        let idx: u32 = derivation.account_index().into();
        if idx == account_id {
            return Ok(account_uuid);
        }
    }

    Err(ipc_err(errors::ACCOUNT_NOT_FOUND, "account not found"))
}

fn queued_broadcasts_dir(wallet_dir: &Path) -> PathBuf {
    wallet_dir.join("queued_broadcasts")
}

fn queued_broadcast_bin_path(queue_dir: &Path, txid: &str) -> PathBuf {
    queue_dir.join(format!("{txid}.bin"))
}

fn queued_broadcast_meta_path(queue_dir: &Path, txid: &str) -> PathBuf {
    queue_dir.join(format!("{txid}.json"))
}

fn queue_broadcast<C: Clock>(
    clock: &C,
    wallet_id: Uuid,
    wallet_dir: &Path,
    wallet_dek: &Dek,
    txid: String,
    tx_bytes: &[u8],
    last_error: Option<String>,
) -> anyhow::Result<()> {
    let queue_dir = queued_broadcasts_dir(wallet_dir);
    std::fs::create_dir_all(&queue_dir)
        .with_context(|| format!("failed to create queued broadcasts dir: {}", queue_dir.display()))?;

    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let nonce_ref: &XNonce = XNonce::from_slice(&nonce);
    let cipher = XChaCha20Poly1305::new_from_slice(&wallet_dek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let aad = format!("wallet_id={wallet_id};txid={txid};aead_scheme=xchacha20poly1305;v=1");
    let ciphertext = cipher
        .encrypt(
            nonce_ref,
            Payload {
                msg: tx_bytes,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to encrypt queued tx: {e}"))?;

    let bin_path = queued_broadcast_bin_path(&queue_dir, &txid);
    let meta_path = queued_broadcast_meta_path(&queue_dir, &txid);

    let mut out = Vec::with_capacity(24 + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    std::fs::write(&bin_path, out)
        .with_context(|| format!("failed to write queued tx bytes: {}", bin_path.display()))?;

    let meta = QueuedBroadcastMeta {
        created_at_ms: to_unix_ms(clock.now())?,
        last_error,
    };
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)
        .with_context(|| format!("failed to write queued tx metadata: {}", meta_path.display()))?;

    Ok(())
}

fn decrypt_queued_tx_bytes(wallet_id: Uuid, wallet_dek: &Dek, bin_path: &Path) -> anyhow::Result<Vec<u8>> {
    let bytes = std::fs::read(bin_path)
        .with_context(|| format!("failed to read queued tx bytes: {}", bin_path.display()))?;
    if bytes.len() < 24 {
        return Err(anyhow::anyhow!("queued tx blob too short"));
    }
    let (nonce_bytes, ciphertext) = bytes.split_at(24);
    let nonce_ref: &XNonce = XNonce::from_slice(nonce_bytes);
    let cipher = XChaCha20Poly1305::new_from_slice(&wallet_dek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let txid = bin_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    let aad = format!("wallet_id={wallet_id};txid={txid};aead_scheme=xchacha20poly1305;v=1");
    let plaintext = cipher
        .decrypt(
            nonce_ref,
            Payload {
                msg: ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to decrypt queued tx: {e}"))?;

    Ok(plaintext)
}

fn delete_queued_broadcast(wallet_dir: &Path, txid: String) {
    let queue_dir = queued_broadcasts_dir(wallet_dir);
    let _ = std::fs::remove_file(queued_broadcast_bin_path(&queue_dir, &txid));
    let _ = std::fs::remove_file(queued_broadcast_meta_path(&queue_dir, &txid));
}

fn update_queued_broadcast_error<C: Clock>(
    clock: &C,
    wallet_dir: &Path,
    txid: &str,
    last_error: Option<String>,
) -> anyhow::Result<()> {
    let queue_dir = queued_broadcasts_dir(wallet_dir);
    let meta_path = queued_broadcast_meta_path(&queue_dir, txid);
    let existing: QueuedBroadcastMeta = std::fs::read(&meta_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(QueuedBroadcastMeta {
            created_at_ms: to_unix_ms(clock.now())?,
            last_error: None,
        });

    let updated = QueuedBroadcastMeta {
        created_at_ms: existing.created_at_ms,
        last_error,
    };
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&updated)?)
        .with_context(|| format!("failed to update queued broadcast meta: {}", meta_path.display()))?;
    Ok(())
}

fn send_transaction_bytes(grpc_url: &str, tx_bytes: &[u8]) -> anyhow::Result<()> {
    let client = zkore_network::grpc_client::GrpcClient::new(grpc_url.to_string());
    let tx_bytes = tx_bytes.to_vec();
    block_on(async move { client.send_transaction(tx_bytes).await })
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => tokio::runtime::Runtime::new()
            .expect("create tokio runtime")
            .block_on(future),
    }
}

fn to_unix_ms(time: SystemTime) -> anyhow::Result<i64> {
    let dur = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ipc_err(errors::INTERNAL_ERROR, "time went backwards"))?;
    i64::try_from(dur.as_millis()).map_err(|_| ipc_err(errors::INTERNAL_ERROR, "timestamp overflow"))
}

fn current_chain_height(conn: &mut Connection, network: Network) -> Option<u32> {
    #[allow(deprecated)]
    use zcash_client_backend::data_api::WalletRead as _;

    let params = zcash_consensus_network(network);
    let wdb = zcash_client_sqlite::WalletDb::from_connection(
        conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    wdb.chain_height().ok().flatten().map(u32::from)
}

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}
