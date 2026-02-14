//! Transaction command handlers.

use tracing::warn;
use zstash_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use zstash_core::ipc::v1::common::IpcResult;
use zstash_engine::wallet_manager::open_wallet_db_for_tx;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn prepare_send_impl(
    state: &AppState,
    request: PrepareSendRequest,
) -> IpcResult<PrepareSendResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
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
    })
}

pub fn confirm_send_impl(
    state: &AppState,
    request: ConfirmSendRequest,
) -> IpcResult<ConfirmSendResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    warn!("Test bridge: confirm_send invoked");

    map_anyhow(|| {
        let (ctx, spending_key, proposal_id) = {
            let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
            let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
            let (ctx, spending_key) =
                mgr.prepare_confirm_send(&request.proposal_id, &request.reauth_token, &mut tx_svc)?;
            (ctx, spending_key, request.proposal_id.clone())
        };

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
            None,
        )
    })
}

pub fn cancel_send_impl(
    state: &AppState,
    request: CancelSendRequest,
) -> IpcResult<CancelSendResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

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

pub fn retry_broadcast_impl(
    state: &AppState,
    request: RetryBroadcastRequest,
) -> IpcResult<RetryBroadcastResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let task = {
            let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
            let tx_svc = state.tx_service.lock().expect("mutex poisoned");
            let task =
                mgr.prepare_retry_broadcast_task(&request.txid, &request.reauth_token, &tx_svc)?;
            mgr.validate_retry_broadcast_task(&task)?;
            task
        };

        let txid =
            zstash_engine::wallet_manager::WalletManager::execute_prepared_retry_broadcast_task(
                task, None, None,
            )?;
        Ok(RetryBroadcastResponse {
            schema_version: SCHEMA_VERSION,
            txid,
        })
    })
}

pub fn list_transactions_impl(
    state: &AppState,
    request: ListTransactionsRequest,
) -> IpcResult<ListTransactionsResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

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

pub fn shield_funds_impl(
    state: &AppState,
    request: ShieldFundsRequest,
) -> IpcResult<ShieldFundsResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let (ctx, spending_key) = {
            let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.prepare_shield_funds(request.account_id, &request.reauth_token)?
        };

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
            request.account_id,
            request.consolidate,
            spending_key,
            None,
        )
    })
}
