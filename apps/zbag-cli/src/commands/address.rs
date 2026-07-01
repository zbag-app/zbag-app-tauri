//! Address generation command.

use std::path::Path;

use anyhow::Result;
use clap::{Args, ValueEnum};

use zbag_core::domain::AddressType;

use crate::cli_app_state::CliAppState;
use crate::output::OutputMode;
use crate::password;

#[derive(Args)]
pub struct AddressArgs {
    /// Account ID (usually 0 for the default account)
    account_id: u32,

    /// Address type
    #[arg(short = 't', long, default_value = "shielded")]
    address_type: AddressTypeArg,

    /// Wallet ID or prefix
    #[arg(short, long)]
    wallet: Option<String>,

    /// Password (will prompt if wallet is locked)
    #[arg(short, long)]
    password: Option<String>,
}

#[derive(Clone, ValueEnum)]
pub enum AddressTypeArg {
    Shielded,
    Transparent,
}

impl From<AddressTypeArg> for AddressType {
    fn from(arg: AddressTypeArg) -> Self {
        match arg {
            AddressTypeArg::Shielded => AddressType::ShieldedOnly,
            AddressTypeArg::Transparent => AddressType::Transparent,
        }
    }
}

pub async fn run(args: AddressArgs, data_dir: &Path, output: &OutputMode) -> Result<()> {
    let AddressArgs {
        account_id,
        address_type,
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
                "multiple wallets found - specify one with: zbag address {} --wallet <ID>",
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

    let address_type: AddressType = address_type.into();
    let address_info = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        wm.get_receive_address(account_id, address_type)?
    };

    let type_str = match address_info.address_type {
        AddressType::ShieldedOnly => "Shielded",
        AddressType::Transparent => "Transparent",
    };
    output.print_address(&address_info.encoded, type_str);

    Ok(())
}
