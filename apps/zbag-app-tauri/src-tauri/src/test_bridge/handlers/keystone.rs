//! Keystone hardware wallet command handlers.

use zbag_core::ipc::v1::commands::keystone::{
    BuildSigningRequestRequest, BuildSigningRequestResponse, CreateKeystoneWalletRequest,
    CreateKeystoneWalletResponse, FinalizeSigningRequest, FinalizeSigningResponse,
    ImportUfvkRequest, ImportUfvkResponse,
};
use zbag_core::ipc::v1::common::IpcResult;
use zbag_engine::wallet_manager::WalletManager;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn import_ufvk_impl(
    state: &AppState,
    request: ImportUfvkRequest,
) -> IpcResult<ImportUfvkResponse> {
    use zbag_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let account = mgr.import_ufvk(
            request.wallet_id,
            &request.ufvk,
            &request.name,
            request.seed_fingerprint.as_deref(),
            request.zip32_account_index,
        )?;
        Ok(ImportUfvkResponse {
            schema_version: SCHEMA_VERSION,
            account,
        })
    })
}

pub fn build_signing_request_impl(
    state: &AppState,
    request: BuildSigningRequestRequest,
) -> IpcResult<BuildSigningRequestResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
        mgr.build_signing_request(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
            &mut tx_svc,
        )
    })
}

pub fn finalize_signing_impl(
    state: &AppState,
    request: FinalizeSigningRequest,
) -> IpcResult<FinalizeSigningResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let task = {
            let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
            mgr.prepare_finalize_signing_task(
                &request.signing_request_id,
                &request.signed_payload,
                &request.reauth_token,
                &mut tx_svc,
            )?
        };
        WalletManager::execute_prepared_finalize_signing_task(task, None)
    })
}

pub fn create_keystone_wallet_impl(
    state: &AppState,
    request: CreateKeystoneWalletRequest,
) -> IpcResult<CreateKeystoneWalletResponse> {
    use zbag_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let (mut mgr, mut tx_svc) = state.lock_wallet_then_tx_service();
        let (wallet, account) = mgr.create_keystone_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
            &request.ufvk,
            request.birthday_height,
            request.seed_fingerprint.as_deref(),
            request.zip32_account_index,
            &mut tx_svc,
        )?;

        Ok(CreateKeystoneWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet,
            account,
        })
    })
}
