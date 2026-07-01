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
    let BalanceArgs {
        account_id,
        wallet,
        password,
    } = args;
    let mut provided_password = password::wrap_password_arg(password)?;
    let state = CliAppState::new(data_dir, false)?;

    // If wallet specified, load it
    let wallet_info = if let Some(wallet_prefix) = wallet.as_deref() {
        state.get_wallet_by_prefix(wallet_prefix)?
    } else {
        let wallets = state.list_wallets()?;
        if wallets.is_empty() {
            anyhow::bail!("no wallets found - create one with: zbag wallet create --name <NAME>");
        }
        if wallets.len() > 1 {
            anyhow::bail!(
                "multiple wallets found - specify one with: zbag balance {} --wallet <ID>",
                account_id
            );
        }
        wallets.into_iter().next().unwrap()
    };

    let (_, unlocked) = state.load_wallet(wallet_info.id)?;
    if !unlocked {
        let password = match provided_password.take() {
            Some(p) => p,
            None => password::get_password(None, "Password: ")?,
        };
        state.unlock_wallet(wallet_info.id, &password, false)?;
    }
    // Drop any unused provided password promptly.
    let _ = provided_password.take();

    let balance = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        wm.get_balance(account_id)?
    };

    output.print_balance(&balance, &wallet_info.name);

    Ok(())
}
