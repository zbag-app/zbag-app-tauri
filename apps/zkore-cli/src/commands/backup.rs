//! Backup verification command.

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand};
use console::style;

use crate::cli_app_state::CliAppState;
use crate::output::OutputMode;
use crate::password;

#[derive(Args)]
pub struct BackupArgs {
    #[command(subcommand)]
    command: BackupCommand,
}

#[derive(Subcommand)]
enum BackupCommand {
    /// Verify backup by providing seed words at specific indices
    Verify {
        /// Wallet ID or prefix
        #[arg(short, long)]
        wallet: String,

        /// Password (will prompt if wallet is locked)
        #[arg(short, long)]
        password: Option<String>,

        /// Full seed phrase (24 words, space-separated) for automatic verification
        /// The CLI will extract the words needed for the challenge
        #[arg(long)]
        seed: Option<String>,
    },
}

pub async fn run(args: BackupArgs, data_dir: &Path, output: &OutputMode) -> Result<()> {
    match args.command {
        BackupCommand::Verify {
            wallet,
            password,
            seed,
        } => {
            run_verify(
                &wallet,
                password.as_deref(),
                seed.as_deref(),
                data_dir,
                output,
            )
            .await
        }
    }
}

async fn run_verify(
    wallet_prefix: &str,
    password: Option<&str>,
    seed: Option<&str>,
    data_dir: &Path,
    output: &OutputMode,
) -> Result<()> {
    let state = CliAppState::new(data_dir, false)?;

    // Find and load the wallet
    let wallet_info = state.get_wallet_by_prefix(wallet_prefix)?;
    let (_, unlocked) = state.load_wallet(wallet_info.id)?;

    // Unlock if needed
    if !unlocked {
        let pwd = password::get_password(password, "Password: ")?;
        state.unlock_wallet(wallet_info.id, &pwd, false)?;
    }

    // If seed phrase provided, do automatic verification
    if let Some(seed_phrase) = seed {
        let words: Vec<&str> = seed_phrase.split_whitespace().collect();
        if words.len() != 24 {
            anyhow::bail!("seed phrase must be 24 words, got {}", words.len());
        }

        // Get challenge and immediately verify (challenge is in-memory only)
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        let challenge = wm.get_backup_challenge(wallet_info.id)?;

        // Extract words at the challenge indices (1-indexed)
        let mut word_map: HashMap<u8, String> = HashMap::new();
        for &index in &challenge.indices {
            let word_idx = (index as usize).saturating_sub(1); // Convert to 0-indexed
            if word_idx >= words.len() {
                anyhow::bail!("invalid index {} for 24-word seed", index);
            }
            word_map.insert(index, words[word_idx].to_lowercase());
        }

        // Verify
        let result = wm.verify_backup(wallet_info.id, &challenge.challenge_id, &word_map);

        if output.is_json() {
            match &result {
                Ok(()) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "success": true,
                            "message": "Backup verified successfully"
                        }))
                        .unwrap()
                    );
                }
                Err(e) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "success": false,
                            "error": e.to_string()
                        }))
                        .unwrap()
                    );
                }
            }
        } else {
            match &result {
                Ok(()) => {
                    println!("{}", style("Backup verified successfully!").green().bold());
                }
                Err(e) => {
                    println!(
                        "{} {}",
                        style("Backup verification failed:").red().bold(),
                        e
                    );
                }
            }
        }
        return result.map_err(Into::into);
    }

    // Get backup challenge
    let challenge = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        wm.get_backup_challenge(wallet_info.id)?
    };

    if output.is_json() {
        // In JSON mode, output the challenge for scripted verification
        let json = serde_json::json!({
            "challenge_id": challenge.challenge_id,
            "indices": challenge.indices,
            "expires_at": challenge.expires_at,
            "message": "Provide words at the specified indices to verify backup"
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
        return Ok(());
    }

    // Interactive mode - prompt user for words
    println!();
    println!(
        "{} Backup Verification for '{}'",
        style("*").cyan(),
        wallet_info.name
    );
    println!();
    println!("Please enter the seed words at the following positions:");
    println!("(This verifies you have correctly backed up your seed phrase)");
    println!();

    let mut word_map: HashMap<u8, String> = HashMap::new();

    for &index in &challenge.indices {
        let word = prompt_word(index)?;
        word_map.insert(index, word);
    }

    // Verify the backup
    let result = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        wm.verify_backup(wallet_info.id, &challenge.challenge_id, &word_map)
    };

    match result {
        Ok(()) => {
            println!();
            println!("{}", style("Backup verified successfully!").green().bold());
            println!("Your seed phrase backup has been confirmed.");
        }
        Err(e) => {
            println!();
            println!(
                "{} {}",
                style("Backup verification failed:").red().bold(),
                e
            );
            println!();
            println!("Please check that you entered the correct words.");
            println!(
                "You can try again with: zkore backup verify --wallet {}",
                wallet_prefix
            );
            return Err(e);
        }
    }

    Ok(())
}

/// Prompt user to enter a seed word at a specific index.
fn prompt_word(index: u8) -> Result<String> {
    eprint!("  Word #{}: ", index);
    io::stderr().flush()?;

    let mut word = String::new();
    io::stdin().read_line(&mut word)?;

    let word = word.trim().to_lowercase();
    if word.is_empty() {
        anyhow::bail!("word cannot be empty");
    }

    Ok(word)
}
