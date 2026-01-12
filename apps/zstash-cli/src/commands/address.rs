//! Address generation command.

use std::path::Path;

use anyhow::Result;
use clap::{Args, ValueEnum};

use zstash_core::domain::AddressType;

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
    let state = CliAppState::new(data_dir, false)?;

    // If wallet specified, load it
    let _wallet_info = if let Some(wallet_prefix) = &args.wallet {
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
            anyhow::bail!("no wallets found - create one with: zstash wallet create --name <NAME>");
        }
        if wallets.len() > 1 {
            anyhow::bail!(
                "multiple wallets found - specify one with: zstash address {} --wallet <ID>",
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

    let address_type: AddressType = args.address_type.into();
    let address_info = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        wm.get_receive_address(args.account_id, address_type)?
    };

    let type_str = match address_info.address_type {
        AddressType::ShieldedOnly => "Shielded",
        AddressType::Transparent => "Transparent",
    };
    output.print_address(&address_info.encoded, type_str);

    Ok(())
}
