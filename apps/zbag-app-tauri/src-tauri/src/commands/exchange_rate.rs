use tauri::State;

use zbag_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetExchangeRateResponse, GetFiatSettingsRequest,
    GetFiatSettingsResponse, SetFiatSettingsRequest, SetFiatSettingsResponse,
};
use zbag_core::ipc::v1::common::{IpcResult, ensure_schema_version};

use crate::exchange_logic;
use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zbag_get_fiat_settings")]
pub fn zbag_get_fiat_settings(
    state: State<'_, AppState>,
    request: GetFiatSettingsRequest,
) -> IpcResult<GetFiatSettingsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| exchange_logic::get_fiat_settings(state.inner()))
}

#[tauri::command(rename = "zbag_set_fiat_settings")]
pub fn zbag_set_fiat_settings(
    state: State<'_, AppState>,
    request: SetFiatSettingsRequest,
) -> IpcResult<SetFiatSettingsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| exchange_logic::set_fiat_settings(state.inner(), request))
}

#[tauri::command(rename = "zbag_get_exchange_rate")]
pub async fn zbag_get_exchange_rate(
    state: State<'_, AppState>,
    request: GetExchangeRateRequest,
) -> Result<IpcResult<GetExchangeRateResponse>, ()> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return Ok(IpcResult::Err { err });
    }

    Ok(exchange_logic::get_exchange_rate(state.inner(), request).await)
}
