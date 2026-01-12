//! Seed phrase viewing command.

use std::path::Path;

use anyhow::Result;
use clap::Args;
use console::style;

use zstash_core::ipc::v1::commands::wallet::ReauthPurpose;

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
    let state = CliAppState::new(data_dir, false)?;

    // If wallet specified, load it; otherwise find the single wallet or error
    let wallet_info = if let Some(wallet_prefix) = &args.wallet {
        let info = state.get_wallet_by_prefix(wallet_prefix)?;
        let (_, unlocked) = state.load_wallet(info.id)?;
        if !unlocked {
            let password = password::get_password(args.password.as_deref(), "Password: ")?;
            state.unlock_wallet(info.id, &password, false)?;
        }
        info
    } else {
        // Try to find a wallet
        let wallets = state.list_wallets()?;
        if wallets.is_empty() {
            anyhow::bail!("no wallets found - create one with: zstash wallet create --name <NAME>");
        }
        if wallets.len() > 1 {
            anyhow::bail!("multiple wallets found - specify one with: zstash seed --wallet <ID>");
        }
        let info = wallets.into_iter().next().unwrap();
        let (_, unlocked) = state.load_wallet(info.id)?;
        if !unlocked {
            let password = password::get_password(args.password.as_deref(), "Password: ")?;
            state.unlock_wallet(info.id, &password, false)?;
        }
        info
    };

    // Re-authenticate to view seed phrase
    // The reauth flow requires password even if wallet is already unlocked
    let reauth_password = password::get_password(
        args.password.as_deref(),
        "Re-enter password to view seed phrase: ",
    )?;

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

    print_seed_phrase(output, &wallet_info.name, &seed_phrase);

    Ok(())
}

fn print_seed_phrase(output: &OutputMode, wallet_name: &str, seed_phrase: &[String]) {
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
            print!("{:>2}. {:<12}", i + 1, word);
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
