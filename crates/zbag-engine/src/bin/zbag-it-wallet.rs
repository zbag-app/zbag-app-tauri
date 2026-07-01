use std::path::{Path, PathBuf};

use anyhow::Context as _;
use bip39::Mnemonic;
use rand::RngCore as _;
use zeroize::Zeroize as _;

use zbag_core::permissions::create_dir_all_secure;
use zcash_client_backend::address::Address;
use zcash_client_backend::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_protocol::consensus::Network as ConsensusNetwork;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut force_new = false;
    let mut print_mnemonic = false;
    let mut account_id: u32 = 0;
    let mut mnemonic_file: Option<PathBuf> = None;
    let mut network = ConsensusNetwork::TestNetwork;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--force-new" => force_new = true,
            "--print-mnemonic" => print_mnemonic = true,
            "--account" => {
                let value = args.get(i + 1).context("--account requires a value")?;
                account_id = value
                    .parse()
                    .with_context(|| format!("invalid --account value: {value}"))?;
                i += 1;
            }
            "--mnemonic-file" => {
                let value = args
                    .get(i + 1)
                    .context("--mnemonic-file requires a value")?;
                mnemonic_file = Some(PathBuf::from(value));
                i += 1;
            }
            "--network" => {
                let value = args.get(i + 1).context("--network requires a value")?;
                network = match value.as_str() {
                    "testnet" => ConsensusNetwork::TestNetwork,
                    "mainnet" => ConsensusNetwork::MainNetwork,
                    other => {
                        anyhow::bail!("invalid --network value: {other} (expected testnet|mainnet)")
                    }
                };
                i += 1;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => anyhow::bail!("unknown arg: {other} (use --help)"),
        }
        i += 1;
    }

    let mnemonic_path = match mnemonic_file {
        Some(path) => path,
        None => default_mnemonic_path()?,
    };

    let (mut phrase, source) = load_or_create_mnemonic(&mnemonic_path, force_new)?;
    let mnemonic = parse_mnemonic(&phrase)?;

    let mut seed_bytes = mnemonic.to_seed_normalized("");
    let account = zip32::AccountId::try_from(account_id)
        .map_err(|_| anyhow::anyhow!("invalid account id: {account_id}"))?;
    let usk =
        UnifiedSpendingKey::from_seed(&network, &seed_bytes, account).context("derive usk")?;
    seed_bytes.zeroize();

    let ufvk = usk.to_unified_full_viewing_key();
    let (ua, _) = ufvk
        .default_address(UnifiedAddressRequest::SHIELDED)
        .context("derive unified address")?;
    let address = Address::Unified(ua).encode(&network);

    println!("{address}");
    std::io::Write::flush(&mut std::io::stdout()).ok();

    if print_mnemonic {
        println!();
        println!("mnemonic ({source}):");
        println!("{phrase}");
    } else {
        eprintln!("mnemonic source: {source}");
        eprintln!("mnemonic file: {}", mnemonic_path.display());
        eprintln!("(use --print-mnemonic to print it)");
    }

    phrase.zeroize();

    Ok(())
}

fn print_help() {
    eprintln!(
        "\
zbag-it-wallet: create/load a local integration-test wallet and print its unified address.

USAGE:
  cargo run -p zbag-engine --bin zbag-it-wallet -- [OPTIONS]

OPTIONS:
  --network testnet|mainnet    (default: testnet)
  --account N                 (default: 0)
  --mnemonic-file PATH        (default: $HOME/.zbag/dev/it-wallet.mnemonic)
  --force-new                 overwrite mnemonic file
  --print-mnemonic            print mnemonic after address
  -h, --help                  show this help

ENV:
  ZBAG_IT_MNEMONIC           if set, used instead of --mnemonic-file
"
    );
}

fn default_mnemonic_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".zbag/dev/it-wallet.mnemonic"))
}

fn load_or_create_mnemonic(path: &Path, force_new: bool) -> anyhow::Result<(String, &'static str)> {
    if let Ok(env_phrase) = std::env::var("ZBAG_IT_MNEMONIC") {
        return Ok((env_phrase, "env:ZBAG_IT_MNEMONIC"));
    }

    if !force_new {
        match std::fs::read_to_string(path) {
            Ok(contents) => return Ok((contents.trim().to_string(), "file")),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| format!("failed to read {}", path.display()));
            }
        }
    }

    let mut entropy = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy).context("generate mnemonic")?;
    entropy.zeroize();

    let mut phrase = mnemonic.to_string();
    if let Err(err) = write_secret_file(path, phrase.as_bytes()) {
        phrase.zeroize();
        return Err(err);
    }

    Ok((phrase, "generated"))
}

fn write_secret_file(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all_secure(parent).with_context(|| {
            format!("failed to create mnemonic directory: {}", parent.display())
        })?;
    }

    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }

    let mut file = options
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    std::io::Write::write_all(&mut file, contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    std::io::Write::write_all(&mut file, b"\n")
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn parse_mnemonic(phrase: &str) -> anyhow::Result<Mnemonic> {
    let trimmed = phrase.trim();
    let mnemonic =
        Mnemonic::parse_in_normalized(bip39::Language::English, trimmed).map_err(|_e| {
            anyhow::anyhow!(
                "invalid mnemonic (expected 24 English words); set ZBAG_IT_MNEMONIC or delete mnemonic file"
            )
        })?;
    if mnemonic.words().count() != 24 {
        anyhow::bail!(
            "invalid mnemonic length: expected 24 words, got {}",
            mnemonic.words().count()
        );
    }
    Ok(mnemonic)
}
