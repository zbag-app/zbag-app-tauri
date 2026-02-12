use std::sync::Arc;
use std::time::Instant;

use tauri::State;

use zstash_core::errors;
use zstash_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use zstash_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zstash_engine::error::find_engine_ipc_error;

use crate::events;
use crate::state::AppState;

use super::util::{map_anyhow, to_ipc_error};

#[tauri::command(rename = "zstash_prepare_send")]
pub fn zstash_prepare_send(
    state: State<'_, AppState>,
    request: PrepareSendRequest,
) -> IpcResult<PrepareSendResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let account_id = request.account_id;
    let started_at = Instant::now();
    tracing::info!(
        wallet_id = "-",
        account_id,
        proposal_id = "-",
        txid = "-",
        phase = "tauri.zstash_prepare_send.start",
        elapsed_ms = 0u128,
        error_code = "none",
        error_message = "",
        "send lifecycle event"
    );

    let result = (|| {
        let lock_wait_started = Instant::now();
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        if zstash_engine::logging::temporary_debug_enabled() {
            tracing::debug!(
                phase = "tauri.zstash_prepare_send.wallet_manager_lock_acquired",
                elapsed_ms = lock_wait_started.elapsed().as_millis(),
                "temporary send debug"
            );
        }
        mgr.prepare_send(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
        )
    })();

    match result {
        Ok(response) => {
            tracing::info!(
                wallet_id = "-",
                account_id,
                proposal_id = %response.proposal_id,
                txid = "-",
                phase = "tauri.zstash_prepare_send.success",
                elapsed_ms = started_at.elapsed().as_millis(),
                error_code = "none",
                error_message = "",
                "send lifecycle event"
            );
            IpcResult::ok(response)
        }
        Err(err) => {
            let (error_code, error_message) = match find_engine_ipc_error(&err) {
                Some(engine) => (engine.code.to_string(), engine.message.clone()),
                None => (errors::INTERNAL_ERROR.to_string(), err.to_string()),
            };
            tracing::warn!(
                wallet_id = "-",
                account_id,
                proposal_id = "-",
                txid = "-",
                phase = "tauri.zstash_prepare_send.error",
                elapsed_ms = started_at.elapsed().as_millis(),
                error_code = %error_code,
                error_message = %error_message,
                "send lifecycle event"
            );
            IpcResult::Err {
                err: to_ipc_error(err),
            }
        }
    }
}

#[tauri::command(rename = "zstash_confirm_send")]
pub fn zstash_confirm_send(
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

    let proposal_id = request.proposal_id.clone();
    let started_at = Instant::now();
    tracing::info!(
        wallet_id = "-",
        account_id = "unknown",
        proposal_id = %proposal_id,
        txid = "-",
        phase = "tauri.zstash_confirm_send.start",
        elapsed_ms = 0u128,
        error_code = "none",
        error_message = "",
        "send lifecycle event"
    );

    let result = (|| {
        let lock_wait_started = Instant::now();
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        if zstash_engine::logging::temporary_debug_enabled() {
            tracing::debug!(
                proposal_id = %proposal_id,
                phase = "tauri.zstash_confirm_send.wallet_manager_lock_acquired",
                elapsed_ms = lock_wait_started.elapsed().as_millis(),
                "temporary send debug"
            );
        }
        mgr.confirm_send(&request.proposal_id, &request.reauth_token, Some(handler))
    })();

    match result {
        Ok(response) => {
            tracing::info!(
                wallet_id = "-",
                account_id = "unknown",
                proposal_id = %proposal_id,
                txid = %response.txid,
                phase = "tauri.zstash_confirm_send.success",
                elapsed_ms = started_at.elapsed().as_millis(),
                error_code = "none",
                error_message = "",
                "send lifecycle event"
            );
            IpcResult::ok(response)
        }
        Err(err) => {
            let (error_code, error_message) = match find_engine_ipc_error(&err) {
                Some(engine) => (engine.code.to_string(), engine.message.clone()),
                None => (errors::INTERNAL_ERROR.to_string(), err.to_string()),
            };
            tracing::warn!(
                wallet_id = "-",
                account_id = "unknown",
                proposal_id = %proposal_id,
                txid = "-",
                phase = "tauri.zstash_confirm_send.error",
                elapsed_ms = started_at.elapsed().as_millis(),
                error_code = %error_code,
                error_message = %error_message,
                "send lifecycle event"
            );
            IpcResult::Err {
                err: to_ipc_error(err),
            }
        }
    }
}

#[tauri::command(rename = "zstash_cancel_send")]
pub fn zstash_cancel_send(
    state: State<'_, AppState>,
    request: CancelSendRequest,
) -> IpcResult<CancelSendResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        Ok(CancelSendResponse {
            schema_version: SCHEMA_VERSION,
            cancelled: mgr.cancel_send(&request.proposal_id),
        })
    })
}

#[tauri::command(rename = "zstash_retry_broadcast")]
pub fn zstash_retry_broadcast(
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

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let txid = mgr.retry_broadcast(&request.txid, &request.reauth_token, Some(handler))?;
        Ok(RetryBroadcastResponse {
            schema_version: SCHEMA_VERSION,
            txid,
        })
    })
}

#[tauri::command(rename = "zstash_list_transactions")]
pub fn zstash_list_transactions(
    state: State<'_, AppState>,
    request: ListTransactionsRequest,
) -> IpcResult<ListTransactionsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.list_transactions(request.account_id, request.limit, request.offset)
    })
}

#[tauri::command(rename = "zstash_shield_funds")]
pub fn zstash_shield_funds(
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

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.shield_funds(
            request.account_id,
            request.consolidate,
            &request.reauth_token,
            Some(handler),
        )
    })
}
