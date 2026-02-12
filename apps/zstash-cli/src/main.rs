#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use zstash_core::permissions::create_dir_all_secure;

mod cli_app_state;
mod commands;
mod file_key_store;
mod output;
mod password;
mod progress;

use output::OutputMode;

#[derive(Parser)]
#[command(name = "zstash")]
#[command(author, version, about = "zSTASH Zcash wallet CLI")]
#[command(propagate_version = true)]
struct Cli {
    /// Output in JSON format (for scripting/agent use)
    #[arg(long, global = true)]
    json: bool,

    /// Enable Tor for network connections
    #[arg(long, global = true)]
    tor: bool,

    /// Custom data directory (default: ~/.zstash)
    #[arg(long, global = true, env = "ZSTASH_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Verbose output (repeat for more: -v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Wallet management (create, restore, list, unlock, lock)
    Wallet(commands::wallet::WalletArgs),

    /// Sync blockchain data
    Sync(commands::sync::SyncArgs),

    /// Transaction operations (send, shield, list)
    #[command(name = "tx")]
    Transaction(commands::tx::TxArgs),

    /// View account balance
    Balance(commands::balance::BalanceArgs),

    /// Generate receive addresses
    Address(commands::address::AddressArgs),

    /// Lightwalletd server management
    Server(commands::server::ServerArgs),

    /// View seed phrase (requires password)
    Seed(commands::seed::SeedArgs),

    /// Backup verification
    Backup(commands::backup::BackupArgs),
}

fn main() {
    if let Err(e) = run() {
        let output = OutputMode::new(std::env::args().any(|a| a == "--json"));
        output.print_error(&e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    init_logging(cli.verbose);

    // Create async runtime
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async { run_command(cli).await })
}

async fn run_command(cli: Cli) -> Result<()> {
    let output = OutputMode::new(cli.json);
    let data_dir = cli.data_dir.unwrap_or_else(default_data_dir);

    // Ensure data directory exists
    create_dir_all_secure(&data_dir)?;

    match cli.command {
        Commands::Wallet(args) => commands::wallet::run(args, &data_dir, cli.tor, &output).await,
        Commands::Sync(args) => commands::sync::run(args, &data_dir, cli.tor, &output).await,
        Commands::Transaction(args) => commands::tx::run(args, &data_dir, cli.tor, &output).await,
        Commands::Balance(args) => commands::balance::run(args, &data_dir, &output).await,
        Commands::Address(args) => commands::address::run(args, &data_dir, &output).await,
        Commands::Server(args) => commands::server::run(args, &data_dir, &output).await,
        Commands::Seed(args) => commands::seed::run(args, &data_dir, &output).await,
        Commands::Backup(args) => commands::backup::run(args, &data_dir, &output).await,
    }
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .expect("HOME directory not found")
        .join(".zstash")
}

fn init_logging(verbosity: u8) {
    use tracing_subscriber::EnvFilter;

    let filter = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}
