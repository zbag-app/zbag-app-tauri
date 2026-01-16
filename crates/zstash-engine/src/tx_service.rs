//! Transaction proposal creation, signing, and broadcast (US2+).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use rand::RngCore as _;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, instrument};
use uuid::Uuid;

use zstash_core::domain::{
    Network, RecipientKind, TransactionInfo, TransactionStatus, TransactionType,
};
use zstash_core::errors;
use zstash_core::ipc::v1::commands::keystone::{
    BuildSigningRequestResponse, FinalizeSigningResponse, SigningRequest, SigningSummary,
};
use zstash_core::ipc::v1::commands::transaction::{
    ConfirmSendResponse, ListTransactionsResponse, PrepareSendResponse, ShieldFundsResponse,
    TransactionSummary,
};
use zstash_core::ipc::v1::common::SCHEMA_VERSION;
use zstash_core::ipc::v1::events::TransactionChangedEvent;

use crate::db::AppDb;
use crate::encryption::Dek;
use crate::error::ipc_err;
use crate::reauth::Clock;
use crate::tokio_runtime::block_on;

const PROPOSAL_TTL: Duration = Duration::from_secs(10 * 60);
const QUEUED_BROADCAST_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const MAX_SHIELDING_INPUTS_PER_TX: usize = 200;

pub type TxEventHandler = Arc<dyn Fn(TransactionChangedEvent) + Send + Sync>;

pub struct TxService<C: Clock> {
    clock: C,
    proposals: HashMap<String, ProposalRecord>,
    queued_broadcasts: HashMap<Uuid, HashMap<String, QueuedBroadcastEntry>>,
    /// Pending signing requests: maps signing_request_id to full PCZT with proofs (base64).
    /// Used in the two-PCZT flow for hardware signing.
    pending_signing_requests: HashMap<String, PendingSigningRequest>,
    tor_manager: Option<Arc<zstash_tor::TorManager>>,
}

/// A pending hardware signing request containing the full PCZT with proofs.
#[derive(Debug, Clone)]
struct PendingSigningRequest {
    wallet_id: Uuid,
    /// Full PCZT with proofs (base64 encoded).
    pczt_with_proofs: String,
    expires_at: SystemTime,
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
            pending_signing_requests: HashMap::new(),
            tor_manager: None,
        }
    }

    pub fn set_tor_manager(&mut self, tor_manager: Arc<zstash_tor::TorManager>) {
        self.tor_manager = Some(tor_manager);
    }

    pub fn proposal_account_id(&self, proposal_id: &str) -> Option<u32> {
        self.proposals.get(proposal_id).map(|r| r.account_id)
    }

    pub fn scan_queued_broadcasts(
        &mut self,
        wallet_id: Uuid,
        wallet_dir: &Path,
    ) -> anyhow::Result<()> {
        let queue_dir = queued_broadcasts_dir(wallet_dir);
        if !queue_dir.exists() {
            self.queued_broadcasts.remove(&wallet_id);
            return Ok(());
        }

        let now = self.clock.now();
        let mut entries: HashMap<String, QueuedBroadcastEntry> = HashMap::new();

        for dir_entry in std::fs::read_dir(&queue_dir).with_context(|| {
            format!(
                "failed to read queued broadcasts dir: {}",
                queue_dir.display()
            )
        })? {
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
                Err(e) => {
                    tracing::debug!(path = ?path, error = ?e, "skipping unreadable queue file");
                    continue;
                }
            };

            let meta: QueuedBroadcastMeta = match serde_json::from_slice(&meta_bytes) {
                Ok(meta) => meta,
                Err(e) => {
                    tracing::debug!(path = ?path, error = ?e, "skipping unparseable queue meta file");
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::debug!(path = ?path, error = ?e, "failed to cleanup queue file");
                    }
                    continue;
                }
            };

            let created_at = UNIX_EPOCH + Duration::from_millis(meta.created_at_ms.max(0) as u64);
            let bin_path = queued_broadcast_bin_path(&queue_dir, &txid);

            if !bin_path.exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::debug!(path = ?path, error = ?e, "failed to cleanup orphaned queue meta file");
                }
                continue;
            }

            if now.duration_since(created_at).unwrap_or(Duration::ZERO) > QUEUED_BROADCAST_RETENTION
            {
                if let Err(e) = std::fs::remove_file(&bin_path) {
                    tracing::debug!(path = ?bin_path, error = ?e, "failed to cleanup expired queue bin file");
                }
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::debug!(path = ?path, error = ?e, "failed to cleanup expired queue meta file");
                }
                continue;
            }

            entries.insert(
                txid.clone(),
                QueuedBroadcastEntry {
                    last_error: meta.last_error,
                    bin_path,
                },
            );
        }

        self.queued_broadcasts.insert(wallet_id, entries);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
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

        let balance =
            crate::balance::get_balance(wallet_db_conn, network, account_id).map_err(|e| {
                ipc_err(
                    errors::INTERNAL_ERROR,
                    format!("balance lookup failed: {}", e),
                )
            })?;
        let shielded_spendable = balance.shielded_spendable.parse::<u64>().map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("invalid shielded_spendable format: {}", e),
            )
        })?;
        let shielded_pending = balance.shielded_pending.parse::<u64>().map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("invalid shielded_pending format: {}", e),
            )
        })?;
        let transparent_total = balance.transparent_total.parse::<u64>().map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("invalid transparent_total format: {}", e),
            )
        })?;
        let spendable_if_transparent = shielded_spendable.saturating_add(transparent_total);
        let amount_u64 = u64::from(amount);

        if shielded_spendable < amount_u64
            && transparent_total > 0
            && spendable_if_transparent >= amount_u64
        {
            return Err(ipc_err(
                errors::TRANSPARENT_SPEND_BLOCKED,
                "shielded funds are insufficient; shield transparent funds first",
            ));
        }

        if shielded_spendable < amount_u64
            && shielded_pending > 0
            && shielded_spendable.saturating_add(shielded_pending) >= amount_u64
        {
            return Err(ipc_err(
                errors::INSUFFICIENT_FUNDS,
                "insufficient spendable funds (some funds are still pending sync/restore)",
            ));
        }

        let params = zcash_consensus_network(network);
        let account_uuid = resolve_wallet_account_uuid(wallet_db_conn, network, account_id)
            .context("failed to resolve wallet account")?;

        let fee_rule = zcash_client_backend::fees::StandardFeeRule::Zip317;
        let confirmations_policy =
            zcash_client_backend::data_api::wallet::ConfirmationsPolicy::default();

        let proposal = {
            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut *wallet_db_conn,
                params,
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
                if err_str.contains("Insufficient balance")
                    || err_str.contains("Insufficient funds")
                {
                    let spendable_if_transparent =
                        shielded_spendable.saturating_add(transparent_total);
                    if shielded_spendable < amount_u64
                        && transparent_total > 0
                        && spendable_if_transparent >= amount_u64
                    {
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

    #[allow(clippy::too_many_arguments)]
    pub fn build_signing_request(
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
    ) -> anyhow::Result<BuildSigningRequestResponse> {
        ensure_backup_complete(app_db, wallet_id)?;

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

        let balance =
            crate::balance::get_balance(wallet_db_conn, network, account_id).map_err(|e| {
                ipc_err(
                    errors::INTERNAL_ERROR,
                    format!("balance lookup failed: {}", e),
                )
            })?;
        let shielded_spendable = balance.shielded_spendable.parse::<u64>().map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("invalid shielded_spendable format: {}", e),
            )
        })?;
        let shielded_pending = balance.shielded_pending.parse::<u64>().map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("invalid shielded_pending format: {}", e),
            )
        })?;
        let transparent_total = balance.transparent_total.parse::<u64>().map_err(|e| {
            ipc_err(
                errors::INTERNAL_ERROR,
                format!("invalid transparent_total format: {}", e),
            )
        })?;
        let spendable_if_transparent = shielded_spendable.saturating_add(transparent_total);
        let amount_u64 = u64::from(amount);

        if shielded_spendable < amount_u64
            && transparent_total > 0
            && spendable_if_transparent >= amount_u64
        {
            return Err(ipc_err(
                errors::TRANSPARENT_SPEND_BLOCKED,
                "shielded funds are insufficient; shield transparent funds first",
            ));
        }

        if shielded_spendable < amount_u64
            && shielded_pending > 0
            && shielded_spendable.saturating_add(shielded_pending) >= amount_u64
        {
            return Err(ipc_err(
                errors::INSUFFICIENT_FUNDS,
                "insufficient spendable funds (some funds are still pending sync/restore)",
            ));
        }

        let params = zcash_consensus_network(network);
        let account_uuid = resolve_wallet_account_uuid(wallet_db_conn, network, account_id)
            .context("failed to resolve wallet account")?;

        let fee_rule = zcash_client_backend::fees::StandardFeeRule::Zip317;
        let confirmations_policy =
            zcash_client_backend::data_api::wallet::ConfirmationsPolicy::default();

        let (proposal, pczt_full, pczt_for_signer) = {
            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut *wallet_db_conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let proposal =
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
                .map_err(|err| {
                    let err_str = err.to_string();
                    if err_str.contains("Insufficient balance")
                        || err_str.contains("Insufficient funds")
                    {
                        ipc_err(errors::INSUFFICIENT_FUNDS, "insufficient funds")
                    } else {
                        ipc_err(
                            errors::TRANSACTION_FAILED,
                            format!("failed to propose transaction: {err}"),
                        )
                    }
                })?;

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

            let pczt = zcash_client_backend::data_api::wallet::create_pczt_from_proposal::<
                _,
                _,
                std::convert::Infallible,
                _,
                std::convert::Infallible,
                _,
            >(
                &mut wdb,
                &params,
                account_uuid,
                zcash_client_backend::wallet::OvkPolicy::Sender,
                &proposal,
            )
            .map_err(|e| {
                ipc_err(
                    errors::SIGNING_FAILED,
                    format!("failed to create signing request: {e}"),
                )
            })?;

            // Generate proofs for the PCZT (required for finalization).
            // This matches Zashi's two-PCZT flow: full PCZT stays in backend, redacted goes to signer.
            let prover = zcash_proofs::prover::LocalTxProver::bundled();
            let mut pczt_prover = pczt::roles::prover::Prover::new(pczt);

            // Generate Orchard proof if needed
            if pczt_prover.requires_orchard_proof() {
                pczt_prover = pczt_prover
                    .create_orchard_proof(&orchard::circuit::ProvingKey::build())
                    .map_err(|e| {
                        ipc_err(
                            errors::SIGNING_FAILED,
                            format!("failed to create Orchard proof: {e:?}"),
                        )
                    })?;
            }

            // Generate Sapling proofs if needed
            if pczt_prover.requires_sapling_proofs() {
                pczt_prover = pczt_prover
                    .create_sapling_proofs(&prover, &prover)
                    .map_err(|e| {
                        ipc_err(
                            errors::SIGNING_FAILED,
                            format!("failed to create Sapling proofs: {e:?}"),
                        )
                    })?;
            }

            let pczt_with_proofs = pczt_prover.finish();

            // Store full PCZT with proofs, return redacted version for signer
            let pczt_full = zstash_keystone::pczt::encode_pczt_full(&pczt_with_proofs);
            let pczt_for_signer = zstash_keystone::pczt::encode_pczt_for_signer(&pczt_with_proofs);

            Ok::<_, anyhow::Error>((proposal, pczt_full, pczt_for_signer))
        }?;

        let fee = proposal_total_fee(&proposal)?;
        let fee_str = u64::from(fee).to_string();
        let amount_str = amount_u64.to_string();

        let summary = SigningSummary {
            recipient: recipient.to_string(),
            recipient_kind,
            amount: amount_str,
            fee: fee_str.clone(),
            memo_present: memo.is_some(),
            tx_type: TransactionType::Send,
        };

        // Generate a unique signing request ID and store the full PCZT with proofs
        let signing_request_id = Uuid::new_v4().to_string();
        let now = self.clock.now();
        let expires_at = now + PROPOSAL_TTL;

        self.pending_signing_requests.insert(
            signing_request_id.clone(),
            PendingSigningRequest {
                wallet_id,
                pczt_with_proofs: pczt_full,
                expires_at,
            },
        );

        Ok(BuildSigningRequestResponse {
            schema_version: SCHEMA_VERSION,
            signing_request: SigningRequest {
                signing_request_id,
                pczt_payload: pczt_for_signer,
                qr_frames: vec![],
                summary,
            },
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[instrument(skip_all, fields(signing_request_id = %signing_request_id, wallet_id = %wallet_id))]
    pub fn finalize_signing(
        &mut self,
        app_db: &AppDb,
        wallet_id: Uuid,
        network: Network,
        wallet_dir: &Path,
        wallet_dek: &Dek,
        wallet_db_conn: &mut Connection,
        grpc_url: &str,
        signing_request_id: &str,
        signed_payload: &str,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<FinalizeSigningResponse> {
        debug!("Starting finalize_signing");
        let _ = on_tx_changed;

        ensure_backup_complete(app_db, wallet_id)?;

        // Retrieve the stored PCZT with proofs
        let now = self.clock.now();
        let (pending_wallet_id, pending_expires_at) = self
            .pending_signing_requests
            .get(signing_request_id)
            .map(|r| (r.wallet_id, r.expires_at))
            .ok_or_else(|| {
                ipc_err(
                    errors::PROPOSAL_NOT_FOUND,
                    "signing request not found or expired",
                )
            })?;

        // Check expiration
        if now > pending_expires_at {
            self.pending_signing_requests.remove(signing_request_id);
            return Err(ipc_err(errors::PROPOSAL_EXPIRED, "signing request expired"));
        }

        // Check wallet ID matches
        if pending_wallet_id != wallet_id {
            return Err(ipc_err(
                errors::PROPOSAL_NOT_FOUND,
                "signing request not found",
            ));
        }

        let pending_request = self
            .pending_signing_requests
            .remove(signing_request_id)
            .expect("pending signing request should exist");

        // Decode both PCZTs
        debug!("Decoding stored PCZT with proofs");
        let pczt_with_proofs = zstash_keystone::pczt::decode_pczt_full(
            &pending_request.pczt_with_proofs,
        )
        .map_err(|e| {
            error!("Failed to decode stored PCZT: {}", e);
            ipc_err(errors::INTERNAL_ERROR, format!("invalid stored PCZT: {e}"))
        })?;

        debug!("Decoding signed PCZT from hardware wallet");
        let pczt_with_sigs =
            zstash_keystone::pczt::decode_pczt_base64(signed_payload).map_err(|e| {
                error!("Failed to decode signed payload: {}", e);
                ipc_err(errors::INVALID_PCZT, format!("invalid signed payload: {e}"))
            })?;

        // Combine the two PCZTs (proofs + signatures)
        debug!("Combining proved and signed PCZTs");
        let pczt = zstash_keystone::pczt::combine_pczts(pczt_with_proofs, pczt_with_sigs).map_err(
            |e| {
                error!("Failed to combine PCZTs: {}", e);
                ipc_err(
                    errors::SIGNING_FAILED,
                    format!("failed to combine PCZTs: {e}"),
                )
            },
        )?;
        debug!("PCZTs combined successfully");

        let params = zcash_consensus_network(network);

        let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut *wallet_db_conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let prover = zcash_proofs::prover::LocalTxProver::bundled();
        let (sapling_spend_vk, sapling_output_vk) = prover.verifying_keys();
        let orchard_vk = orchard::circuit::VerifyingKey::build();

        debug!("Extracting and storing transaction from combined PCZT");
        let txid =
            zcash_client_backend::data_api::wallet::extract_and_store_transaction_from_pczt::<
                _,
                zcash_client_sqlite::ReceivedNoteId,
            >(
                &mut wdb,
                pczt,
                Some((&sapling_spend_vk, &sapling_output_vk)),
                Some(&orchard_vk),
            )
            .map_err(|e| {
                error!("Failed to extract transaction from PCZT: {}", e);
                ipc_err(
                    errors::SIGNING_FAILED,
                    format!("failed to finalize signing: {e}"),
                )
            })?;
        debug!(%txid, "Transaction extracted from PCZT");

        #[allow(deprecated)]
        use zcash_client_backend::data_api::WalletRead as _;

        let tx = wdb
            .get_transaction(txid)
            .map_err(|e| {
                ipc_err(
                    errors::TRANSACTION_FAILED,
                    format!("failed to load tx: {e}"),
                )
            })?
            .ok_or_else(|| ipc_err(errors::TRANSACTION_FAILED, "tx bytes unavailable"))?;

        let mut tx_bytes = Vec::new();
        tx.write(&mut tx_bytes).map_err(|e| {
            ipc_err(
                errors::TRANSACTION_FAILED,
                format!("failed to serialize tx: {e}"),
            )
        })?;

        let txid_str = txid.to_string();

        if let Err(err) = self.send_transaction_bytes(grpc_url, &tx_bytes) {
            queue_broadcast(
                &self.clock,
                wallet_id,
                wallet_dir,
                wallet_dek,
                txid_str.clone(),
                &tx_bytes,
                Some(format!("{err:#}")),
            )?;
        } else {
            delete_queued_broadcast(wallet_dir, txid_str.clone());
        }

        self.scan_queued_broadcasts(wallet_id, wallet_dir)?;

        debug!(txid = %txid_str, "finalize_signing completed successfully");
        Ok(FinalizeSigningResponse {
            schema_version: SCHEMA_VERSION,
            txid: txid_str,
        })
    }

    pub fn cancel_send(&mut self, proposal_id: &str) -> bool {
        self.proposals.remove(proposal_id).is_some()
    }

    /// Take a proposal out of the service, transferring ownership.
    /// Used by JobService to run the proposal asynchronously.
    pub fn take_proposal(
        &mut self,
        proposal_id: &str,
    ) -> Option<
        zcash_client_backend::proposal::Proposal<
            zcash_client_backend::fees::StandardFeeRule,
            zcash_client_sqlite::ReceivedNoteId,
        >,
    > {
        self.proposals.remove(proposal_id).map(|r| r.proposal)
    }

    /// Clear all pending proposals for a specific wallet.
    /// Called during wallet logout to prevent stale proposals.
    pub fn clear_proposals_for_wallet(&mut self, wallet_id: Uuid) {
        self.proposals
            .retain(|_, record| record.wallet_id != wallet_id);
    }

    #[allow(clippy::too_many_arguments)]
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
        let (proposal_wallet_id, proposal_expires_at) = self
            .proposals
            .get(proposal_id)
            .map(|r| (r.wallet_id, r.expires_at))
            .ok_or_else(|| ipc_err(errors::PROPOSAL_NOT_FOUND, "proposal not found"))?;

        if proposal_wallet_id != wallet_id {
            return Err(ipc_err(errors::PROPOSAL_NOT_FOUND, "proposal not found"));
        }
        if now > proposal_expires_at {
            self.proposals.remove(proposal_id);
            return Err(ipc_err(errors::PROPOSAL_EXPIRED, "proposal expired"));
        }

        let record = self
            .proposals
            .remove(proposal_id)
            .expect("proposal should exist");

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
            params,
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
        .map_err(|e| {
            ipc_err(
                errors::TRANSACTION_FAILED,
                format!("failed to build tx: {e}"),
            )
        })?;

        let mut broadcast_errors: HashMap<String, String> = HashMap::new();

        #[allow(deprecated)]
        use zcash_client_backend::data_api::WalletRead as _;

        for txid in txids.iter() {
            let tx = wdb
                .get_transaction(*txid)
                .map_err(|e| {
                    ipc_err(
                        errors::TRANSACTION_FAILED,
                        format!("failed to load tx: {e}"),
                    )
                })?
                .ok_or_else(|| ipc_err(errors::TRANSACTION_FAILED, "tx bytes unavailable"))?;

            let mut tx_bytes = Vec::new();
            tx.write(&mut tx_bytes).map_err(|e| {
                ipc_err(
                    errors::TRANSACTION_FAILED,
                    format!("failed to serialize tx: {e}"),
                )
            })?;

            if let Err(err) = self.send_transaction_bytes(grpc_url, &tx_bytes) {
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
            Some(entry) => (TransactionStatus::Failed, entry.last_error.clone(), true),
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

    #[allow(clippy::too_many_arguments)]
    pub fn shield_funds(
        &mut self,
        app_db: &AppDb,
        wallet_id: Uuid,
        network: Network,
        wallet_dir: &Path,
        wallet_dek: &Dek,
        wallet_db_conn: &mut Connection,
        grpc_url: &str,
        account_id: u32,
        consolidate: bool,
        spending_key: zcash_client_backend::keys::UnifiedSpendingKey,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<ShieldFundsResponse> {
        ensure_spend_allowed(app_db, wallet_id, account_id)?;
        let _ = consolidate;

        #[allow(deprecated)]
        use zcash_client_backend::data_api::{InputSource as _, WalletRead as _};
        use zcash_client_backend::fees::ChangeStrategy as _;
        use zcash_primitives::transaction::fees::transparent as transparent_fees;

        let account_uuid = resolve_wallet_account_uuid(wallet_db_conn, network, account_id)
            .context("failed to resolve wallet account")?;

        let params = zcash_consensus_network(network);

        let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut *wallet_db_conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let receivers = wdb
            .get_transparent_receivers(account_uuid, false, false)
            .context("failed to list transparent receivers")?;
        let from_addrs: Vec<_> = receivers.into_keys().collect();

        let chain_tip_height = wdb
            .chain_height()
            .context("failed to read chain height")?
            .ok_or_else(|| ipc_err(errors::TRANSACTION_FAILED, "must scan blocks first"))?;
        let target_height: zcash_client_backend::data_api::wallet::TargetHeight =
            (chain_tip_height + 1).into();
        let confirmations_policy =
            zcash_client_backend::data_api::wallet::ConfirmationsPolicy::default();

        let mut transparent_inputs = Vec::new();
        for addr in from_addrs.iter() {
            let outputs = wdb
                .get_spendable_transparent_outputs(addr, target_height, confirmations_policy)
                .context("failed to list transparent outputs")?;
            transparent_inputs.extend(outputs.into_iter().map(|u| u.into_wallet_output()));
        }

        if transparent_inputs.is_empty() {
            return Err(ipc_err(
                errors::INSUFFICIENT_FUNDS,
                "no transparent funds to shield",
            ));
        }

        let fee_rule = zcash_client_backend::fees::StandardFeeRule::Zip317;
        let change_strategy = zcash_client_backend::fees::standard::SingleOutputChangeStrategy::new(
            fee_rule,
            None,
            zcash_protocol::ShieldedProtocol::Orchard,
            zcash_client_backend::fees::DustOutputPolicy::default(),
        );

        let spending_keys =
            zcash_client_backend::data_api::wallet::SpendingKeys::from_unified_spending_key(
                spending_key,
            );
        let prover = zcash_proofs::prover::LocalTxProver::bundled();

        let batches: Vec<Vec<_>> = transparent_inputs
            .chunks(MAX_SHIELDING_INPUTS_PER_TX)
            .map(|chunk| chunk.to_vec())
            .collect();

        let mut primary_txid: Option<String> = None;
        let mut primary_fee: Option<u64> = None;

        for batch in batches {
            if batch.is_empty() {
                continue;
            }

            let mut input_selection = batch;
            change_strategy
                .fetch_wallet_meta(&wdb, account_uuid, target_height, &[])
                .context("failed to load wallet metadata for shielding")?;

            let balance = loop {
                #[derive(Debug)]
                struct Zip317P2pkhTransparentInput<'a> {
                    utxo: &'a zcash_client_backend::wallet::WalletTransparentOutput,
                }

                impl transparent_fees::InputView for Zip317P2pkhTransparentInput<'_> {
                    fn outpoint(&self) -> &zcash_transparent::bundle::OutPoint {
                        self.utxo.outpoint()
                    }

                    fn coin(&self) -> &zcash_transparent::bundle::TxOut {
                        self.utxo.txout()
                    }

                    fn serialized_size(&self) -> transparent_fees::InputSize {
                        match self.utxo.recipient_address() {
                            zcash_transparent::address::TransparentAddress::PublicKeyHash(_) => {
                                transparent_fees::InputSize::Known(149)
                            }
                            _ => transparent_fees::InputSize::Unknown(self.utxo.outpoint().clone()),
                        }
                    }
                }

                let input_views: Vec<_> = input_selection
                    .iter()
                    .map(|utxo| Zip317P2pkhTransparentInput { utxo })
                    .collect();

                match change_strategy.compute_balance::<_, std::convert::Infallible>(
                    &params,
                    target_height,
                    &input_views,
                    &[] as &[zcash_transparent::bundle::TxOut],
                    &zcash_client_backend::fees::sapling::EmptyBundleView,
                    &zcash_client_backend::fees::orchard::EmptyBundleView,
                    None,
                    &(),
                ) {
                    Ok(balance) => break Some(balance),
                    Err(zcash_client_backend::fees::ChangeError::DustInputs {
                        transparent,
                        ..
                    }) => {
                        let exclusions: BTreeSet<zcash_transparent::bundle::OutPoint> =
                            transparent.into_iter().collect();
                        input_selection.retain(|i| !exclusions.contains(i.outpoint()));
                        if input_selection.is_empty() {
                            break None;
                        }
                    }
                    Err(zcash_client_backend::fees::ChangeError::InsufficientFunds {
                        available,
                        required,
                    }) => {
                        let required_u64 = u64::from(required);
                        let details = serde_json::json!({
                            "required_minimum_zatoshis": required_u64.to_string(),
                            "available_zatoshis": u64::from(available).to_string(),
                            "estimated_fee_zatoshis": required_u64.to_string(),
                        });
                        return Err(anyhow::anyhow!(
                            crate::error::EngineIpcError::new(
                                errors::INSUFFICIENT_FUNDS,
                                "insufficient transparent balance to cover shielding fee",
                            )
                            .with_details(details)
                        ));
                    }
                    Err(other) => {
                        return Err(ipc_err(
                            errors::TRANSACTION_FAILED,
                            format!("failed to compute shielding balance: {other}"),
                        ));
                    }
                }
            };

            if input_selection.is_empty() {
                continue;
            }
            let Some(balance) = balance else {
                continue;
            };

            let input_total: u64 = input_selection
                .iter()
                .map(|i| u64::from(i.value()))
                .fold(0u64, |acc, v| acc.saturating_add(v));
            let fee_u64: u64 = u64::from(balance.fee_required());

            let proposal = zcash_client_backend::proposal::Proposal::<
                zcash_client_backend::fees::StandardFeeRule,
                std::convert::Infallible,
            >::single_step(
                zcash_client_backend::zip321::TransactionRequest::empty(),
                BTreeMap::new(),
                input_selection,
                None,
                balance,
                fee_rule,
                target_height,
                true,
            )
            .map_err(|e| {
                ipc_err(
                    errors::TRANSACTION_FAILED,
                    format!("invalid shielding proposal: {e}"),
                )
            })?;

            let txids = zcash_client_backend::data_api::wallet::create_proposed_transactions::<
                _,
                _,
                std::convert::Infallible,
                _,
                std::convert::Infallible,
                std::convert::Infallible,
            >(
                &mut wdb,
                &params,
                &prover,
                &prover,
                &spending_keys,
                zcash_client_backend::wallet::OvkPolicy::Sender,
                &proposal,
            )
            .map_err(|e| {
                ipc_err(
                    errors::TRANSACTION_FAILED,
                    format!("failed to build shielding tx: {e}"),
                )
            })?;

            #[allow(deprecated)]
            use zcash_client_backend::data_api::WalletRead as _;

            for txid in txids.iter() {
                let tx = wdb
                    .get_transaction(*txid)
                    .map_err(|e| {
                        ipc_err(
                            errors::TRANSACTION_FAILED,
                            format!("failed to load tx: {e}"),
                        )
                    })?
                    .ok_or_else(|| ipc_err(errors::TRANSACTION_FAILED, "tx bytes unavailable"))?;

                let mut tx_bytes = Vec::new();
                tx.write(&mut tx_bytes).map_err(|e| {
                    ipc_err(
                        errors::TRANSACTION_FAILED,
                        format!("failed to serialize tx: {e}"),
                    )
                })?;

                if let Err(err) = self.send_transaction_bytes(grpc_url, &tx_bytes) {
                    let err_msg = format!("{err:#}");
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

                let txid_str = txid.to_string();
                if primary_txid.is_none() {
                    primary_txid = Some(txid_str.clone());
                    primary_fee = Some(fee_u64);
                }

                self.scan_queued_broadcasts(wallet_id, wallet_dir)?;

                if let Some(handler) = on_tx_changed.as_ref() {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    let queued = self
                        .queued_broadcasts
                        .get(&wallet_id)
                        .and_then(|m| m.get(&txid_str))
                        .cloned();

                    let (status, last_error, can_retry_broadcast) = match queued {
                        Some(entry) => (TransactionStatus::Failed, entry.last_error.clone(), true),
                        None => (TransactionStatus::Pending, None, false),
                    };

                    let shielded_value = input_total.saturating_sub(fee_u64).to_string();
                    handler(TransactionChangedEvent {
                        schema_version: SCHEMA_VERSION,
                        event: "tx.changed".to_string(),
                        transaction: TransactionInfo {
                            txid: txid_str,
                            account_id,
                            tx_type: TransactionType::Shield,
                            value: shielded_value,
                            fee: fee_u64.to_string(),
                            memo_present: false,
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
            }
        }

        let Some(primary_txid) = primary_txid else {
            return Err(ipc_err(
                errors::INSUFFICIENT_FUNDS,
                "no transparent funds to shield",
            ));
        };
        let fee = primary_fee.unwrap_or(0).to_string();

        Ok(ShieldFundsResponse {
            schema_version: SCHEMA_VERSION,
            txid: primary_txid,
            fee,
        })
    }

    #[allow(clippy::too_many_arguments)]
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

        // Parse hex txid to bytes (reversed for internal representation)
        let txid_bytes: Vec<u8> = hex::decode(txid)
            .map_err(|_| ipc_err(errors::TRANSACTION_FAILED, "invalid txid hex"))?
            .into_iter()
            .rev()
            .collect();

        // Query v_tx_outputs (v_tx_sent was removed in zcash_client_sqlite 0.6.0)
        // Get hd_account_index which is the user-facing account ID (ZIP-32 account index)
        let account_id: Option<i64> = wallet_db_conn
            .query_row(
                "SELECT a.hd_account_index FROM v_tx_outputs vo \
                 JOIN accounts a ON a.uuid = vo.from_account_uuid \
                 WHERE vo.txid = ?1 AND vo.from_account_uuid IS NOT NULL \
                 LIMIT 1",
                [&txid_bytes],
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
            .ok_or_else(|| {
                ipc_err(
                    errors::QUEUED_BROADCAST_NOT_FOUND,
                    "queued broadcast not found",
                )
            })?;

        let tx_bytes = decrypt_queued_tx_bytes(wallet_id, wallet_dek, &entry.bin_path)?;

        match self.send_transaction_bytes(grpc_url, &tx_bytes) {
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

    #[allow(clippy::too_many_arguments)]
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

        let account_uuid = resolve_wallet_account_uuid(wallet_db_conn, network, account_id)
            .context("failed to resolve wallet account")?;
        let account_uuid_bytes = account_uuid.expose_uuid().as_bytes().to_vec();

        let total_count: i64 = wallet_db_conn.query_row(
            "SELECT COUNT(*) FROM v_transactions WHERE account_uuid = ?1",
            [account_uuid_bytes.clone()],
            |row| row.get(0),
        )?;
        let total_count = total_count.max(0) as u32;

        let chain_height = current_chain_height(wallet_db_conn, network);

        let mut stmt = wallet_db_conn.prepare(
            "SELECT txid,\n                    mined_height,\n                    expiry_height,\n                    fee_paid,\n                    total_spent,\n                    total_received,\n                    memo_count,\n                    block_time,\n                    is_shielding,\n                    sent_note_count,\n                    received_note_count\n             FROM v_transactions\n             WHERE account_uuid = ?1\n             ORDER BY COALESCE(block_time, 0) DESC, txid DESC\n             LIMIT ?2 OFFSET ?3",
        )?;

        let mut rows = stmt.query(params![account_uuid_bytes, limit as i64, offset as i64])?;
        let mut transactions = Vec::new();

        while let Some(row) = rows.next()? {
            let txid_bytes: Vec<u8> = row.get(0)?;
            let txid = if txid_bytes.len() == 32 {
                let mut buf = [0u8; 32];
                buf.copy_from_slice(&txid_bytes);
                zcash_protocol::TxId::from_bytes(buf).to_string()
            } else {
                return Err(ipc_err(errors::TRANSACTION_FAILED, "invalid txid encoding"));
            };
            let mined_height: Option<i64> = row.get(1)?;
            let expiry_height: Option<i64> = row.get(2)?;
            let fee_paid: Option<i64> = row.get(3)?;
            let total_spent: i64 = row.get(4)?;
            let total_received: i64 = row.get(5)?;
            let memo_count: i64 = row.get(6)?;
            let block_time: Option<i64> = row.get(7)?;
            let is_shielding: bool = row.get(8)?;
            let sent_note_count: i64 = row.get(9)?;
            let received_note_count: i64 = row.get(10)?;

            let tx_type = if is_shielding {
                TransactionType::Shield
            } else if sent_note_count > 0 {
                TransactionType::Send
            } else if received_note_count > 0 {
                TransactionType::Receive
            } else {
                TransactionType::Consolidate
            };

            let total_spent_u64 = u64::try_from(total_spent.max(0)).unwrap_or(0);
            let total_received_u64 = u64::try_from(total_received.max(0)).unwrap_or(0);
            let fee_u64 = fee_paid
                .and_then(|f| u64::try_from(f.max(0)).ok())
                .unwrap_or(0);

            let value_u64 = match tx_type {
                TransactionType::Send => total_spent_u64
                    .saturating_sub(total_received_u64)
                    .saturating_sub(fee_u64),
                TransactionType::Shield => total_received_u64,
                TransactionType::Receive => total_received_u64,
                TransactionType::Consolidate => total_received_u64,
            };

            let mined_height_u32 = mined_height.and_then(|h| u32::try_from(h).ok());
            let expiry_height_u32 = expiry_height.and_then(|h| u32::try_from(h).ok());
            let confirmed_at = block_time.map(|t| t.saturating_mul(1000));

            let mut status = if mined_height_u32.is_some() {
                TransactionStatus::Confirmed
            } else {
                TransactionStatus::Pending
            };

            if mined_height_u32.is_none()
                && let Some(expiry) = expiry_height_u32
                && let Some(height) = chain_height
                && height > expiry
            {
                status = TransactionStatus::Expired;
                delete_queued_broadcast(wallet_dir, txid.clone());
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

            // Fetch memo content if memos exist for this transaction
            let memo = if memo_count > 0 {
                fetch_transaction_memos(wallet_db_conn, &txid_bytes)?
            } else {
                None
            };

            transactions.push(TransactionInfo {
                txid,
                account_id,
                tx_type,
                value: value_u64.to_string(),
                fee: fee_u64.to_string(),
                memo_present: memo_count > 0,
                memo,
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

/// Fetches all non-empty memos for a transaction from received and sent notes tables.
/// Returns memos concatenated with newlines, or None if no memos found.
fn fetch_transaction_memos(conn: &Connection, txid_bytes: &[u8]) -> anyhow::Result<Option<String>> {
    // Query all memo sources using UNION - orchard, sapling received notes and sent notes
    let mut stmt = conn.prepare(
        "SELECT memo FROM orchard_received_notes
         JOIN transactions ON transactions.id_tx = orchard_received_notes.transaction_id
         WHERE transactions.txid = ?1 AND memo IS NOT NULL AND memo != X'F6'
         UNION ALL
         SELECT memo FROM sapling_received_notes
         JOIN transactions ON transactions.id_tx = sapling_received_notes.transaction_id
         WHERE transactions.txid = ?1 AND memo IS NOT NULL AND memo != X'F6'
         UNION ALL
         SELECT memo FROM sent_notes
         JOIN transactions ON transactions.id_tx = sent_notes.transaction_id
         WHERE transactions.txid = ?1 AND memo IS NOT NULL AND memo != X'F6'",
    )?;

    let mut rows = stmt.query([txid_bytes])?;
    let mut memos = Vec::new();

    while let Some(row) = rows.next()? {
        let memo_bytes: Vec<u8> = row.get(0)?;
        // Try to parse as MemoBytes and convert to text
        if let Ok(memo_bytes_obj) = zcash_protocol::memo::MemoBytes::from_bytes(&memo_bytes)
            && let Ok(memo) = zcash_protocol::memo::Memo::try_from(memo_bytes_obj)
        {
            match memo {
                zcash_protocol::memo::Memo::Text(text) => {
                    memos.push(text.to_string());
                }
                zcash_protocol::memo::Memo::Empty => {}
                _ => {
                    // For Future or Arbitrary memos, show as hex
                    memos.push(format!("[binary: {} bytes]", memo_bytes.len()));
                }
            }
        }
    }

    if memos.is_empty() {
        Ok(None)
    } else {
        Ok(Some(memos.join("\n")))
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

fn ensure_backup_complete(app_db: &AppDb, wallet_id: Uuid) -> anyhow::Result<()> {
    let backup_required =
        crate::db::backup_meta::get_backup_required(app_db.conn(), wallet_id)?.unwrap_or(true);
    if backup_required {
        return Err(ipc_err(errors::BACKUP_REQUIRED, "backup required"));
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
    let addr = crate::address_service::decode_address(network, recipient)?;

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

    if let Some(memo) = memo
        && memo.len() > 512
    {
        return Err(ipc_err(errors::MEMO_TOO_LONG, "memo too long"));
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

    for account_uuid in wdb
        .get_account_ids()
        .context("failed to list wallet accounts")?
    {
        let Some(account) = wdb
            .get_account(account_uuid)
            .context("failed to load account")?
        else {
            continue;
        };
        // Check key_source first (software wallets, zSTASH-tagged imports including HardwareSigner)
        if let Some(key_source) = account.source().key_source()
            && crate::account_key_source::parse_account_id_from_key_source(key_source)
                == Some(account_id)
        {
            return Ok(account_uuid);
        }

        // Then check key_derivation (hardware wallets with ZIP-32 derivation)
        if let Some(derivation) = account.source().key_derivation() {
            let idx: u32 = derivation.account_index().into();
            if idx == account_id {
                return Ok(account_uuid);
            }
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
    std::fs::create_dir_all(&queue_dir).with_context(|| {
        format!(
            "failed to create queued broadcasts dir: {}",
            queue_dir.display()
        )
    })?;

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
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?).with_context(|| {
        format!(
            "failed to write queued tx metadata: {}",
            meta_path.display()
        )
    })?;

    Ok(())
}

fn decrypt_queued_tx_bytes(
    wallet_id: Uuid,
    wallet_dek: &Dek,
    bin_path: &Path,
) -> anyhow::Result<Vec<u8>> {
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

impl<C: Clock> TxService<C> {
    fn send_transaction_bytes(&self, grpc_url: &str, tx_bytes: &[u8]) -> anyhow::Result<()> {
        let client = match self.tor_manager.as_ref() {
            Some(tor) => zstash_network::grpc_client::GrpcClient::new_with_tor(
                grpc_url.to_string(),
                Arc::clone(tor),
            ),
            None => zstash_network::grpc_client::GrpcClient::new(grpc_url.to_string()),
        };

        let tx_bytes = tx_bytes.to_vec();
        block_on(async move { client.send_transaction(tx_bytes).await })
    }
}

fn delete_queued_broadcast(wallet_dir: &Path, txid: String) {
    let queue_dir = queued_broadcasts_dir(wallet_dir);
    let bin_path = queued_broadcast_bin_path(&queue_dir, &txid);
    let meta_path = queued_broadcast_meta_path(&queue_dir, &txid);
    if let Err(e) = std::fs::remove_file(&bin_path) {
        tracing::debug!(path = ?bin_path, error = ?e, "failed to delete queued broadcast bin file");
    }
    if let Err(e) = std::fs::remove_file(&meta_path) {
        tracing::debug!(path = ?meta_path, error = ?e, "failed to delete queued broadcast meta file");
    }
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
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&updated)?).with_context(|| {
        format!(
            "failed to update queued broadcast meta: {}",
            meta_path.display()
        )
    })?;
    Ok(())
}

fn to_unix_ms(time: SystemTime) -> anyhow::Result<i64> {
    let dur = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ipc_err(errors::INTERNAL_ERROR, "time went backwards"))?;
    i64::try_from(dur.as_millis())
        .map_err(|_| ipc_err(errors::INTERNAL_ERROR, "timestamp overflow"))
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use rusqlite::Connection;
    use uuid::Uuid;

    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct TestClock(SystemTime);

    impl Clock for TestClock {
        fn now(&self) -> SystemTime {
            self.0
        }
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("zstash_{prefix}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn set_backup_not_required(app_db: &AppDb, wallet_id: Uuid) {
        let wallet = zstash_core::domain::WalletInfo {
            id: wallet_id,
            name: "Test Wallet".to_string(),
            wallet_type: zstash_core::domain::WalletType::Software,
            network: Network::Testnet,
            remember_unlock_enabled: false,
            created_at: 0,
            last_opened_at: Some(0),
        };
        crate::db::wallet_meta::insert_wallet(app_db.conn(), &wallet, "/tmp")
            .expect("insert wallet");
        crate::db::backup_meta::set_backup_required(app_db.conn(), wallet_id, false)
            .expect("set backup_required=false");
    }

    #[test]
    fn finalize_signing_does_not_consume_request_on_wallet_mismatch() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let clock = TestClock(now);
        let mut service = TxService::new(clock);

        let signing_wallet_id = Uuid::new_v4();
        let other_wallet_id = Uuid::new_v4();
        let signing_request_id = Uuid::new_v4().to_string();

        service.pending_signing_requests.insert(
            signing_request_id.clone(),
            PendingSigningRequest {
                wallet_id: signing_wallet_id,
                pczt_with_proofs: "not-a-pczt".to_string(),
                expires_at: now + Duration::from_secs(60),
            },
        );

        let root = temp_root("tx_service_wallet_mismatch");
        let app_db = AppDb::open(root.join("app.db")).expect("open app db");
        set_backup_not_required(&app_db, other_wallet_id);

        let wallet_dir = root.join("wallet");
        let wallet_dek = Dek([0u8; 32]);
        let mut wallet_db_conn = Connection::open_in_memory().expect("open wallet db");

        let err = service
            .finalize_signing(
                &app_db,
                other_wallet_id,
                Network::Testnet,
                &wallet_dir,
                &wallet_dek,
                &mut wallet_db_conn,
                "grpc://example.invalid",
                &signing_request_id,
                "",
                None,
            )
            .expect_err("wallet mismatch should fail");

        let ipc = crate::error::find_engine_ipc_error(&err).expect("engine ipc error");
        assert_eq!(ipc.code, errors::PROPOSAL_NOT_FOUND);
        assert!(
            service
                .pending_signing_requests
                .contains_key(&signing_request_id),
            "pending signing request should remain"
        );
    }

    #[test]
    fn finalize_signing_removes_expired_request() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let clock = TestClock(now);
        let mut service = TxService::new(clock);

        let wallet_id = Uuid::new_v4();
        let signing_request_id = Uuid::new_v4().to_string();

        service.pending_signing_requests.insert(
            signing_request_id.clone(),
            PendingSigningRequest {
                wallet_id,
                pczt_with_proofs: "not-a-pczt".to_string(),
                expires_at: now - Duration::from_secs(1),
            },
        );

        let root = temp_root("tx_service_expired_request");
        let app_db = AppDb::open(root.join("app.db")).expect("open app db");
        set_backup_not_required(&app_db, wallet_id);

        let wallet_dir = root.join("wallet");
        let wallet_dek = Dek([0u8; 32]);
        let mut wallet_db_conn = Connection::open_in_memory().expect("open wallet db");

        let err = service
            .finalize_signing(
                &app_db,
                wallet_id,
                Network::Testnet,
                &wallet_dir,
                &wallet_dek,
                &mut wallet_db_conn,
                "grpc://example.invalid",
                &signing_request_id,
                "",
                None,
            )
            .expect_err("expired request should fail");

        let ipc = crate::error::find_engine_ipc_error(&err).expect("engine ipc error");
        assert_eq!(ipc.code, errors::PROPOSAL_EXPIRED);
        assert!(
            !service
                .pending_signing_requests
                .contains_key(&signing_request_id),
            "expired signing request should be removed"
        );
    }

    /// Creates a minimal ProposalRecord for testing expiration and wallet mismatch logic.
    /// The proposal itself is not used - only wallet_id and expires_at matter for these tests.
    fn test_proposal_record(wallet_id: Uuid, expires_at: SystemTime) -> ProposalRecord {
        let fee = zcash_protocol::value::Zatoshis::ZERO;
        let balance = zcash_client_backend::fees::TransactionBalance::new(vec![], fee).unwrap();
        let target_height: zcash_client_backend::data_api::wallet::TargetHeight =
            zcash_protocol::consensus::BlockHeight::from_u32(1).into();

        let proposal = zcash_client_backend::proposal::Proposal::<
            zcash_client_backend::fees::StandardFeeRule,
            zcash_client_sqlite::ReceivedNoteId,
        >::single_step(
            zcash_client_backend::zip321::TransactionRequest::empty(),
            BTreeMap::new(),
            vec![],
            None,
            balance,
            zcash_client_backend::fees::StandardFeeRule::Zip317,
            target_height,
            false,
        )
        .expect("create test proposal");

        ProposalRecord {
            wallet_id,
            account_id: 0,
            expires_at,
            proposal,
            summary: TransactionSummary {
                recipient: "test".to_string(),
                recipient_kind: zstash_core::domain::RecipientKind::Orchard,
                amount: "0".to_string(),
                fee: "0".to_string(),
                memo_present: false,
                total_spend: "0".to_string(),
            },
        }
    }

    #[test]
    fn confirm_send_does_not_consume_proposal_on_wallet_mismatch() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let clock = TestClock(now);
        let mut service = TxService::new(clock);

        let proposal_wallet_id = Uuid::new_v4();
        let other_wallet_id = Uuid::new_v4();
        let proposal_id = Uuid::new_v4().to_string();

        service.proposals.insert(
            proposal_id.clone(),
            test_proposal_record(proposal_wallet_id, now + Duration::from_secs(60)),
        );

        let root = temp_root("tx_service_confirm_wallet_mismatch");
        let app_db = AppDb::open(root.join("app.db")).expect("open app db");
        set_backup_not_required(&app_db, other_wallet_id);

        let wallet_dir = root.join("wallet");
        let wallet_dek = Dek([0u8; 32]);
        let mut wallet_db_conn = Connection::open_in_memory().expect("open wallet db");

        // Create a dummy spending key - it won't be used since we fail early on wallet mismatch
        let seed = [0u8; 32];
        let spending_key = zcash_client_backend::keys::UnifiedSpendingKey::from_seed(
            &zcash_protocol::consensus::Network::TestNetwork,
            &seed,
            zip32::AccountId::ZERO,
        )
        .expect("create test spending key");

        let err = service
            .confirm_send(
                &app_db,
                other_wallet_id,
                Network::Testnet,
                &wallet_dir,
                &wallet_dek,
                &mut wallet_db_conn,
                "grpc://example.invalid",
                &proposal_id,
                spending_key,
                None,
            )
            .expect_err("wallet mismatch should fail");

        let ipc = crate::error::find_engine_ipc_error(&err).expect("engine ipc error");
        assert_eq!(ipc.code, errors::PROPOSAL_NOT_FOUND);
        assert!(
            service.proposals.contains_key(&proposal_id),
            "proposal should remain after wallet mismatch"
        );
    }

    #[test]
    fn confirm_send_removes_expired_proposal() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let clock = TestClock(now);
        let mut service = TxService::new(clock);

        let wallet_id = Uuid::new_v4();
        let proposal_id = Uuid::new_v4().to_string();

        // Insert an already-expired proposal
        service.proposals.insert(
            proposal_id.clone(),
            test_proposal_record(wallet_id, now - Duration::from_secs(1)),
        );

        let root = temp_root("tx_service_confirm_expired");
        let app_db = AppDb::open(root.join("app.db")).expect("open app db");
        set_backup_not_required(&app_db, wallet_id);

        let wallet_dir = root.join("wallet");
        let wallet_dek = Dek([0u8; 32]);
        let mut wallet_db_conn = Connection::open_in_memory().expect("open wallet db");

        let seed = [0u8; 32];
        let spending_key = zcash_client_backend::keys::UnifiedSpendingKey::from_seed(
            &zcash_protocol::consensus::Network::TestNetwork,
            &seed,
            zip32::AccountId::ZERO,
        )
        .expect("create test spending key");

        let err = service
            .confirm_send(
                &app_db,
                wallet_id,
                Network::Testnet,
                &wallet_dir,
                &wallet_dek,
                &mut wallet_db_conn,
                "grpc://example.invalid",
                &proposal_id,
                spending_key,
                None,
            )
            .expect_err("expired proposal should fail");

        let ipc = crate::error::find_engine_ipc_error(&err).expect("engine ipc error");
        assert_eq!(ipc.code, errors::PROPOSAL_EXPIRED);
        assert!(
            !service.proposals.contains_key(&proposal_id),
            "expired proposal should be removed"
        );
    }
}
