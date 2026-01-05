//! Password and seed phrase input handling.

use std::io::{self, Write};

use anyhow::{Context as _, Result};
use zeroize::Zeroizing;

/// Get password, either from provided value or by prompting.
pub fn get_password(provided: Option<&str>, prompt: &str) -> Result<String> {
    match provided {
        Some(p) => Ok(p.to_string()),
        None => {
            eprint!("{}", prompt);
            io::stderr().flush()?;
            let password = rpassword::read_password().context("failed to read password")?;
            Ok(password)
        }
    }
}

/// Get password with confirmation (for wallet creation).
pub fn get_password_with_confirm(provided: Option<&str>) -> Result<String> {
    match provided {
        Some(p) => Ok(p.to_string()),
        None => {
            eprint!("Password: ");
            io::stderr().flush()?;
            let password =
                Zeroizing::new(rpassword::read_password().context("failed to read password")?);

            if password.is_empty() {
                anyhow::bail!("password cannot be empty");
            }

            eprint!("Confirm password: ");
            io::stderr().flush()?;
            let confirm =
                Zeroizing::new(rpassword::read_password().context("failed to read password")?);

            if *password != *confirm {
                anyhow::bail!("passwords do not match");
            }

            Ok(password.to_string())
        }
    }
}

/// Get seed phrase, either from provided value or by prompting.
pub fn get_seed_phrase(provided: Option<&str>) -> Result<String> {
    match provided {
        Some(s) => {
            validate_seed_phrase(s)?;
            Ok(s.to_string())
        }
        None => {
            eprintln!("Enter your 24-word seed phrase (words separated by spaces):");
            eprint!("> ");
            io::stderr().flush()?;

            let mut phrase = String::new();
            io::stdin().read_line(&mut phrase)?;

            let phrase = phrase.trim().to_string();
            validate_seed_phrase(&phrase)?;

            Ok(phrase)
        }
    }
}

/// Validate seed phrase word count.
fn validate_seed_phrase(phrase: &str) -> Result<()> {
    let word_count = phrase.split_whitespace().count();
    if word_count != 24 {
        anyhow::bail!("seed phrase must be 24 words (got {})", word_count);
    }
    Ok(())
}

/// Prompt for confirmation (y/n).
pub fn confirm_action(message: &str) -> Result<bool> {
    eprint!("{} [y/N]: ", message);
    io::stderr().flush()?;

    let mut response = String::new();
    io::stdin().read_line(&mut response)?;

    Ok(response.trim().to_lowercase() == "y")
}
