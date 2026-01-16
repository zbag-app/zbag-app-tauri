use std::sync::Arc;

use tauri::State;

use zstash_core::ipc::v1::commands::job::{
    CancelJobRequest, CancelJobResponse, GetJobStatusRequest, GetJobStatusResponse,
    ListJobsRequest, ListJobsResponse, StartSendJobRequest, StartSendJobResponse,
    StartShieldJobRequest, StartShieldJobResponse,
};
use zstash_core::ipc::v1::common::{IpcResult, ensure_schema_version};

use crate::events;
use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zstash_start_send_job")]
pub fn zstash_start_send_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: StartSendJobRequest,
) -> IpcResult<StartSendJobResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let job_handler = {
        let app = app.clone();
        Arc::new(move |event| {
            let _ = events::emit_job_progress(&app, event);
        })
    };

    let tx_handler = Arc::new(move |event| {
        let _ = events::emit_transaction_changed(&app, event);
    });

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let tor_manager = Some(Arc::clone(&state.tor_manager));
        mgr.start_send_job(
            &request.proposal_id,
            &request.reauth_token,
            tor_manager,
            Some(job_handler),
            Some(tx_handler),
        )
    })
}

#[tauri::command(rename = "zstash_start_shield_job")]
pub fn zstash_start_shield_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: StartShieldJobRequest,
) -> IpcResult<StartShieldJobResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let job_handler = {
        let app = app.clone();
        Arc::new(move |event| {
            let _ = events::emit_job_progress(&app, event);
        })
    };

    let tx_handler = Arc::new(move |event| {
        let _ = events::emit_transaction_changed(&app, event);
    });

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let tor_manager = Some(Arc::clone(&state.tor_manager));
        mgr.start_shield_job(
            request.account_id,
            request.consolidate,
            &request.reauth_token,
            tor_manager,
            Some(job_handler),
            Some(tx_handler),
        )
    })
}

#[tauri::command(rename = "zstash_cancel_job")]
pub fn zstash_cancel_job(
    state: State<'_, AppState>,
    request: CancelJobRequest,
) -> IpcResult<CancelJobResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.cancel_job(&request.job_id)
    })
}

#[tauri::command(rename = "zstash_get_job_status")]
pub fn zstash_get_job_status(
    state: State<'_, AppState>,
    request: GetJobStatusRequest,
) -> IpcResult<GetJobStatusResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.get_job_status(&request.job_id)
    })
}

#[tauri::command(rename = "zstash_list_jobs")]
pub fn zstash_list_jobs(
    state: State<'_, AppState>,
    request: ListJobsRequest,
) -> IpcResult<ListJobsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.list_jobs()
    })
}
