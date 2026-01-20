//! Backup verification command.

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand};
use console::style;
use zeroize::Zeroizing;

use zstash_core::sensitive::SensitiveString;

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
        ///
        /// SECURITY: Avoid passing seed phrases via CLI arguments when possible. They may be
        /// stored in shell history or visible to other processes via process listings. Prefer the
        /// interactive prompts.
        #[arg(long)]
        seed: Option<SensitiveString>,
    },
}

pub async fn run(args: BackupArgs, data_dir: &Path, output: &OutputMode) -> Result<()> {
    match args.command {
        BackupCommand::Verify {
            wallet,
            password,
            seed,
        } => run_verify(&wallet, password, seed, data_dir, output).await,
    }
}

async fn run_verify(
    wallet_prefix: &str,
    password: Option<String>,
    seed: Option<SensitiveString>,
    data_dir: &Path,
    output: &OutputMode,
) -> Result<()> {
    let mut provided_password = password::wrap_password_arg(password)?;
    let state = CliAppState::new(data_dir, false)?;

    // Find and load the wallet
    let wallet_info = state.get_wallet_by_prefix(wallet_prefix)?;
    let (_, unlocked) = state.load_wallet(wallet_info.id)?;

    // Unlock if needed
    if !unlocked {
        let pwd = match provided_password.take() {
            Some(p) => p,
            None => password::get_password(None, "Password: ")?,
        };
        state.unlock_wallet(wallet_info.id, &pwd, false)?;
    }
    // Drop any unused provided password promptly.
    let _ = provided_password.take();

    // If seed phrase provided, do automatic verification
    if let Some(seed_phrase) = seed {
        eprintln!(
            "SECURITY WARNING: Avoid passing seed phrases via CLI arguments (`--seed`) when possible. \
They may be stored in shell history or visible to other processes via process listings."
        );

        // Get challenge (challenge is in-memory only)
        let challenge = {
            let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
            wm.get_backup_challenge(wallet_info.id)?
        };

        // Extract words at the challenge indices (1-indexed)
        let mut word_map: HashMap<u8, SensitiveString> = HashMap::new();
        {
            let words: Vec<&str> = seed_phrase.split_whitespace().collect();
            if words.len() != 24 {
                anyhow::bail!("seed phrase must be 24 words, got {}", words.len());
            }

            for &index in &challenge.indices {
                let word_idx = (index as usize).saturating_sub(1); // Convert to 0-indexed
                if word_idx >= words.len() {
                    anyhow::bail!("invalid index {} for 24-word seed", index);
                }
                word_map.insert(index, words[word_idx].into());
            }
        }
        drop(seed_phrase);

        // Verify
        let result = {
            let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
            wm.verify_backup(wallet_info.id, &challenge.challenge_id, &word_map)
        };

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
        return result;
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

    let mut word_map: HashMap<u8, SensitiveString> = HashMap::new();

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
                "You can try again with: zstash backup verify --wallet {}",
                wallet_prefix
            );
            return Err(e);
        }
    }

    Ok(())
}

/// Prompt user to enter a seed word at a specific index.
fn prompt_word(index: u8) -> Result<SensitiveString> {
    eprint!("  Word #{}: ", index);
    io::stderr().flush()?;

    let mut raw_word = Zeroizing::new(String::new());
    io::stdin().read_line(&mut raw_word)?;

    let mut word = SensitiveString::new(std::mem::take(&mut *raw_word));
    word.trim_in_place();

    if word.is_empty() {
        anyhow::bail!("word cannot be empty");
    }

    Ok(word)
}
