//! Transaction command handlers.

use tracing::warn;
use zstash_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

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
        mgr.prepare_send(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
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
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.confirm_send(&request.proposal_id, &request.reauth_token, None)
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
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        Ok(CancelSendResponse {
            schema_version: SCHEMA_VERSION,
            cancelled: mgr.cancel_send(&request.proposal_id),
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
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let txid = mgr.retry_broadcast(&request.txid, &request.reauth_token, None, None)?;
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
        mgr.list_transactions(request.account_id, request.limit, request.offset)
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
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.shield_funds(
            request.account_id,
            request.consolidate,
            &request.reauth_token,
            None,
        )
    })
}
