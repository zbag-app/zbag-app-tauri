//! Sync command implementation.

use std::io::IsTerminal as _;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use anyhow::Result;
use clap::Args;

use zstash_core::domain::SyncPhase;
use zstash_core::ipc::v1::events::SyncProgressEvent;

use crate::cli_app_state::{CliAppState, network_dir_name};
use crate::output::OutputMode;
use crate::password;
use crate::progress;

#[derive(Args)]
pub struct SyncArgs {
    /// Wallet ID or prefix
    wallet: String,

    /// Password (will prompt if wallet is locked)
    #[arg(short, long)]
    password: Option<String>,

    /// Log progress every second (for benchmarking/analysis)
    #[arg(long)]
    progress_log: bool,
}

pub async fn run(
    args: SyncArgs,
    data_dir: &Path,
    enable_tor: bool,
    output: &OutputMode,
) -> Result<()> {
    let SyncArgs {
        wallet,
        password,
        progress_log,
    } = args;
    let mut provided_password = password::wrap_password_arg(password)?;
    let state = CliAppState::new(data_dir, enable_tor)?;

    let wallet_info = state.get_wallet_by_prefix(&wallet)?;

    // Load and unlock wallet if needed
    let (info, unlocked) = state.load_wallet(wallet_info.id)?;
    if !unlocked {
        let password = match provided_password.take() {
            Some(p) => p,
            None => password::get_password(None, "Password: ")?,
        };
        state.unlock_wallet(wallet_info.id, &password, false)?;
    }
    // Drop any unused provided password promptly.
    let _ = provided_password.take();

    // Get wallet DB path and DEK
    let (wallet_db_path, wallet_dek, account_ids) = {
        let mut wm = state.wallet_manager.lock().expect("mutex poisoned");
        let dek = wm.unlocked_wallet_dek(wallet_info.id)?;
        let accounts = wm.list_wallet_db_account_ids(wallet_info.id)?;
        let wallet_dir = wm
            .wallets_root()
            .join(network_dir_name(info.network))
            .join(wallet_info.id.to_string());
        let db_path = wallet_dir.join("wallet.sqlite");
        (db_path, dek, accounts)
    };

    // Create progress handler
    let stderr_is_tty = std::io::stderr().is_terminal();
    let progress_bar = if output.is_json() || !stderr_is_tty {
        None
    } else {
        Some(progress::create_sync_progress_bar())
    };

    // Track start height for progress logging (captured from first progress event)
    let start_height = Arc::new(AtomicU32::new(0));
    let start_height_clone = start_height.clone();

    let pb_clone = progress_bar.clone();
    let output_clone = output.clone();
    let on_progress: Option<Arc<dyn Fn(SyncProgressEvent) + Send + Sync>> =
        Some(Arc::new(move |event| {
            // Capture start height from first meaningful progress event
            if event.progress.scan_frontier_height > 0 {
                start_height_clone
                    .compare_exchange(
                        0,
                        event.progress.scan_frontier_height,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    )
                    .ok();
            }

            if output_clone.is_json() {
                output_clone.print_sync_progress(&event.progress);
            } else if let Some(ref pb) = pb_clone {
                progress::update_sync_progress(pb, &event.progress);
            }
        }));

    // Record sync start time
    let sync_start = Instant::now();

    // Start sync
    {
        let wm = state.wallet_manager.lock().expect("mutex poisoned");
        state.sync_service.start_sync(
            wm.app_db(),
            wallet_info.id,
            info.network,
            wallet_db_path,
            wallet_dek,
            account_ids,
            state.tor_manager.clone(),
            on_progress,
            None, // balance handler
        )?;
    }

    // Spawn progress logger task if enabled
    let logger_handle = if progress_log && !output.is_json() {
        let pb = progress_bar.clone();
        let sync_service = state.sync_service.clone();
        let wallet_id = wallet_info.id;
        let start_height = start_height.clone();

        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                let progress = sync_service.get_progress(wallet_id);

                // Skip if sync hasn't really started yet
                if progress.scan_frontier_height == 0 {
                    continue;
                }

                let elapsed = sync_start.elapsed();
                let start_h = start_height.load(Ordering::SeqCst);
                if start_h == 0 {
                    continue;
                }
                let chain_tip = progress
                    .wallet_tip_height
                    .max(progress.scan_frontier_height);
                let line =
                    progress::format_progress_log_line(elapsed, &progress, start_h, chain_tip);
                if stderr_is_tty {
                    if let Some(ref pb) = pb {
                        pb.println(line);
                    } else {
                        eprintln!("{line}");
                    }
                } else {
                    println!("{line}");
                }
            }
        }))
    } else {
        None
    };

    // Wait for sync to complete (poll progress)
    loop {
        let progress = state.sync_service.get_progress(wallet_info.id);

        if progress.phase == SyncPhase::Idle && progress.progress_percent >= 100 {
            break;
        }

        // Sync may have failed or been cancelled if idle with 0%
        if progress.phase == SyncPhase::Idle && progress.progress_percent == 0 {
            // Check if we ever started
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let progress_after = state.sync_service.get_progress(wallet_info.id);
            if progress_after.phase == SyncPhase::Idle && progress_after.progress_percent == 0 {
                break;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Cancel progress logger
    if let Some(handle) = logger_handle {
        handle.abort();
    }

    if let Some(pb) = progress_bar {
        pb.finish_and_clear();
    }

    output.print_sync_complete();
    Ok(())
}
