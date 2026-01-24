//! Exchange rate and fiat settings command handlers.

use zstash_core::domain::FiatDisplaySettings;
use zstash_core::errors;
use zstash_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetExchangeRateResponse, GetFiatSettingsRequest,
    GetFiatSettingsResponse, SetFiatSettingsRequest, SetFiatSettingsResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::{map_anyhow, system_time_to_unix_ms};

pub fn get_fiat_settings_impl(
    state: &AppState,
    request: GetFiatSettingsRequest,
) -> IpcResult<GetFiatSettingsResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let settings = zstash_engine::db::fiat_meta::get_fiat_settings(mgr.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(GetFiatSettingsResponse {
            schema_version: SCHEMA_VERSION,
            settings,
        })
    })
}

pub fn set_fiat_settings_impl(
    state: &AppState,
    request: SetFiatSettingsRequest,
) -> IpcResult<SetFiatSettingsResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        if request.enabled && !request.privacy_acknowledged {
            return Err(zstash_engine::error::ipc_err(
                errors::EXCHANGE_RATE_PRIVACY_ACK_REQUIRED,
                "Privacy acknowledgement required to enable fiat display",
            ));
        }

        let settings = FiatDisplaySettings {
            enabled: request.enabled,
            currency: request.currency,
            privacy_acknowledged: request.privacy_acknowledged,
        };

        let updated_at_ms = system_time_to_unix_ms(std::time::SystemTime::now()).map_err(|e| {
            zstash_engine::error::ipc_err(errors::INTERNAL_ERROR, format!("time error: {e}"))
        })?;

        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::fiat_meta::upsert_fiat_settings(
            mgr.app_db().conn(),
            &settings,
            updated_at_ms,
        )
        .map_err(|e| anyhow::anyhow!(e))?;

        Ok(SetFiatSettingsResponse {
            schema_version: SCHEMA_VERSION,
            settings,
        })
    })
}

pub async fn get_exchange_rate_impl(
    state: &AppState,
    request: GetExchangeRateRequest,
) -> IpcResult<GetExchangeRateResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let settings = {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        match zstash_engine::db::fiat_meta::get_fiat_settings(mgr.app_db().conn()) {
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
        Err(zstash_network::exchange_rate::ExchangeRateError::RateLimited(secs)) => {
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
