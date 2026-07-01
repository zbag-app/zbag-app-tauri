//! Exchange rate and fiat settings command handlers.

use zbag_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetExchangeRateResponse, GetFiatSettingsRequest,
    GetFiatSettingsResponse, SetFiatSettingsRequest, SetFiatSettingsResponse,
};
use zbag_core::ipc::v1::common::IpcResult;

use crate::exchange_logic;
use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn get_fiat_settings_impl(
    state: &AppState,
    request: GetFiatSettingsRequest,
) -> IpcResult<GetFiatSettingsResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| exchange_logic::get_fiat_settings(state))
}

pub fn set_fiat_settings_impl(
    state: &AppState,
    request: SetFiatSettingsRequest,
) -> IpcResult<SetFiatSettingsResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| exchange_logic::set_fiat_settings(state, request))
}

pub async fn get_exchange_rate_impl(
    state: &AppState,
    request: GetExchangeRateRequest,
) -> IpcResult<GetExchangeRateResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    exchange_logic::get_exchange_rate(state, request).await
}
