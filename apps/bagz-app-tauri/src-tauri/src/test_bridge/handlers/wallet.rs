//! Wallet-related command handlers.

use bagz_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse,
    LockWalletRequest, LockWalletResponse, LogoutWalletRequest, LogoutWalletResponse,
    ReauthWalletRequest, ReauthWalletResponse, UnlockWalletRequest, UnlockWalletResponse,
    ViewSeedPhraseRequest, ViewSeedPhraseResponse,
};
use bagz_core::ipc::v1::common::IpcResult;
use tracing::warn;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;
use crate::wallet_logic;

pub fn list_wallets_impl(
    state: &AppState,
    request: ListWalletsRequest,
) -> IpcResult<ListWalletsResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::list_wallets(state))
}

pub fn create_wallet_impl(
    state: &AppState,
    request: CreateWalletRequest,
) -> IpcResult<CreateWalletResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

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

    map_anyhow(|| wallet_logic::create_wallet(state, request, birthday_height))
}

pub fn load_wallet_impl(
    state: &AppState,
    request: LoadWalletRequest,
) -> IpcResult<LoadWalletResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::load_wallet(state, request.wallet_id))
}

pub fn get_wallet_status_impl(
    state: &AppState,
    request: GetWalletStatusRequest,
) -> IpcResult<GetWalletStatusResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::get_wallet_status(state, request.wallet_id))
}

pub fn unlock_wallet_impl(
    state: &AppState,
    request: UnlockWalletRequest,
) -> IpcResult<UnlockWalletResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::unlock_wallet(state, request))
}

pub fn lock_wallet_impl(
    state: &AppState,
    request: LockWalletRequest,
) -> IpcResult<LockWalletResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::lock_wallet(state, request.wallet_id))
}

pub fn logout_wallet_impl(
    state: &AppState,
    request: LogoutWalletRequest,
) -> IpcResult<LogoutWalletResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::logout_wallet(state, request.wallet_id))
}

pub fn reauth_wallet_impl(
    state: &AppState,
    request: ReauthWalletRequest,
) -> IpcResult<ReauthWalletResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::reauth_wallet(state, request))
}

pub fn view_seed_phrase_impl(
    state: &AppState,
    request: ViewSeedPhraseRequest,
) -> IpcResult<ViewSeedPhraseResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    warn!("view_seed_phrase called - sensitive endpoint accessed");

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::view_seed_phrase(state, request))
}
