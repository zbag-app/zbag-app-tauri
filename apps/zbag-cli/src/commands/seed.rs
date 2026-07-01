//! Seed phrase viewing command.

use std::path::Path;

use anyhow::Result;
use clap::Args;
use console::style;

use zbag_core::ipc::v1::commands::wallet::ReauthPurpose;
use zbag_core::sensitive::SensitiveString;

use crate::cli_app_state::CliAppState;
use crate::output::OutputMode;
use crate::password;

#[derive(Args)]
pub struct SeedArgs {
    /// Wallet ID or prefix
    #[arg(short, long)]
    wallet: Option<String>,

    /// Password (will prompt if not provided)
    #[arg(short, long)]
    password: Option<String>,
}

pub async fn run(args: SeedArgs, data_dir: &Path, output: &OutputMode) -> Result<()> {
    let SeedArgs { wallet, password } = args;
    let mut provided_password = password::wrap_password_arg(password)?;
    let state = CliAppState::new(data_dir, false)?;

    // If wallet specified, load it; otherwise find the single wallet or error
    let wallet_info = if let Some(wallet_prefix) = wallet.as_deref() {
        state.get_wallet_by_prefix(wallet_prefix)?
    } else {
        let wallets = state.list_wallets()?;
        if wallets.is_empty() {
            anyhow::bail!("no wallets found - create one with: zbag wallet create --name <NAME>");
        }
        if wallets.len() > 1 {
            anyhow::bail!("multiple wallets found - specify one with: zbag seed --wallet <ID>");
        }
        wallets.into_iter().next().unwrap()
    };

    let (_, unlocked) = state.load_wallet(wallet_info.id)?;
    // Re-authenticate to view seed phrase (the reauth flow requires a password even if the wallet
    // is already unlocked). Prefer consuming a provided password via `take()` to minimize the
    // sensitive value's lifetime.
    let reauth_password = if !unlocked {
        let pwd = match provided_password.take() {
            Some(p) => p,
            None => password::get_password(None, "Password: ")?,
        };
        state.unlock_wallet(wallet_info.id, &pwd, false)?;
        pwd
    } else if let Some(p) = provided_password.take() {
        p
    } else {
        password::get_password(None, "Re-enter password to view seed phrase: ")?
    };

    let seed_phrase = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");

        // Get reauth token
        let (token, _expires_at) = wm.reauth_wallet(
            wallet_info.id,
            &reauth_password,
            ReauthPurpose::ViewSeedPhrase,
        )?;

        // View seed phrase using the token
        wm.view_seed_phrase(wallet_info.id, &token)?
    };

    drop(reauth_password);

    print_seed_phrase(output, &wallet_info.name, &seed_phrase);

    Ok(())
}

fn print_seed_phrase(output: &OutputMode, wallet_name: &str, seed_phrase: &[SensitiveString]) {
    if output.is_json() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "wallet_name": wallet_name,
                "seed_phrase": seed_phrase
            }))
            .unwrap()
        );
    } else {
        println!();
        println!(
            "{}",
            style("WARNING: Keep your seed phrase secret!").red().bold()
        );
        println!(
            "{}",
            style("Anyone with these words can access your funds.").red()
        );
        println!();
        println!("Seed phrase for wallet '{}':", style(wallet_name).cyan());
        println!();

        for (i, word) in seed_phrase.iter().enumerate() {
            print!("{:>2}. {:<12}", i + 1, word.as_ref());
            if (i + 1) % 4 == 0 {
                println!();
            }
        }
        if !seed_phrase.len().is_multiple_of(4) {
            println!();
        }
        println!();
    }
}
