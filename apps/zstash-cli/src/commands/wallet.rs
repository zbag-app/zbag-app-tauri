//! Wallet management commands.

use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};

use zstash_core::domain::Network;
use zstash_engine::wallet_manager::fetch_birthday_height_for_new_wallet;

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
        #[arg(long)]
        seed: Option<String>,

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
            let state = CliAppState::new(data_dir, enable_tor)?;
            let password = password::get_password_with_confirm(password.as_deref())?;
            let network: Network = network.into();

            // Fetch birthday height from chain tip for new wallet
            let grpc_url = resolve_grpc_url(&state, network)?;
            let birthday =
                fetch_birthday_height_for_new_wallet(&grpc_url, state.tor_manager.clone()).await;

            let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
            let result = wm.create_wallet(&name, network, &password, remember, birthday)?;

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
            let state = CliAppState::new(data_dir, enable_tor)?;
            let password = password::get_password_with_confirm(password.as_deref())?;
            let seed_phrase = password::get_seed_phrase(seed.as_deref())?;
            let network: Network = network.into();

            let birthday_ms = birthday.map(|b| parse_birthday_date(&b)).transpose()?;

            let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
            let result = wm.restore_wallet(
                &name,
                network,
                &password,
                remember,
                &seed_phrase,
                birthday_ms,
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

            let password = password::get_password(password.as_deref(), "Password: ")?;
            state.unlock_wallet(wallet_info.id, &password, remember)?;

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
    zstash_engine::server_resolver::resolve_grpc_url(wm.app_db(), network)
}

fn parse_birthday_date(date_str: &str) -> Result<i64> {
    let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("invalid date format, expected YYYY-MM-DD"))?;
    let datetime = date.and_hms_opt(0, 0, 0).unwrap();
    Ok(datetime.and_utc().timestamp_millis())
}
