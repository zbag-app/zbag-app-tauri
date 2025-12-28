use std::collections::HashMap;

use tauri::State;

use zkore_core::ipc::v1::commands::backup::{
    GetBackupChallengeRequest, GetBackupChallengeResponse, RestoreWalletRequest,
    RestoreWalletResponse, VerifyBackupRequest, VerifyBackupResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zkore_get_backup_challenge")]
pub fn zkore_get_backup_challenge(
    state: State<'_, AppState>,
    request: GetBackupChallengeRequest,
) -> IpcResult<GetBackupChallengeResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let challenge = mgr.get_backup_challenge(request.wallet_id)?;
        Ok(GetBackupChallengeResponse {
            schema_version: SCHEMA_VERSION,
            challenge: zkore_core::ipc::v1::commands::backup::BackupChallenge {
                challenge_id: challenge.challenge_id,
                indices: challenge.indices,
                expires_at: challenge.expires_at,
            },
        })
    })())
}

#[tauri::command(rename = "zkore_verify_backup")]
pub fn zkore_verify_backup(
    state: State<'_, AppState>,
    request: VerifyBackupRequest,
) -> IpcResult<VerifyBackupResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let word_challenges: HashMap<u8, String> = request.word_challenges.into_iter().collect();
        mgr.verify_backup(request.wallet_id, &request.challenge_id, &word_challenges)?;
        Ok(VerifyBackupResponse {
            schema_version: SCHEMA_VERSION,
            verified: true,
        })
    })())
}

#[tauri::command(rename = "zkore_restore_wallet")]
pub fn zkore_restore_wallet(
    state: State<'_, AppState>,
    request: RestoreWalletRequest,
) -> IpcResult<RestoreWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let restored = mgr.restore_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
            &request.seed_phrase,
            request.birthday_date,
        )?;

        Ok(RestoreWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: restored.wallet,
            birthday_height: restored.birthday_height,
        })
    })())
}
