use std::sync::Arc;

use tauri::State;

use zkore_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::events;
use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zkore_prepare_send")]
pub fn zkore_prepare_send(
    state: State<'_, AppState>,
    request: PrepareSendRequest,
) -> IpcResult<PrepareSendResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.prepare_send(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
        )
    })())
}

#[tauri::command(rename = "zkore_confirm_send")]
pub fn zkore_confirm_send(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ConfirmSendRequest,
) -> IpcResult<ConfirmSendResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handler = Arc::new(move |event| {
        let _ = events::emit_transaction_changed(&app, event);
    });

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.confirm_send(&request.proposal_id, &request.reauth_token, Some(handler))
    })())
}

#[tauri::command(rename = "zkore_cancel_send")]
pub fn zkore_cancel_send(
    state: State<'_, AppState>,
    request: CancelSendRequest,
) -> IpcResult<CancelSendResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        Ok(CancelSendResponse {
            schema_version: SCHEMA_VERSION,
            cancelled: mgr.cancel_send(&request.proposal_id),
        })
    })())
}

#[tauri::command(rename = "zkore_retry_broadcast")]
pub fn zkore_retry_broadcast(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: RetryBroadcastRequest,
) -> IpcResult<RetryBroadcastResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handler = Arc::new(move |event| {
        let _ = events::emit_transaction_changed(&app, event);
    });

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let txid = mgr.retry_broadcast(&request.txid, &request.reauth_token, Some(handler))?;
        Ok(RetryBroadcastResponse {
            schema_version: SCHEMA_VERSION,
            txid,
        })
    })())
}

#[tauri::command(rename = "zkore_list_transactions")]
pub fn zkore_list_transactions(
    state: State<'_, AppState>,
    request: ListTransactionsRequest,
) -> IpcResult<ListTransactionsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.list_transactions(request.account_id, request.limit, request.offset)
    })())
}

#[tauri::command(rename = "zkore_shield_funds")]
pub fn zkore_shield_funds(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ShieldFundsRequest,
) -> IpcResult<ShieldFundsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handler = Arc::new(move |event| {
        let _ = events::emit_transaction_changed(&app, event);
    });

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.shield_funds(
            request.account_id,
            request.consolidate,
            &request.reauth_token,
            Some(handler),
        )
    })())
}
