//! Wallet-related command handlers.

use zstash_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse,
    LockWalletRequest, LockWalletResponse, LogoutWalletRequest, LogoutWalletResponse,
    ReauthWalletRequest, ReauthWalletResponse, UnlockWalletRequest, UnlockWalletResponse,
    ViewSeedPhraseRequest, ViewSeedPhraseResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::{load_accounts_for_wallet, map_anyhow, system_time_to_unix_ms};

pub fn list_wallets_impl(
    state: &AppState,
    request: ListWalletsRequest,
) -> IpcResult<ListWalletsResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let wallets = mgr.list_wallets()?;
        Ok(ListWalletsResponse {
            schema_version: SCHEMA_VERSION,
            wallets,
        })
    })
}

pub fn create_wallet_impl(
    state: &AppState,
    request: CreateWalletRequest,
) -> IpcResult<CreateWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    // WARNING: Test bridge divergence from production behavior
    // =========================================================
    // In production, birthday height is fetched from lightwalletd to optimize
    // initial sync. In test-bridge mode, we skip this to avoid nested runtime
    // issues, using Sapling activation height instead. This means test-created
    // wallets will scan from an earlier block height than production wallets.
    let birthday_height: Option<u32> = None;

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let created = mgr.create_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
            birthday_height,
        )?;

        Ok(CreateWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: created.wallet,
            seed_phrase: created.seed_phrase,
            backup_challenge: created.backup_challenge,
        })
    })
}

pub fn load_wallet_impl(
    state: &AppState,
    request: LoadWalletRequest,
) -> IpcResult<LoadWalletResponse> {
    use zstash_core::domain::{SyncPhase, SyncProgress, WalletLockStatus};
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");

        // Stop sync for the previously-active wallet (best effort)
        if let Some(prev_wallet_id) = mgr.active_wallet_info().map(|w| w.id)
            && prev_wallet_id != request.wallet_id
        {
            mgr.observe_sync_stop_requested(prev_wallet_id);
            let _ = state.sync_service.stop_sync(prev_wallet_id, None);
            mgr.observe_sync_progress(
                prev_wallet_id,
                SyncProgress {
                    phase: SyncPhase::Idle,
                    scan_frontier_height: 0,
                    wallet_tip_height: 0,
                    progress_percent: 0,
                    eta_seconds: None,
                    retry_in_seconds: None,
                    error_message: None,
                },
            );
        }

        let (wallet, lock_status) = mgr.load_wallet(request.wallet_id)?;

        let accounts = if lock_status == WalletLockStatus::Locked {
            vec![]
        } else {
            load_accounts_for_wallet(&mut mgr, wallet.id)?
        };

        // Note: We skip auto-sync in test bridge mode since we don't have an AppHandle
        // for event emission. Tests can manually call start_sync if needed.

        Ok(LoadWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet,
            lock_status,
            accounts,
        })
    })
}

pub fn get_wallet_status_impl(
    state: &AppState,
    request: GetWalletStatusRequest,
) -> IpcResult<GetWalletStatusResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.compute_wallet_status(request.wallet_id)?;
        Ok(GetWalletStatusResponse {
            schema_version: SCHEMA_VERSION,
            status,
        })
    })
}

pub fn unlock_wallet_impl(
    state: &AppState,
    request: UnlockWalletRequest,
) -> IpcResult<UnlockWalletResponse> {
    use zstash_core::domain::WalletLockStatus;
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.unlock_wallet(
            request.wallet_id,
            &request.password,
            request.remember_unlock,
        )?;
        Ok(UnlockWalletResponse {
            schema_version: SCHEMA_VERSION,
            unlocked: status == WalletLockStatus::Unlocked,
        })
    })
}

pub fn lock_wallet_impl(
    state: &AppState,
    request: LockWalletRequest,
) -> IpcResult<LockWalletResponse> {
    use zstash_core::domain::{SyncPhase, SyncProgress, WalletLockStatus};
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.observe_sync_stop_requested(request.wallet_id);
        let _ = state.sync_service.stop_sync(request.wallet_id, None);
        mgr.observe_sync_progress(
            request.wallet_id,
            SyncProgress {
                phase: SyncPhase::Idle,
                scan_frontier_height: 0,
                wallet_tip_height: 0,
                progress_percent: 0,
                eta_seconds: None,
                retry_in_seconds: None,
                error_message: None,
            },
        );
        let status = mgr.lock_wallet(request.wallet_id)?;
        Ok(LockWalletResponse {
            schema_version: SCHEMA_VERSION,
            locked: status == WalletLockStatus::Locked,
        })
    })
}

pub fn logout_wallet_impl(
    state: &AppState,
    request: LogoutWalletRequest,
) -> IpcResult<LogoutWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let _ = state.sync_service.stop_sync(request.wallet_id, None);
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.logout_wallet(request.wallet_id)?;
        Ok(LogoutWalletResponse {
            schema_version: SCHEMA_VERSION,
            success: true,
        })
    })
}

pub fn reauth_wallet_impl(
    state: &AppState,
    request: ReauthWalletRequest,
) -> IpcResult<ReauthWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (token, expires_at) =
            mgr.reauth_wallet(request.wallet_id, &request.password, request.purpose)?;
        Ok(ReauthWalletResponse {
            schema_version: SCHEMA_VERSION,
            reauth_token: token,
            expires_at: system_time_to_unix_ms(expires_at)?,
        })
    })
}

pub fn view_seed_phrase_impl(
    state: &AppState,
    request: ViewSeedPhraseRequest,
) -> IpcResult<ViewSeedPhraseResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let seed_phrase = mgr.view_seed_phrase(request.wallet_id, &request.reauth_token)?;
        Ok(ViewSeedPhraseResponse {
            schema_version: SCHEMA_VERSION,
            seed_phrase,
        })
    })
}
