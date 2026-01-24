//! Backup and restore command handlers.

use zstash_core::ipc::v1::commands::backup::{
    GetBackupChallengeRequest, GetBackupChallengeResponse, RestoreWalletRequest,
    RestoreWalletResponse, VerifyBackupRequest, VerifyBackupResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn get_backup_challenge_impl(
    state: &AppState,
    request: GetBackupChallengeRequest,
) -> IpcResult<GetBackupChallengeResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

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

pub fn verify_backup_impl(
    state: &AppState,
    request: VerifyBackupRequest,
) -> IpcResult<VerifyBackupResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};
    use zstash_core::sensitive::SensitiveString;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let word_challenges: std::collections::HashMap<u8, SensitiveString> =
            request.word_challenges.into_iter().collect();
        mgr.verify_backup(request.wallet_id, &request.challenge_id, &word_challenges)?;
        Ok(VerifyBackupResponse {
            schema_version: SCHEMA_VERSION,
            verified: true,
        })
    })
}

pub fn restore_wallet_impl(
    state: &AppState,
    request: RestoreWalletRequest,
) -> IpcResult<RestoreWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

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
        let restored = mgr.restore_wallet(
            &name,
            network,
            &password,
            remember_unlock,
            seed_phrase,
            birthday_date,
        )?;

        Ok(RestoreWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: restored.wallet,
            birthday_height: restored.birthday_height,
        })
    })
}
