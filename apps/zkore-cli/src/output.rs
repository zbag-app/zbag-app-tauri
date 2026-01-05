//! Output formatting for text and JSON modes.

use console::style;
use serde::Serialize;

use zkore_core::domain::{Balance, Network, SyncProgress, WalletInfo};

/// Output mode for CLI.
#[derive(Clone)]
pub struct OutputMode {
    json: bool,
}

impl OutputMode {
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    pub fn is_json(&self) -> bool {
        self.json
    }

    /// Print a simple message.
    pub fn print_message(&self, msg: &str) {
        if self.json {
            self.print_json(&serde_json::json!({ "message": msg }));
        } else {
            println!("{}", msg);
        }
    }

    /// Print an error.
    pub fn print_error(&self, err: &anyhow::Error) {
        if self.json {
            self.print_json(&serde_json::json!({
                "error": err.to_string(),
                "chain": err.chain().skip(1).map(|e| e.to_string()).collect::<Vec<_>>()
            }));
        } else {
            eprintln!("{} {}", style("error:").red().bold(), err);
            for cause in err.chain().skip(1) {
                eprintln!("  {} {}", style("caused by:").yellow(), cause);
            }
        }
    }

    /// Print list of wallets.
    pub fn print_wallet_list(&self, wallets: &[WalletInfo]) {
        if self.json {
            self.print_json(&wallets);
        } else {
            if wallets.is_empty() {
                println!("No wallets found.");
                println!();
                println!(
                    "Create one with: {} wallet create --name <NAME>",
                    style("zkore").cyan()
                );
                return;
            }

            println!("{}", style("Wallets:").bold());
            for wallet in wallets {
                let short_id = short_uuid(&wallet.id);
                let network_style = match wallet.network {
                    Network::Mainnet => style("mainnet").green(),
                    Network::Testnet => style("testnet").yellow(),
                };
                println!(
                    "  {} {} [{}]",
                    style(&short_id).cyan(),
                    wallet.name,
                    network_style
                );
            }
        }
    }

    /// Print wallet details.
    pub fn print_wallet_details(&self, wallet: &WalletInfo) {
        if self.json {
            self.print_json(&wallet);
        } else {
            println!("{}", style("Wallet Details:").bold());
            println!("  ID:       {}", wallet.id);
            println!("  Short ID: {}", short_uuid(&wallet.id));
            println!("  Name:     {}", wallet.name);
            println!("  Network:  {:?}", wallet.network);
            println!("  Type:     {:?}", wallet.wallet_type);
            println!("  Remember: {}", wallet.remember_unlock_enabled);
        }
    }

    /// Print wallet created message with seed phrase.
    pub fn print_wallet_created(&self, wallet: &WalletInfo, seed_phrase: &[String]) {
        if self.json {
            self.print_json(&serde_json::json!({
                "wallet": wallet,
                "seed_phrase": seed_phrase
            }));
        } else {
            println!("{}", style("Wallet created successfully!").green().bold());
            println!();
            println!("  ID:   {}", style(short_uuid(&wallet.id)).cyan());
            println!("  Name: {}", wallet.name);
            println!();
            println!(
                "{}",
                style("IMPORTANT: Write down your seed phrase!")
                    .red()
                    .bold()
            );
            println!(
                "{}",
                style("This is the ONLY way to recover your wallet.").red()
            );
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
            println!(
                "Next step: {} sync {}",
                style("zkore").cyan(),
                short_uuid(&wallet.id)
            );
        }
    }

    /// Print wallet restored message.
    pub fn print_wallet_restored(&self, wallet: &WalletInfo, birthday_height: u32) {
        if self.json {
            self.print_json(&serde_json::json!({
                "wallet": wallet,
                "birthday_height": birthday_height
            }));
        } else {
            println!("{}", style("Wallet restored successfully!").green().bold());
            println!();
            println!(
                "  ID:              {}",
                style(short_uuid(&wallet.id)).cyan()
            );
            println!("  Name:            {}", wallet.name);
            println!("  Birthday Height: {}", birthday_height);
            println!();
            println!(
                "Next step: {} sync {}",
                style("zkore").cyan(),
                short_uuid(&wallet.id)
            );
        }
    }

    /// Print balance.
    pub fn print_balance(&self, balance: &Balance, wallet_name: &str) {
        if self.json {
            self.print_json(&balance);
        } else {
            let shielded: u64 = balance.shielded_spendable.parse().unwrap_or(0);
            let pending: u64 = balance.shielded_pending.parse().unwrap_or(0);
            let transparent: u64 = balance.transparent_total.parse().unwrap_or(0);
            let total: u64 = balance.total.parse().unwrap_or(0);

            println!("{} Balance:", style(wallet_name).bold());
            println!("  Shielded:    {} ZEC", format_zec(shielded));
            if pending > 0 {
                println!("  Pending:     {} ZEC", style(format_zec(pending)).dim());
            }
            if transparent > 0 {
                println!(
                    "  Transparent: {} ZEC {}",
                    style(format_zec(transparent)).yellow(),
                    style("(shield recommended)").dim()
                );
            }
            println!(
                "  {}",
                style(format!("Total: {} ZEC", format_zec(total))).bold()
            );
        }
    }

    /// Print sync progress (JSON only - text uses progress bar).
    pub fn print_sync_progress(&self, progress: &SyncProgress) {
        if self.json {
            self.print_json(&progress);
        }
        // For non-JSON, progress is handled by indicatif progress bar
    }

    /// Print sync complete message.
    pub fn print_sync_complete(&self) {
        if self.json {
            self.print_json(&serde_json::json!({
                "status": "complete",
                "message": "Sync complete"
            }));
        } else {
            println!("{}", style("Sync complete!").green().bold());
        }
    }

    /// Print address.
    pub fn print_address(&self, address: &str, address_type: &str) {
        if self.json {
            self.print_json(&serde_json::json!({
                "address": address,
                "address_type": address_type
            }));
        } else {
            println!("{}", style("Receive Address:").bold());
            println!("  Type: {}", address_type);
            println!("  {}", style(address).cyan());
        }
    }

    /// Print transaction sent.
    pub fn print_tx_sent(&self, txid: &str) {
        if self.json {
            self.print_json(&serde_json::json!({ "txid": txid }));
        } else {
            println!("{}", style("Transaction sent!").green().bold());
            println!("  TXID: {}", style(txid).cyan());
        }
    }

    /// Print generic JSON value.
    fn print_json<T: Serialize>(&self, value: &T) {
        println!("{}", serde_json::to_string_pretty(value).unwrap());
    }
}

/// Format zatoshis as ZEC with 8 decimal places.
fn format_zec(zatoshis: u64) -> String {
    let zec = zatoshis as f64 / 100_000_000.0;
    format!("{:.8}", zec)
}

/// Get short UUID (first 8 characters).
pub fn short_uuid(id: &uuid::Uuid) -> String {
    id.to_string()[..8].to_string()
}
