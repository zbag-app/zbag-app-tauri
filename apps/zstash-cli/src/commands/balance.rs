//! Balance query command.

use std::path::Path;

use anyhow::Result;
use clap::Args;

use crate::cli_app_state::CliAppState;
use crate::output::OutputMode;
use crate::password;

#[derive(Args)]
pub struct BalanceArgs {
    /// Account ID (usually 0 for the default account)
    account_id: u32,

    /// Wallet ID or prefix (if not the currently loaded wallet)
    #[arg(short, long)]
    wallet: Option<String>,

    /// Password (will prompt if wallet is locked)
    #[arg(short, long)]
    password: Option<String>,
}

pub async fn run(args: BalanceArgs, data_dir: &Path, output: &OutputMode) -> Result<()> {
    let state = CliAppState::new(data_dir, false)?;

    // If wallet specified, load it
    let wallet_info = if let Some(wallet_prefix) = &args.wallet {
        let info = state.get_wallet_by_prefix(wallet_prefix)?;
        let (_, unlocked) = state.load_wallet(info.id)?;
        if !unlocked {
            let password = password::get_password(args.password.as_deref(), "Password: ")?;
            state.unlock_wallet(info.id, &password, false)?;
        }
        info
    } else {
        // Try to find an active wallet
        let wallets = state.list_wallets()?;
        if wallets.is_empty() {
            anyhow::bail!("no wallets found - create one with: zkore wallet create --name <NAME>");
        }
        if wallets.len() > 1 {
            anyhow::bail!(
                "multiple wallets found - specify one with: zkore balance {} --wallet <ID>",
                args.account_id
            );
        }
        let info = wallets.into_iter().next().unwrap();
        let (_, unlocked) = state.load_wallet(info.id)?;
        if !unlocked {
            let password = password::get_password(args.password.as_deref(), "Password: ")?;
            state.unlock_wallet(info.id, &password, false)?;
        }
        info
    };

    let balance = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        wm.get_balance(args.account_id)?
    };

    output.print_balance(&balance, &wallet_info.name);

    Ok(())
}
