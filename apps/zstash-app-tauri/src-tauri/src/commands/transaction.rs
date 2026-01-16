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
use zstash_engine::wallet_manager::open_wallet_db_for_tx;

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
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
        mgr.prepare_send(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
            &mut tx_svc,
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

    map_anyhow(|| {
        // Phase 1: Extract context while holding wallet_manager lock briefly
        let (ctx, spending_key, proposal_id) = {
            let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
            let tx_svc = state.tx_service.lock().expect("mutex poisoned");
            let (ctx, spending_key) =
                mgr.prepare_confirm_send(&request.proposal_id, &request.reauth_token, &tx_svc)?;
            (ctx, spending_key, request.proposal_id.clone())
        };
        // wallet_manager lock is released here

        // Phase 2: Open a fresh database connection and perform expensive operations
        // without holding the wallet_manager lock, allowing other operations to proceed.
        let mut conn = open_wallet_db_for_tx(&ctx)?;
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");

        tx_svc.confirm_send(
            &ctx.app_db_path,
            ctx.wallet_id,
            ctx.network,
            &ctx.wallet_dir,
            &ctx.dek,
            &mut conn,
            &ctx.grpc_url,
            &proposal_id,
            spending_key,
            Some(handler),
        )
    })
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
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
        Ok(CancelSendResponse {
            schema_version: SCHEMA_VERSION,
            cancelled: tx_svc.cancel_send(&request.proposal_id),
        })
    })
}

#[tauri::command(rename = "zstash_retry_broadcast")]
pub async fn zstash_retry_broadcast(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: RetryBroadcastRequest,
) -> Result<IpcResult<RetryBroadcastResponse>, ()> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return Ok(IpcResult::Err { err });
    }

    let wallet_manager = Arc::clone(&state.wallet_manager);
    let prepare_wallet_manager = Arc::clone(&wallet_manager);
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

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
        let txid = mgr.retry_broadcast(
            &request.txid,
            &request.reauth_token,
            Some(handler),
            &mut tx_svc,
        )?;
        Ok(RetryBroadcastResponse {
            schema_version: SCHEMA_VERSION,
            txid,
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
            let txid = zstash_engine::wallet_manager::WalletManager::
                execute_prepared_retry_broadcast_task(
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
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
        mgr.list_transactions(
            request.account_id,
            request.limit,
            request.offset,
            &mut tx_svc,
        )
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
        // Phase 1: Extract context while holding wallet_manager lock briefly
        let (ctx, spending_key, account_id, consolidate) = {
            let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
            let (ctx, spending_key) =
                mgr.prepare_shield_funds(request.account_id, &request.reauth_token)?;
            (ctx, spending_key, request.account_id, request.consolidate)
        };
        // wallet_manager lock is released here

        // Phase 2: Open a fresh database connection and perform expensive operations
        // without holding the wallet_manager lock, allowing other operations to proceed.
        let mut conn = open_wallet_db_for_tx(&ctx)?;
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");

        tx_svc.shield_funds(
            &ctx.app_db_path,
            ctx.wallet_id,
            ctx.network,
            &ctx.wallet_dir,
            &ctx.dek,
            &mut conn,
            &ctx.grpc_url,
            account_id,
            consolidate,
            spending_key,
            Some(handler),
        )
    })
}
