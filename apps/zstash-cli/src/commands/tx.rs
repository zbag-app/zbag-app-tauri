//! Transaction commands.

use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand};

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
            let state = CliAppState::new(data_dir, enable_tor)?;

            // Load and unlock wallet
            let wallet_info =
                load_and_unlock_wallet(&state, wallet.as_deref(), password.as_deref())?;

            // Prepare the transaction
            let prepare_response = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                wm.prepare_send(account_id, &to, &amount, memo.as_deref(), allow_transparent)?
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
            let password = password::get_password(password.as_deref(), "Password: ")?;
            let reauth_token = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let (token, _expires) =
                    wm.reauth_wallet(wallet_info.id, &password, ReauthPurpose::Spend)?;
                token
            };

            // Confirm and broadcast
            let confirm_response = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                wm.confirm_send(&prepare_response.proposal_id, &reauth_token, None)?
            };

            output.print_tx_sent(&confirm_response.txid);
        }

        TxCommand::Shield {
            account_id,
            consolidate,
            wallet,
            password,
        } => {
            let state = CliAppState::new(data_dir, enable_tor)?;

            // Load and unlock wallet
            let wallet_info =
                load_and_unlock_wallet(&state, wallet.as_deref(), password.as_deref())?;

            // Re-authenticate for spending
            let password = password::get_password(password.as_deref(), "Password: ")?;
            let reauth_token = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let (token, _expires) =
                    wm.reauth_wallet(wallet_info.id, &password, ReauthPurpose::Spend)?;
                token
            };

            // Shield funds
            let response = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                wm.shield_funds(account_id, consolidate, &reauth_token, None)?
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
            let state = CliAppState::new(data_dir, false)?;

            // Load and unlock wallet
            load_and_unlock_wallet(&state, wallet.as_deref(), password.as_deref())?;

            // List transactions
            let response = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                wm.list_transactions(account_id, limit, offset)?
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
            let state = CliAppState::new(data_dir, enable_tor)?;

            // Load and unlock wallet
            let wallet_info =
                load_and_unlock_wallet(&state, wallet.as_deref(), password.as_deref())?;

            // Re-authenticate for spending
            let password = password::get_password(password.as_deref(), "Password: ")?;
            let reauth_token = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                let (token, _expires) =
                    wm.reauth_wallet(wallet_info.id, &password, ReauthPurpose::Spend)?;
                token
            };

            // Retry broadcast
            let result_txid = {
                let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
                wm.retry_broadcast(&txid, &reauth_token, None)?
            };

            output.print_tx_sent(&result_txid);
        }
    }

    Ok(())
}

fn load_and_unlock_wallet(
    state: &CliAppState,
    wallet_prefix: Option<&str>,
    password: Option<&str>,
) -> Result<zstash_core::domain::WalletInfo> {
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
    if !unlocked {
        let password = password::get_password(password, "Password: ")?;
        state.unlock_wallet(wallet_info.id, &password, false)?;
    }

    Ok(wallet_info)
}
