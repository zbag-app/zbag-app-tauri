use std::collections::HashMap;

use tauri::State;

use zstash_core::ipc::v1::commands::backup::{
    GetBackupChallengeRequest, GetBackupChallengeResponse, RestoreWalletRequest,
    RestoreWalletResponse, VerifyBackupRequest, VerifyBackupResponse,
};
use zstash_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zstash_core::sensitive::SensitiveString;

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zstash_get_backup_challenge")]
pub fn zstash_get_backup_challenge(
    state: State<'_, AppState>,
    request: GetBackupChallengeRequest,
) -> IpcResult<GetBackupChallengeResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let challenge = mgr.get_backup_challenge(request.wallet_id)?;
        Ok(GetBackupChallengeResponse {
            schema_version: SCHEMA_VERSION,
            challenge: zstash_core::ipc::v1::commands::backup::BackupChallenge {
                challenge_id: challenge.challenge_id,
                indices: challenge.indices,
                expires_at: challenge.expires_at,
            },
        })
    })
}

#[tauri::command(rename = "zstash_verify_backup")]
pub fn zstash_verify_backup(
    state: State<'_, AppState>,
    request: VerifyBackupRequest,
) -> IpcResult<VerifyBackupResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let word_challenges: HashMap<u8, SensitiveString> =
            request.word_challenges.into_iter().collect();
        mgr.verify_backup(request.wallet_id, &request.challenge_id, &word_challenges)?;
        Ok(VerifyBackupResponse {
            schema_version: SCHEMA_VERSION,
            verified: true,
        })
    })
}

#[tauri::command(rename = "zstash_restore_wallet")]
pub fn zstash_restore_wallet(
    state: State<'_, AppState>,
    request: RestoreWalletRequest,
) -> IpcResult<RestoreWalletResponse> {
    let RestoreWalletRequest {
        schema_version,
        name,
        network,
        password,
        remember_unlock,
        seed_phrase,
        birthday_date,
    } = request;

    if let Err(err) = ensure_schema_version(schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
        let restored = mgr.restore_wallet(
            &name,
            network,
            &password,
            remember_unlock,
            seed_phrase,
            birthday_date,
            &mut tx_svc,
        )?;

        Ok(RestoreWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: restored.wallet,
            birthday_height: restored.birthday_height,
        })
    })
}
