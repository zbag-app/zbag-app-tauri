use zbag_core::domain::FiatDisplaySettings;
use zbag_core::errors;
use zbag_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetExchangeRateResponse, GetFiatSettingsResponse,
    SetFiatSettingsRequest, SetFiatSettingsResponse,
};
use zbag_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION};
use zbag_engine::db::fiat_meta;
use zbag_engine::error::ipc_err;

use crate::state::AppState;
use crate::time_utils::system_time_to_unix_ms;

pub fn get_fiat_settings(state: &AppState) -> anyhow::Result<GetFiatSettingsResponse> {
    let mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let settings =
        fiat_meta::get_fiat_settings(mgr.app_db().conn()).map_err(|e| anyhow::anyhow!(e))?;

    Ok(GetFiatSettingsResponse {
        schema_version: SCHEMA_VERSION,
        settings,
    })
}

pub fn set_fiat_settings(
    state: &AppState,
    request: SetFiatSettingsRequest,
) -> anyhow::Result<SetFiatSettingsResponse> {
    if request.enabled && !request.privacy_acknowledged {
        return Err(ipc_err(
            errors::EXCHANGE_RATE_PRIVACY_ACK_REQUIRED,
            "Privacy acknowledgement required to enable fiat display",
        ));
    }

    let settings = FiatDisplaySettings {
        enabled: request.enabled,
        currency: request.currency,
        privacy_acknowledged: request.privacy_acknowledged,
    };

    let updated_at_ms = system_time_to_unix_ms(std::time::SystemTime::now())
        .map_err(|e| ipc_err(errors::INTERNAL_ERROR, format!("time error: {e}")))?;

    let mgr = state.wallet_manager.lock().expect("mutex poisoned");
    fiat_meta::upsert_fiat_settings(mgr.app_db().conn(), &settings, updated_at_ms)
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(SetFiatSettingsResponse {
        schema_version: SCHEMA_VERSION,
        settings,
    })
}

pub async fn get_exchange_rate(
    state: &AppState,
    request: GetExchangeRateRequest,
) -> IpcResult<GetExchangeRateResponse> {
    let settings = {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        match fiat_meta::get_fiat_settings(mgr.app_db().conn()) {
            Ok(s) => s,
            Err(e) => {
                return IpcResult::err(
                    errors::INTERNAL_ERROR,
                    format!("Failed to get fiat settings: {e}"),
                );
            }
        }
    };

    if !settings.enabled {
        return IpcResult::ok(GetExchangeRateResponse {
            schema_version: SCHEMA_VERSION,
            rate: None,
            is_stale: true,
            fiat_enabled: false,
            refresh_cooldown_secs: 0,
        });
    }

    let cooldown = state.exchange_rate_service.refresh_cooldown_secs() as u32;

    if let Some(rate) = state
        .exchange_rate_service
        .get_cached_rate(settings.currency)
        && !rate.is_stale()
        && !request.force_refresh
    {
        return IpcResult::ok(GetExchangeRateResponse {
            schema_version: SCHEMA_VERSION,
            rate: Some(rate),
            is_stale: false,
            fiat_enabled: true,
            refresh_cooldown_secs: cooldown,
        });
    }

    match state
        .exchange_rate_service
        .get_rate(settings.currency, request.force_refresh)
        .await
    {
        Ok(rate) => {
            let is_stale = rate.is_stale();
            IpcResult::ok(GetExchangeRateResponse {
                schema_version: SCHEMA_VERSION,
                rate: Some(rate),
                is_stale,
                fiat_enabled: true,
                refresh_cooldown_secs: state.exchange_rate_service.refresh_cooldown_secs() as u32,
            })
        }
        Err(zbag_network::exchange_rate::ExchangeRateError::RateLimited(secs)) => {
            let cached = state
                .exchange_rate_service
                .get_cached_rate(settings.currency);
            let is_stale = cached.as_ref().is_some_and(|r| r.is_stale());
            IpcResult::ok(GetExchangeRateResponse {
                schema_version: SCHEMA_VERSION,
                rate: cached,
                is_stale,
                fiat_enabled: true,
                refresh_cooldown_secs: secs as u32,
            })
        }
        Err(e) => {
            let cached = state
                .exchange_rate_service
                .get_cached_rate(settings.currency);
            if cached.is_some() {
                return IpcResult::ok(GetExchangeRateResponse {
                    schema_version: SCHEMA_VERSION,
                    rate: cached,
                    is_stale: true,
                    fiat_enabled: true,
                    refresh_cooldown_secs: state.exchange_rate_service.refresh_cooldown_secs()
                        as u32,
                });
            }
            IpcResult::err(
                errors::EXCHANGE_RATE_FETCH_FAILED,
                format!("Failed to fetch exchange rate: {e}"),
            )
        }
    }
}
