//! Transaction command handlers.

use bagz_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use bagz_core::ipc::v1::common::IpcResult;
use bagz_engine::wallet_manager::WalletManager;
use tracing::warn;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn prepare_send_impl(
    state: &AppState,
    request: PrepareSendRequest,
) -> IpcResult<PrepareSendResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
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
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    warn!("Test bridge: confirm_send invoked");

    map_anyhow(|| {
        let task = {
            let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
            mgr.prepare_confirm_send_task(&request.proposal_id, &request.reauth_token, &mut tx_svc)?
        };

        WalletManager::execute_prepared_confirm_send_task(task, None)
    })
}

pub fn cancel_send_impl(
    state: &AppState,
    request: CancelSendRequest,
) -> IpcResult<CancelSendResponse> {
    use bagz_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

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
    use bagz_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let task = {
            let (mut mgr, tx_svc) = state.lock_wallet_then_tx_service();
            let task =
                mgr.prepare_retry_broadcast_task(&request.txid, &request.reauth_token, &tx_svc)?;
            mgr.validate_retry_broadcast_task(&task)?;
            task
        };

        let txid =
            bagz_engine::wallet_manager::WalletManager::execute_prepared_retry_broadcast_task(
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
    use bagz_core::ipc::v1::common::ensure_schema_version;

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

pub fn shield_funds_impl(
    state: &AppState,
    request: ShieldFundsRequest,
) -> IpcResult<ShieldFundsResponse> {
    use bagz_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

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

        WalletManager::execute_prepared_shield_funds_task(task, None)
    })
}
