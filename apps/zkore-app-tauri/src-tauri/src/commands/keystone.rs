use tauri::State;

use zkore_core::ipc::v1::commands::keystone::{
    BuildSigningRequestRequest, BuildSigningRequestResponse, CreateKeystoneWalletRequest,
    CreateKeystoneWalletResponse, FinalizeSigningRequest, FinalizeSigningResponse,
    ImportUfvkRequest, ImportUfvkResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zkore_import_ufvk")]
pub fn zkore_import_ufvk(
    state: State<'_, AppState>,
    request: ImportUfvkRequest,
) -> IpcResult<ImportUfvkResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let account = mgr.import_ufvk(request.wallet_id, &request.ufvk, &request.name)?;
        Ok(ImportUfvkResponse {
            schema_version: SCHEMA_VERSION,
            account,
        })
    })
}

#[tauri::command(rename = "zkore_build_signing_request")]
pub fn zkore_build_signing_request(
    state: State<'_, AppState>,
    request: BuildSigningRequestRequest,
) -> IpcResult<BuildSigningRequestResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.build_signing_request(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
        )
    })
}

#[tauri::command(rename = "zkore_finalize_signing")]
pub fn zkore_finalize_signing(
    state: State<'_, AppState>,
    request: FinalizeSigningRequest,
) -> IpcResult<FinalizeSigningResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.finalize_signing(&request.signed_payload, &request.reauth_token, None)
    })
}

#[tauri::command(rename = "zkore_create_keystone_wallet")]
pub fn zkore_create_keystone_wallet(
    state: State<'_, AppState>,
    request: CreateKeystoneWalletRequest,
) -> IpcResult<CreateKeystoneWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (wallet, account) = mgr.create_keystone_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
            &request.ufvk,
            request.birthday_height,
        )?;

        Ok(CreateKeystoneWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet,
            account,
        })
    })
}
