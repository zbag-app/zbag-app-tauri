//! Swap command handlers.

use zbag_core::domain::SwapIntent;
use zbag_core::errors;
use zbag_core::ipc::v1::commands::swap::{
    GetSwapStatusRequest, GetSwapStatusResponse, ListSwapsRequest, ListSwapsResponse,
    RequestSwapQuoteRequest, RequestSwapQuoteResponse, StartSwapRequest, StartSwapResponse,
};
use zbag_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn request_swap_quote_impl(
    state: &AppState,
    request: RequestSwapQuoteRequest,
) -> IpcResult<RequestSwapQuoteResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zbag_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        let intent = SwapIntent {
            swap_type: request.swap_type,
            swap_mode: request.swap_mode,
            input_asset: request.input_asset,
            input_amount: request.input_amount,
            output_asset: request.output_asset,
            output_amount: request.output_amount,
            destination_address: request.destination_address,
            refund_address: request.refund_address,
        };

        state
            .swap_service
            .request_swap_quote(wallet.id, wallet.network, intent)
    })
}

pub fn start_swap_impl(
    state: &AppState,
    request: StartSwapRequest,
) -> IpcResult<StartSwapResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zbag_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state.swap_service.start_swap(
            wallet.id,
            wallet.network,
            &request.quote_id,
            request.allow_transparent_interaction,
            request.reauth_token.as_deref(),
            None,
        )
    })
}

pub fn get_swap_status_impl(
    state: &AppState,
    request: GetSwapStatusRequest,
) -> IpcResult<GetSwapStatusResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zbag_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state
            .swap_service
            .get_swap_status(wallet.id, request.swap_id)
    })
}

pub fn list_swaps_impl(
    state: &AppState,
    request: ListSwapsRequest,
) -> IpcResult<ListSwapsResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zbag_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state.swap_service.list_swaps(wallet.id)
    })
}
