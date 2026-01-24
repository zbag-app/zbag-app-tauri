use tauri::State;

use zstash_core::errors;
use zstash_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetExchangeRateResponse, GetFiatSettingsRequest,
    GetFiatSettingsResponse, SetFiatSettingsRequest, SetFiatSettingsResponse,
};
use zstash_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zstash_engine::db::fiat_meta;
use zstash_engine::error::ipc_err;

use crate::state::AppState;

use super::util::{map_anyhow, system_time_to_unix_ms};

#[tauri::command(rename = "zstash_get_fiat_settings")]
pub fn zstash_get_fiat_settings(
    state: State<'_, AppState>,
    request: GetFiatSettingsRequest,
) -> IpcResult<GetFiatSettingsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let settings =
            fiat_meta::get_fiat_settings(mgr.app_db().conn()).map_err(|e| anyhow::anyhow!(e))?;

        Ok(GetFiatSettingsResponse {
            schema_version: SCHEMA_VERSION,
            settings,
        })
    })
}

#[tauri::command(rename = "zstash_set_fiat_settings")]
pub fn zstash_set_fiat_settings(
    state: State<'_, AppState>,
    request: SetFiatSettingsRequest,
) -> IpcResult<SetFiatSettingsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        // If enabling fiat display, user must acknowledge privacy implications
        if request.enabled && !request.privacy_acknowledged {
            return Err(ipc_err(
                errors::EXCHANGE_RATE_PRIVACY_ACK_REQUIRED,
                "Privacy acknowledgement required to enable fiat display",
            ));
        }

        let settings = zstash_core::domain::FiatDisplaySettings {
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
    })
}

#[tauri::command(rename = "zstash_get_exchange_rate")]
pub async fn zstash_get_exchange_rate(
    state: State<'_, AppState>,
    request: GetExchangeRateRequest,
) -> Result<IpcResult<GetExchangeRateResponse>, ()> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return Ok(IpcResult::Err { err });
    }

    // Get fiat settings
    let settings = {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        match fiat_meta::get_fiat_settings(mgr.app_db().conn()) {
            Ok(s) => s,
            Err(e) => {
                return Ok(IpcResult::err(
                    errors::INTERNAL_ERROR,
                    format!("Failed to get fiat settings: {e}"),
                ));
            }
        }
    };

    // If fiat display is disabled, return early
    if !settings.enabled {
        return Ok(IpcResult::ok(GetExchangeRateResponse {
            schema_version: SCHEMA_VERSION,
            rate: None,
            is_stale: true,
            fiat_enabled: false,
            refresh_cooldown_secs: 0,
        }));
    }

    let cooldown = state.exchange_rate_service.refresh_cooldown_secs() as u32;

    // Try to get cached rate first
    if let Some(rate) = state
        .exchange_rate_service
        .get_cached_rate(settings.currency)
        && !rate.is_stale()
        && !request.force_refresh
    {
        return Ok(IpcResult::ok(GetExchangeRateResponse {
            schema_version: SCHEMA_VERSION,
            rate: Some(rate),
            is_stale: false,
            fiat_enabled: true,
            refresh_cooldown_secs: cooldown,
        }));
    }

    // Fetch fresh rate
    match state
        .exchange_rate_service
        .get_rate(settings.currency, request.force_refresh)
        .await
    {
        Ok(rate) => {
            let is_stale = rate.is_stale();
            Ok(IpcResult::ok(GetExchangeRateResponse {
                schema_version: SCHEMA_VERSION,
                rate: Some(rate),
                is_stale,
                fiat_enabled: true,
                refresh_cooldown_secs: state.exchange_rate_service.refresh_cooldown_secs() as u32,
            }))
        }
        Err(zstash_network::exchange_rate::ExchangeRateError::RateLimited(secs)) => {
            // Return cached rate if available
            let cached = state
                .exchange_rate_service
                .get_cached_rate(settings.currency);
            let is_stale = cached.as_ref().is_some_and(|r| r.is_stale());
            Ok(IpcResult::ok(GetExchangeRateResponse {
                schema_version: SCHEMA_VERSION,
                rate: cached,
                is_stale,
                fiat_enabled: true,
                refresh_cooldown_secs: secs as u32,
            }))
        }
        Err(e) => {
            // Return cached rate if available, with error indicated by is_stale
            let cached = state
                .exchange_rate_service
                .get_cached_rate(settings.currency);
            if cached.is_some() {
                return Ok(IpcResult::ok(GetExchangeRateResponse {
                    schema_version: SCHEMA_VERSION,
                    rate: cached,
                    is_stale: true,
                    fiat_enabled: true,
                    refresh_cooldown_secs: state.exchange_rate_service.refresh_cooldown_secs()
                        as u32,
                }));
            }
            Ok(IpcResult::err(
                errors::EXCHANGE_RATE_FETCH_FAILED,
                format!("Failed to fetch exchange rate: {e}"),
            ))
        }
    }
}
