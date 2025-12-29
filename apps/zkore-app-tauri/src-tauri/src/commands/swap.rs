use std::sync::Arc;

use tauri::State;

use zkore_core::domain::SwapIntent;
use zkore_core::errors;
use zkore_core::ipc::v1::commands::swap::{
    GetSwapStatusRequest, GetSwapStatusResponse, ListSwapsRequest, ListSwapsResponse,
    RequestSwapQuoteRequest, RequestSwapQuoteResponse, StartSwapRequest, StartSwapResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, ensure_schema_version};

use crate::events;
use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zkore_request_swap_quote")]
pub fn zkore_request_swap_quote(
    state: State<'_, AppState>,
    request: RequestSwapQuoteRequest,
) -> IpcResult<RequestSwapQuoteResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zkore_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        let intent = SwapIntent {
            swap_type: request.swap_type,
            input_asset: request.input_asset,
            input_amount: request.input_amount,
            output_asset: request.output_asset,
            destination_address: request.destination_address,
            refund_address: request.refund_address,
        };

        state
            .swap_service
            .request_swap_quote(wallet.id, wallet.network, intent)
    })
}

#[tauri::command(rename = "zkore_start_swap")]
pub fn zkore_start_swap(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: StartSwapRequest,
) -> IpcResult<StartSwapResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handler = Arc::new(move |event| {
        let _ = events::emit_swap_changed(&app, event);
    });

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zkore_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state.swap_service.start_swap(
            wallet.id,
            wallet.network,
            &request.quote_id,
            request.allow_transparent_interaction,
            request.reauth_token.as_deref(),
            Some(handler),
        )
    })
}

#[tauri::command(rename = "zkore_get_swap_status")]
pub fn zkore_get_swap_status(
    state: State<'_, AppState>,
    request: GetSwapStatusRequest,
) -> IpcResult<GetSwapStatusResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zkore_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state.swap_service.get_swap_status(wallet.id, request.swap_id)
    })
}

#[tauri::command(rename = "zkore_list_swaps")]
pub fn zkore_list_swaps(
    state: State<'_, AppState>,
    request: ListSwapsRequest,
) -> IpcResult<ListSwapsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zkore_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state.swap_service.list_swaps(wallet.id)
    })
}
