use std::sync::Arc;
use std::time::Instant;

use tauri::{Runtime, State};

use zbag_core::errors;
use zbag_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use zbag_core::ipc::v1::common::{IpcError, IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zbag_engine::error::find_engine_ipc_error;
use zbag_engine::wallet_manager::WalletManager;

use crate::events;
use crate::state::AppState;

use super::util::{map_anyhow, to_ipc_error};

#[tauri::command(rename = "zbag_prepare_send")]
pub fn zbag_prepare_send(
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
        phase = "tauri.zbag_prepare_send.start",
        elapsed_ms = 0u128,
        error_code = "none",
        error_message = "",
        "send lifecycle event"
    );

    let result = {
        let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
        mgr.prepare_send(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
            &mut tx_svc,
        )
    };

    match result {
        Ok(response) => {
            tracing::info!(
                wallet_id = "-",
                account_id,
                proposal_id = %response.proposal_id,
                txid = "-",
                phase = "tauri.zbag_prepare_send.success",
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
                phase = "tauri.zbag_prepare_send.error",
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

#[tauri::command(rename = "zbag_confirm_send")]
pub fn zbag_confirm_send<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    request: ConfirmSendRequest,
) -> IpcResult<ConfirmSendResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handler = Arc::new(move |event| {
        let _ = events::emit_transaction_changed(&app, event);
    });

    map_anyhow(|| {
        let task = {
            let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
            mgr.prepare_confirm_send_task(&request.proposal_id, &request.reauth_token, &mut tx_svc)?
        };
        WalletManager::execute_prepared_confirm_send_task(task, Some(handler))
    })
}

#[tauri::command(rename = "zbag_cancel_send")]
pub fn zbag_cancel_send(
    state: State<'_, AppState>,
    request: CancelSendRequest,
) -> IpcResult<CancelSendResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
        Ok(CancelSendResponse {
            schema_version: SCHEMA_VERSION,
            cancelled: tx_svc.cancel_send(&request.proposal_id),
        })
    })
}

#[tauri::command(rename = "zbag_retry_broadcast")]
pub async fn zbag_retry_broadcast<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    request: RetryBroadcastRequest,
) -> Result<IpcResult<RetryBroadcastResponse>, ()> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return Ok(IpcResult::Err { err });
    }

    let wallet_manager = Arc::clone(&state.wallet_manager);
    let tx_service = Arc::clone(&state.tx_service);
    let txid = request.txid;
    let reauth_token = request.reauth_token;
    let tx_app = app.clone();
    let failover_app = app.clone();
    let handler = Arc::new(move |event| {
        let _ = events::emit_transaction_changed(&tx_app, event);
    });
    let failover_handler = Arc::new(move |event| {
        let _ = events::emit_server_failover(&failover_app, event);
    });

    let prepare_join = tauri::async_runtime::spawn_blocking(move || {
        map_anyhow(|| {
            let mut mgr = wallet_manager.lock().expect("mutex poisoned");
            let tx_svc = tx_service.lock().expect("mutex poisoned");
            let task = mgr.prepare_retry_broadcast_task(&txid, &reauth_token, &tx_svc)?;
            mgr.validate_retry_broadcast_task(&task)?;
            Ok(task)
        })
    });

    let task = match prepare_join.await {
        Ok(IpcResult::Ok { ok }) => ok,
        Ok(IpcResult::Err { err }) => return Ok(IpcResult::Err { err }),
        Err(_) => {
            return Ok(IpcResult::Err {
                err: IpcError {
                    code: errors::INTERNAL_ERROR.to_string(),
                    message: "internal error".to_string(),
                    details: None,
                },
            });
        }
    };

    let execute_join = tauri::async_runtime::spawn_blocking(move || {
        map_anyhow(|| {
            let txid =
                zbag_engine::wallet_manager::WalletManager::execute_prepared_retry_broadcast_task(
                    task,
                    Some(handler),
                    Some(failover_handler),
                )?;
            Ok(RetryBroadcastResponse {
                schema_version: SCHEMA_VERSION,
                txid,
            })
        })
    });

    match execute_join.await {
        Ok(res) => Ok(res),
        Err(_) => Ok(IpcResult::Err {
            err: IpcError {
                code: errors::INTERNAL_ERROR.to_string(),
                message: "internal error".to_string(),
                details: None,
            },
        }),
    }
}

#[tauri::command(rename = "zbag_list_transactions")]
pub fn zbag_list_transactions(
    state: State<'_, AppState>,
    request: ListTransactionsRequest,
) -> IpcResult<ListTransactionsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
        mgr.list_transactions(
            request.account_id,
            request.limit,
            request.offset,
            &mut tx_svc,
        )
    })
}

#[tauri::command(rename = "zbag_shield_funds")]
pub fn zbag_shield_funds<R: Runtime>(
    app: tauri::AppHandle<R>,
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
        let task = {
            let (mut mgr, tx_svc) = state.lock_wallet_then_tx_service();
            mgr.prepare_shield_funds_task(
                request.account_id,
                request.consolidate,
                &request.reauth_token,
                &tx_svc,
            )?
        };
        WalletManager::execute_prepared_shield_funds_task(task, Some(handler))
    })
}
