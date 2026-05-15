//! Password and seed phrase input handling.

use std::io::{self, Write};

use anyhow::{Context as _, Result};
use zeroize::Zeroizing;

use bagz_core::sensitive::SensitiveString;

/// Wrap a provided password argument in `Zeroizing`, rejecting empty strings.
#[must_use = "the returned password is sensitive; use it and drop it promptly"]
pub fn wrap_password_arg(password: Option<String>) -> Result<Option<Zeroizing<String>>> {
    let password = password.map(Zeroizing::new);
    if let Some(p) = password.as_ref()
        && p.is_empty()
    {
        anyhow::bail!("password cannot be empty");
    }
    Ok(password)
}

/// Get password, either from provided value or by prompting.
#[must_use = "the returned password is sensitive; use it and drop it promptly"]
pub fn get_password(provided: Option<&str>, prompt: &str) -> Result<Zeroizing<String>> {
    match provided {
        Some(p) => {
            if p.is_empty() {
                anyhow::bail!("password cannot be empty");
            }
            Ok(Zeroizing::new(p.to_string()))
        }
        None => {
            eprint!("{}", prompt);
            io::stderr().flush()?;
            Ok(Zeroizing::new(
                rpassword::read_password().context("failed to read password")?,
            ))
        }
    }
}

/// Get password with confirmation (for wallet creation).
#[must_use = "the returned password is sensitive; use it and drop it promptly"]
pub fn get_password_with_confirm(provided: Option<&str>) -> Result<Zeroizing<String>> {
    match provided {
        Some(p) => {
            if p.is_empty() {
                anyhow::bail!("password cannot be empty");
            }
            Ok(Zeroizing::new(p.to_string()))
        }
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
            let confirm_password =
                Zeroizing::new(rpassword::read_password().context("failed to read confirmation")?);

            if *password != *confirm_password {
                anyhow::bail!("passwords do not match");
            }

            Ok(password)
        }
    }
}

/// Get seed phrase, either from provided value or by prompting.
pub fn get_seed_phrase(provided: Option<SensitiveString>) -> Result<SensitiveString> {
    match provided {
        Some(mut phrase) => {
            phrase.trim_in_place();
            validate_seed_phrase(phrase.as_ref())?;
            Ok(phrase)
        }
        None => {
            eprintln!("Enter your 24-word seed phrase (words separated by spaces):");
            eprint!("> ");
            io::stderr().flush()?;

            let mut raw_phrase = Zeroizing::new(String::new());
            io::stdin().read_line(&mut raw_phrase)?;

            // Move the owned buffer into `SensitiveString` (which zeroizes on drop) and then trim
            // in-place to avoid allocating a second copy.
            let mut phrase = SensitiveString::new(std::mem::take(&mut *raw_phrase));
            phrase.trim_in_place();
            validate_seed_phrase(phrase.as_ref())?;
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
