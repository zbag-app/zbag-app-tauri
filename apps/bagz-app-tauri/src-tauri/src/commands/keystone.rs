use tauri::State;

use bagz_core::ipc::v1::commands::keystone::{
    BuildSigningRequestRequest, BuildSigningRequestResponse, CreateKeystoneWalletRequest,
    CreateKeystoneWalletResponse, FinalizeSigningRequest, FinalizeSigningResponse,
    ImportUfvkRequest, ImportUfvkResponse,
};
use bagz_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use bagz_engine::wallet_manager::WalletManager;

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "bagz_import_ufvk")]
pub fn bagz_import_ufvk(
    state: State<'_, AppState>,
    request: ImportUfvkRequest,
) -> IpcResult<ImportUfvkResponse> {
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

#[tauri::command(rename = "bagz_build_signing_request")]
pub fn bagz_build_signing_request(
    state: State<'_, AppState>,
    request: BuildSigningRequestRequest,
) -> IpcResult<BuildSigningRequestResponse> {
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

#[tauri::command(rename = "bagz_finalize_signing")]
pub fn bagz_finalize_signing(
    state: State<'_, AppState>,
    request: FinalizeSigningRequest,
) -> IpcResult<FinalizeSigningResponse> {
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

#[tauri::command(rename = "bagz_create_keystone_wallet")]
pub fn bagz_create_keystone_wallet(
    state: State<'_, AppState>,
    request: CreateKeystoneWalletRequest,
) -> IpcResult<CreateKeystoneWalletResponse> {
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
