//! Sync command implementation.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use clap::Args;

use zkore_core::domain::SyncPhase;
use zkore_core::ipc::v1::events::SyncProgressEvent;

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
}

pub async fn run(
    args: SyncArgs,
    data_dir: &Path,
    enable_tor: bool,
    output: &OutputMode,
) -> Result<()> {
    let state = CliAppState::new(data_dir, enable_tor)?;

    let wallet_info = state.get_wallet_by_prefix(&args.wallet)?;

    // Load and unlock wallet if needed
    let (info, unlocked) = state.load_wallet(wallet_info.id)?;
    if !unlocked {
        let password = password::get_password(args.password.as_deref(), "Password: ")?;
        state.unlock_wallet(wallet_info.id, &password, false)?;
    }

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
    let progress_bar = if output.is_json() {
        None
    } else {
        Some(progress::create_sync_progress_bar())
    };

    let pb_clone = progress_bar.clone();
    let output_clone = output.clone();
    let on_progress: Option<Arc<dyn Fn(SyncProgressEvent) + Send + Sync>> =
        Some(Arc::new(move |event| {
            if output_clone.is_json() {
                output_clone.print_sync_progress(&event.progress);
            } else if let Some(ref pb) = pb_clone {
                progress::update_sync_progress(pb, &event.progress);
            }
        }));

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

    if let Some(pb) = progress_bar {
        pb.finish_and_clear();
    }

    output.print_sync_complete();
    Ok(())
}
