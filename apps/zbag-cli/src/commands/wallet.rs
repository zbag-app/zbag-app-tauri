//! Wallet management commands.

use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};

use zbag_core::domain::Network;
use zbag_core::sensitive::SensitiveString;
use zbag_engine::wallet_manager::fetch_birthday_height_for_new_wallet;

use crate::cli_app_state::CliAppState;
use crate::output::OutputMode;
use crate::password;

#[derive(Args)]
pub struct WalletArgs {
    #[command(subcommand)]
    command: WalletCommand,
}

#[derive(Subcommand)]
enum WalletCommand {
    /// Create a new wallet
    Create {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// Network (mainnet or testnet)
        #[arg(short = 'N', long, default_value = "mainnet")]
        network: NetworkArg,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Remember unlock (store key for auto-unlock)
        #[arg(long)]
        remember: bool,
    },

    /// Restore wallet from seed phrase
    Restore {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// Network (mainnet or testnet)
        #[arg(short = 'N', long, default_value = "mainnet")]
        network: NetworkArg,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Seed phrase (24 words, will prompt if not provided)
        ///
        /// SECURITY: Avoid passing seed phrases via CLI arguments when possible. They may be
        /// stored in shell history or visible to other processes via process listings. Prefer the
        /// interactive prompts.
        #[arg(long)]
        seed: Option<SensitiveString>,

        /// Birthday date (YYYY-MM-DD) for faster sync
        #[arg(long)]
        birthday: Option<String>,

        /// Remember unlock
        #[arg(long)]
        remember: bool,
    },

    /// List all wallets
    List,

    /// Show wallet details
    Show {
        /// Wallet ID or prefix
        wallet: String,
    },

    /// Unlock a wallet
    Unlock {
        /// Wallet ID or prefix
        wallet: String,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Remember unlock
        #[arg(long)]
        remember: bool,
    },

    /// Lock a wallet
    Lock {
        /// Wallet ID or prefix
        wallet: String,
    },
}

#[derive(Clone, ValueEnum)]
pub enum NetworkArg {
    Mainnet,
    Testnet,
}

impl From<NetworkArg> for Network {
    fn from(arg: NetworkArg) -> Self {
        match arg {
            NetworkArg::Mainnet => Network::Mainnet,
            NetworkArg::Testnet => Network::Testnet,
        }
    }
}

pub async fn run(
    args: WalletArgs,
    data_dir: &Path,
    enable_tor: bool,
    output: &OutputMode,
) -> Result<()> {
    match args.command {
        WalletCommand::Create {
            name,
            network,
            password,
            remember,
        } => {
            let mut provided_password = password::wrap_password_arg(password)?;

            let state = CliAppState::new(data_dir, enable_tor)?;
            let network: Network = network.into();

            // Fetch birthday height from chain tip for new wallet
            let grpc_url = resolve_grpc_url(&state, network)?;
            let birthday =
                fetch_birthday_height_for_new_wallet(&grpc_url, state.tor_manager.clone()).await;

            let password = match provided_password.take() {
                Some(p) => p,
                None => password::get_password_with_confirm(None)?,
            };

            let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
            let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
            let result =
                wm.create_wallet(&name, network, &password, remember, birthday, &mut tx_svc)?;
            drop(password);

            output.print_wallet_created(&result.wallet, &result.seed_phrase);
        }

        WalletCommand::Restore {
            name,
            network,
            password,
            seed,
            birthday,
            remember,
        } => {
            let mut provided_password = password::wrap_password_arg(password)?;

            let state = CliAppState::new(data_dir, enable_tor)?;
            let password = match provided_password.take() {
                Some(p) => p,
                None => password::get_password_with_confirm(None)?,
            };
            if seed.is_some() {
                eprintln!(
                    "SECURITY WARNING: Avoid passing seed phrases via CLI arguments (`--seed`) when possible. \
They may be stored in shell history or visible to other processes via process listings."
                );
            }
            let seed_phrase = password::get_seed_phrase(seed)?;
            let network: Network = network.into();

            let birthday_ms = birthday.map(|b| parse_birthday_date(&b)).transpose()?;

            let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
            let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
            let result = wm.restore_wallet(
                &name,
                network,
                &password,
                remember,
                seed_phrase,
                birthday_ms,
                &mut tx_svc,
            )?;

            output.print_wallet_restored(&result.wallet, result.birthday_height);
        }

        WalletCommand::List => {
            let state = CliAppState::new(data_dir, false)?;
            let wallets = state.list_wallets()?;
            output.print_wallet_list(&wallets);
        }

        WalletCommand::Show { wallet } => {
            let state = CliAppState::new(data_dir, false)?;
            let wallet_info = state.get_wallet_by_prefix(&wallet)?;
            output.print_wallet_details(&wallet_info);
        }

        WalletCommand::Unlock {
            wallet,
            password,
            remember,
        } => {
            let mut provided_password = password::wrap_password_arg(password)?;

            let state = CliAppState::new(data_dir, false)?;
            let wallet_info = state.get_wallet_by_prefix(&wallet)?;

            // Load wallet first
            let (_, already_unlocked) = state.load_wallet(wallet_info.id)?;
            if already_unlocked {
                output.print_message(&format!(
                    "Wallet '{}' is already unlocked",
                    wallet_info.name
                ));
                return Ok(());
            }

            let password = match provided_password.take() {
                Some(p) => p,
                None => password::get_password(None, "Password: ")?,
            };
            state.unlock_wallet(wallet_info.id, &password, remember)?;
            drop(password);

            output.print_message(&format!("Wallet '{}' unlocked", wallet_info.name));
        }

        WalletCommand::Lock { wallet } => {
            let state = CliAppState::new(data_dir, false)?;
            let wallet_info = state.get_wallet_by_prefix(&wallet)?;

            state.lock_wallet(wallet_info.id)?;
            output.print_message(&format!("Wallet '{}' locked", wallet_info.name));
        }
    }

    Ok(())
}

fn resolve_grpc_url(state: &CliAppState, network: Network) -> Result<String> {
    let wm = state.wallet_manager.lock().expect("mutex poisoned");
    zbag_engine::server_resolver::resolve_grpc_url(wm.app_db(), network)
}

fn parse_birthday_date(date_str: &str) -> Result<i64> {
    let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("invalid date format, expected YYYY-MM-DD"))?;
    let datetime = date.and_hms_opt(0, 0, 0).unwrap();
    Ok(datetime.and_utc().timestamp_millis())
}
