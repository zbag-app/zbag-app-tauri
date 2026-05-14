//! Transaction commands.

use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand};
use zeroize::Zeroizing;

use zstash_core::ipc::v1::commands::wallet::ReauthPurpose;

use crate::cli_app_state::CliAppState;
use crate::output::OutputMode;
use crate::password;

#[derive(Args)]
pub struct TxArgs {
    #[command(subcommand)]
    command: TxCommand,
}

#[derive(Subcommand)]
enum TxCommand {
    /// Send ZEC to an address
    Send {
        /// Account ID (usually 0)
        account_id: u32,

        /// Recipient address (shielded or transparent)
        #[arg(long)]
        to: String,

        /// Amount in zatoshis
        #[arg(long)]
        amount: String,

        /// Optional memo (for shielded recipients)
        #[arg(long)]
        memo: Option<String>,

        /// Allow sending to transparent addresses (privacy warning)
        #[arg(long)]
        allow_transparent: bool,

        /// Wallet ID or prefix
        #[arg(short, long)]
        wallet: Option<String>,

        /// Password (will prompt if not provided)
        ///
        /// SECURITY: Avoid passing passwords via CLI arguments when possible.
        /// They may be stored in shell history or visible to other processes.
        #[arg(short, long)]
        password: Option<String>,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },

    /// Shield transparent funds
    Shield {
        /// Account ID (usually 0)
        account_id: u32,

        /// Consolidate existing shielded notes
        #[arg(long)]
        consolidate: bool,

        /// Wallet ID or prefix
        #[arg(short, long)]
        wallet: Option<String>,

        /// Password (will prompt if not provided)
        ///
        /// SECURITY: Avoid passing passwords via CLI arguments when possible.
        /// They may be stored in shell history or visible to other processes.
        #[arg(short, long)]
        password: Option<String>,
    },

    /// List transactions
    List {
        /// Account ID (usually 0)
        account_id: u32,

        /// Maximum number of transactions to list
        #[arg(long, default_value = "20")]
        limit: u32,

        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,

        /// Wallet ID or prefix
        #[arg(short, long)]
        wallet: Option<String>,

        /// Password (will prompt if wallet is locked)
        ///
        /// SECURITY: Avoid passing passwords via CLI arguments when possible.
        /// They may be stored in shell history or visible to other processes.
        #[arg(short, long)]
        password: Option<String>,
    },

    /// Retry broadcasting a failed/pending transaction
    Retry {
        /// Transaction ID to retry
        txid: String,

        /// Wallet ID or prefix
        #[arg(short, long)]
        wallet: Option<String>,

        /// Password (will prompt if not provided)
        ///
        /// SECURITY: Avoid passing passwords via CLI arguments when possible.
        /// They may be stored in shell history or visible to other processes.
        #[arg(short, long)]
        password: Option<String>,
    },
}

pub async fn run(
    args: TxArgs,
    data_dir: &Path,
    enable_tor: bool,
    output: &OutputMode,
) -> Result<()> {
    match args.command {
        TxCommand::Send {
            account_id,
            to,
            amount,
            memo,
            allow_transparent,
            wallet,
            password,
            yes,
        } => {
            let provided_password = password::wrap_password_arg(password)?;
            let state = CliAppState::new(data_dir, enable_tor)?;
            let (wallet_info, unlocked) = load_wallet(&state, wallet.as_deref())?;
            let unlock_password =
                unlock_if_needed(&state, wallet_info.id, unlocked, provided_password.as_ref())?;

            // Prepare the transaction
            let prepare_response = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
                wm.prepare_send(
                    account_id,
                    &to,
                    &amount,
                    memo.as_deref(),
                    allow_transparent,
                    &mut tx_svc,
                )?
            };

            // Show summary and confirm
            if !output.is_json() && !yes {
                println!("Transaction Summary:");
                println!("  To:     {}", prepare_response.summary.recipient);
                println!("  Amount: {} zatoshis", prepare_response.summary.amount);
                println!("  Fee:    {} zatoshis", prepare_response.summary.fee);
                println!(
                    "  Total:  {} zatoshis",
                    prepare_response.summary.total_spend
                );
                println!();

                if !password::confirm_action("Send this transaction?")? {
                    output.print_message("Transaction cancelled");
                    return Ok(());
                }
            }

            // Re-authenticate for spending
            let prompted_reauth_password =
                if provided_password.is_some() || unlock_password.is_some() {
                    None
                } else {
                    Some(password::get_password(None, "Password: ")?)
                };
            let reauth_token = {
                let reauth_password = if let Some(p) = provided_password.as_ref() {
                    p
                } else if let Some(p) = unlock_password.as_ref() {
                    p
                } else {
                    prompted_reauth_password.as_ref().expect("prompted above")
                };

                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let (token, _expires) =
                    wm.reauth_wallet(wallet_info.id, reauth_password, ReauthPurpose::Spend)?;
                token
            };
            drop(prompted_reauth_password);
            drop(unlock_password);
            drop(provided_password);

            // Confirm and broadcast (two-phase pattern to release mutex during proving)
            let confirm_response = {
                // Phase 1: Get context
                let (ctx, spending_key) = {
                    let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                    let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
                    wm.prepare_confirm_send(
                        &prepare_response.proposal_id,
                        &reauth_token,
                        &mut tx_svc,
                    )?
                };

                // Phase 2: Expensive operations outside mutex
                let mut conn = zstash_engine::wallet_manager::open_wallet_db_for_tx(&ctx)?;
                let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
                tx_svc.confirm_send(
                    &ctx.app_db_path,
                    ctx.wallet_id,
                    ctx.network,
                    &ctx.wallet_dir,
                    &ctx.dek,
                    &mut conn,
                    &ctx.grpc_url,
                    &prepare_response.proposal_id,
                    spending_key,
                    None,
                )?
            };

            output.print_tx_sent(&confirm_response.txid);
        }

        TxCommand::Shield {
            account_id,
            consolidate,
            wallet,
            password,
        } => {
            let provided_password = password::wrap_password_arg(password)?;
            let state = CliAppState::new(data_dir, enable_tor)?;
            let (wallet_info, unlocked) = load_wallet(&state, wallet.as_deref())?;
            let unlock_password =
                unlock_if_needed(&state, wallet_info.id, unlocked, provided_password.as_ref())?;

            // Re-authenticate for spending
            let prompted_reauth_password =
                if provided_password.is_some() || unlock_password.is_some() {
                    None
                } else {
                    Some(password::get_password(None, "Password: ")?)
                };
            let reauth_token = {
                let reauth_password = if let Some(p) = provided_password.as_ref() {
                    p
                } else if let Some(p) = unlock_password.as_ref() {
                    p
                } else {
                    prompted_reauth_password.as_ref().expect("prompted above")
                };

                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let (token, _expires) =
                    wm.reauth_wallet(wallet_info.id, reauth_password, ReauthPurpose::Spend)?;
                token
            };
            drop(prompted_reauth_password);
            drop(unlock_password);
            drop(provided_password);

            // Shield funds (two-phase pattern to release mutex during proving)
            let response = {
                // Phase 1: Get context
                let (ctx, spending_key) = {
                    let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                    wm.prepare_shield_funds(account_id, &reauth_token)?
                };

                // Phase 2: Expensive operations outside mutex
                let mut conn = zstash_engine::wallet_manager::open_wallet_db_for_tx(&ctx)?;
                let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
                tx_svc.shield_funds(
                    &ctx.app_db_path,
                    ctx.wallet_id,
                    ctx.network,
                    &ctx.wallet_dir,
                    &ctx.dek,
                    &mut conn,
                    &ctx.grpc_url,
                    account_id,
                    consolidate,
                    spending_key,
                    None,
                )?
            };

            output.print_tx_sent(&response.txid);
        }

        TxCommand::List {
            account_id,
            limit,
            offset,
            wallet,
            password,
        } => {
            let provided_password = password::wrap_password_arg(password)?;
            let state = CliAppState::new(data_dir, false)?;
            let (wallet_info, unlocked) = load_wallet(&state, wallet.as_deref())?;
            let unlock_password =
                unlock_if_needed(&state, wallet_info.id, unlocked, provided_password.as_ref())?;
            drop(unlock_password);
            drop(provided_password);

            // List transactions
            let response = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
                wm.list_transactions(account_id, limit, offset, &mut tx_svc)?
            };

            if output.is_json() {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!(
                    "Transactions (showing {} of {}):",
                    response.transactions.len(),
                    response.total_count
                );
                for tx in &response.transactions {
                    let status = format!("{:?}", tx.status);
                    let tx_type = format!("{:?}", tx.tx_type);
                    let amount_display = format!("{} zatoshis", tx.value);
                    let height_display = tx
                        .mined_height
                        .map(|h| h.to_string())
                        .unwrap_or_else(|| "pending".to_string());
                    println!(
                        "  {} | {:>10} | {:>8} | {} | {}",
                        &tx.txid[..8],
                        amount_display,
                        tx_type,
                        status,
                        height_display
                    );
                }
            }
        }

        TxCommand::Retry {
            txid,
            wallet,
            password,
        } => {
            let provided_password = password::wrap_password_arg(password)?;
            let state = CliAppState::new(data_dir, enable_tor)?;
            let (wallet_info, unlocked) = load_wallet(&state, wallet.as_deref())?;
            let unlock_password =
                unlock_if_needed(&state, wallet_info.id, unlocked, provided_password.as_ref())?;

            // Re-authenticate for spending
            let prompted_reauth_password =
                if provided_password.is_some() || unlock_password.is_some() {
                    None
                } else {
                    Some(password::get_password(None, "Password: ")?)
                };
            let reauth_token = {
                let reauth_password = if let Some(p) = provided_password.as_ref() {
                    p
                } else if let Some(p) = unlock_password.as_ref() {
                    p
                } else {
                    prompted_reauth_password.as_ref().expect("prompted above")
                };

                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let (token, _expires) =
                    wm.reauth_wallet(wallet_info.id, reauth_password, ReauthPurpose::Spend)?;
                token
            };
            drop(prompted_reauth_password);
            drop(unlock_password);
            drop(provided_password);

            // Retry broadcast
            let result_txid = {
                let task = {
                    let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                    let tx_svc = state.tx_service.lock().expect("mutex poisoned");
                    let task = wm.prepare_retry_broadcast_task(&txid, &reauth_token, &tx_svc)?;
                    wm.validate_retry_broadcast_task(&task)?;
                    task
                };

                zstash_engine::wallet_manager::WalletManager::execute_prepared_retry_broadcast_task(
                    task, None, None,
                )?
            };

            output.print_tx_sent(&result_txid);
        }
    }

    Ok(())
}

fn load_wallet(
    state: &CliAppState,
    wallet_prefix: Option<&str>,
) -> Result<(zstash_core::domain::WalletInfo, bool)> {
    let wallet_info = if let Some(prefix) = wallet_prefix {
        state.get_wallet_by_prefix(prefix)?
    } else {
        let wallets = state.list_wallets()?;
        if wallets.is_empty() {
            anyhow::bail!("no wallets found - create one with: zstash wallet create --name <NAME>");
        }
        if wallets.len() > 1 {
            anyhow::bail!("multiple wallets found - specify one with --wallet <ID>");
        }
        wallets.into_iter().next().unwrap()
    };

    let (_, unlocked) = state.load_wallet(wallet_info.id)?;
    Ok((wallet_info, unlocked))
}

fn unlock_if_needed(
    state: &CliAppState,
    wallet_id: uuid::Uuid,
    unlocked: bool,
    provided_password: Option<&Zeroizing<String>>,
) -> Result<Option<Zeroizing<String>>> {
    if unlocked {
        return Ok(None);
    }

    match provided_password {
        Some(p) => {
            state.unlock_wallet(wallet_id, p, false)?;
            Ok(None)
        }
        None => {
            let password = password::get_password(None, "Password: ")?;
            state.unlock_wallet(wallet_id, &password, false)?;
            Ok(Some(password))
        }
    }
}
