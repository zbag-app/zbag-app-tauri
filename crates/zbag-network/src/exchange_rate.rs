//! Exchange rate fetching service for ZEC to fiat conversion.
//!
//! This module provides functionality to fetch ZEC exchange rates from CoinGecko.
//! The service respects Tor settings and fails closed when Tor is enabled but not ready.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Deserialize;
use thiserror::Error;

use zbag_core::domain::{ExchangeRate, FiatCurrency};

use crate::http_client::{HttpClient, HttpClientError};
use crate::transport::TransportSelector;

/// Minimum interval between rate fetches (120 seconds, matching Zashi).
const RATE_LIMIT_SECS: u64 = 120;

/// Rate is considered stale after 15 minutes.
const STALE_THRESHOLD_SECS: u64 = 15 * 60;

#[derive(Debug, Error)]
pub enum ExchangeRateError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] HttpClientError),
    #[error("Failed to parse exchange rate response: {0}")]
    ParseError(String),
    #[error("Rate limited: please wait {0} seconds")]
    RateLimited(u64),
    #[error("Exchange rate not available")]
    NotAvailable,
}

/// CoinGecko simple price response structure.
#[derive(Debug, Deserialize)]
struct CoinGeckoResponse {
    zcash: Option<CoinGeckoPrices>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoPrices {
    usd: Option<f64>,
    eur: Option<f64>,
    gbp: Option<f64>,
    chf: Option<f64>,
    cad: Option<f64>,
    aud: Option<f64>,
    jpy: Option<f64>,
}

/// Cached exchange rate with fetch timestamp.
#[derive(Debug, Clone)]
struct CachedRate {
    rates: Vec<ExchangeRate>,
    fetched_at: Instant,
}

/// Exchange rate service that fetches and caches ZEC exchange rates.
#[derive(Clone)]
pub struct ExchangeRateService {
    client: HttpClient,
    cache: Arc<Mutex<Option<CachedRate>>>,
    last_fetch: Arc<Mutex<Option<Instant>>>,
}

impl ExchangeRateService {
    /// Create a new exchange rate service with direct HTTP transport.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: HttpClient::new()?,
            cache: Arc::new(Mutex::new(None)),
            last_fetch: Arc::new(Mutex::new(None)),
        })
    }

    /// Create a new exchange rate service with Tor support.
    pub fn new_with_tor(tor: Arc<zbag_tor::TorManager>) -> anyhow::Result<Self> {
        Ok(Self {
            client: HttpClient::new_with_tor(tor)?,
            cache: Arc::new(Mutex::new(None)),
            last_fetch: Arc::new(Mutex::new(None)),
        })
    }

    /// Create a new exchange rate service with a custom transport selector.
    pub fn new_with_transport(transport: TransportSelector) -> anyhow::Result<Self> {
        Ok(Self {
            client: HttpClient::new_with_transport(transport)?,
            cache: Arc::new(Mutex::new(None)),
            last_fetch: Arc::new(Mutex::new(None)),
        })
    }

    /// Get the cached exchange rate for a specific currency, if available and not stale.
    pub fn get_cached_rate(&self, currency: FiatCurrency) -> Option<ExchangeRate> {
        let cache = self.cache.lock();
        cache
            .as_ref()
            .and_then(|c| c.rates.iter().find(|r| r.currency == currency).cloned())
    }

    /// Check if any cached rate is stale.
    ///
    /// This uses monotonic time (`Instant`) and is the authoritative staleness
    /// check when the service is available. Monotonic time is immune to system
    /// clock adjustments, unlike the wall clock check in `ExchangeRate::is_stale()`.
    pub fn is_cache_stale(&self) -> bool {
        let cache = self.cache.lock();
        match cache.as_ref() {
            Some(c) => c.fetched_at.elapsed() > Duration::from_secs(STALE_THRESHOLD_SECS),
            None => true,
        }
    }

    /// Get seconds until next refresh is allowed (0 if allowed now).
    pub fn refresh_cooldown_secs(&self) -> u64 {
        let last_fetch = self.last_fetch.lock();
        match last_fetch.as_ref() {
            Some(t) => {
                let elapsed = t.elapsed().as_secs();
                RATE_LIMIT_SECS.saturating_sub(elapsed)
            }
            None => 0,
        }
    }

    /// Fetch exchange rates from CoinGecko.
    ///
    /// This method respects rate limiting and will return a cached value if
    /// the rate limit has not elapsed, unless `force` is true.
    ///
    /// # Force refresh behavior
    ///
    /// When `force` is true:
    /// - **Bypasses client-side rate limiting**: The 120-second cooldown between
    ///   requests is skipped.
    /// - **Does NOT update `last_fetch`**: This prevents forced refreshes from
    ///   resetting the normal refresh timer, so subsequent normal requests are
    ///   not penalized.
    /// - **Still subject to CoinGecko's server-side rate limiting**: The API may
    ///   return HTTP 429 responses if too many requests are made.
    pub async fn fetch_rates(&self, force: bool) -> Result<Vec<ExchangeRate>, ExchangeRateError> {
        // Check rate limiting
        let cooldown = self.refresh_cooldown_secs();
        if cooldown > 0 && !force {
            // Return cached rates if available
            let cache = self.cache.lock();
            if let Some(cached) = cache.as_ref() {
                return Ok(cached.rates.clone());
            }
            return Err(ExchangeRateError::RateLimited(cooldown));
        }

        // Fetch from CoinGecko
        let url = reqwest::Url::parse(
            "https://api.coingecko.com/api/v3/simple/price?ids=zcash&vs_currencies=usd,eur,gbp,chf,cad,aud,jpy",
        )
        .map_err(|e| ExchangeRateError::ParseError(e.to_string()))?;

        // Update last fetch time only for non-forced requests.
        //
        // IMPORTANT: We deliberately do this only after the URL is parsed so local
        // failures don't trigger the cooldown.
        //
        // If transport selection fails closed (e.g. Tor is enabled but not ready),
        // we restore the previous timestamp so callers can retry immediately once
        // Tor becomes available.
        let (attempted_at, prev_last_fetch) = if force {
            (None, None)
        } else {
            let attempted_at = Instant::now();
            let mut last_fetch = self.last_fetch.lock();
            let prev = *last_fetch;
            *last_fetch = Some(attempted_at);
            (Some(attempted_at), prev)
        };

        let response = match self.client.get_json(url).await {
            Ok(res) => res,
            Err(err) => {
                // A fail-closed error means we never even attempted a network request.
                // Do not rate-limit retries in this case.
                if let Some(attempted_at) = attempted_at
                    && matches!(err, HttpClientError::FailClosed(_))
                {
                    let mut last_fetch = self.last_fetch.lock();
                    if *last_fetch == Some(attempted_at) {
                        *last_fetch = prev_last_fetch;
                    }
                }
                return Err(err.into());
            }
        };

        if response.status == 429 {
            let retry_secs = response.retry_after.map(|d| d.as_secs()).unwrap_or(60);
            return Err(ExchangeRateError::RateLimited(retry_secs));
        }

        if response.status != 200 {
            return Err(ExchangeRateError::ParseError(format!(
                "API returned status {}",
                response.status
            )));
        }

        let data: CoinGeckoResponse = serde_json::from_value(response.body)
            .map_err(|e| ExchangeRateError::ParseError(e.to_string()))?;

        let prices = data.zcash.ok_or(ExchangeRateError::NotAvailable)?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let mut rates = Vec::new();

        // Helper to push a rate if the price is present
        let mut push_rate = |currency: FiatCurrency, price_opt: Option<f64>| {
            if let Some(price) = price_opt {
                rates.push(ExchangeRate {
                    currency,
                    price,
                    fetched_at_ms: now_ms,
                });
            }
        };

        push_rate(FiatCurrency::USD, prices.usd);
        push_rate(FiatCurrency::EUR, prices.eur);
        push_rate(FiatCurrency::GBP, prices.gbp);
        push_rate(FiatCurrency::CHF, prices.chf);
        push_rate(FiatCurrency::CAD, prices.cad);
        push_rate(FiatCurrency::AUD, prices.aud);
        push_rate(FiatCurrency::JPY, prices.jpy);

        // Update cache
        {
            let mut cache = self.cache.lock();
            *cache = Some(CachedRate {
                rates: rates.clone(),
                fetched_at: Instant::now(),
            });
        }

        Ok(rates)
    }

    /// Get the exchange rate for a specific currency, fetching if necessary.
    pub async fn get_rate(
        &self,
        currency: FiatCurrency,
        force_refresh: bool,
    ) -> Result<ExchangeRate, ExchangeRateError> {
        // Try cached rate first if not forcing refresh.
        // Use is_cache_stale() which relies on monotonic time (Instant) rather than
        // rate.is_stale() which uses wall-clock time that can be affected by clock adjustments.
        if !force_refresh
            && !self.is_cache_stale()
            && let Some(rate) = self.get_cached_rate(currency)
        {
            return Ok(rate);
        }

        // Fetch fresh rates
        let rates = self.fetch_rates(force_refresh).await?;

        rates
            .into_iter()
            .find(|r| r.currency == currency)
            .ok_or(ExchangeRateError::NotAvailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbag_core::domain::{TorState, TorStatus};

    #[test]
    fn test_fiat_currency_code() {
        assert_eq!(FiatCurrency::USD.code(), "USD");
        assert_eq!(FiatCurrency::EUR.code(), "EUR");
        assert_eq!(FiatCurrency::JPY.code(), "JPY");
    }

    #[test]
    fn test_fiat_currency_symbol() {
        assert_eq!(FiatCurrency::USD.symbol(), "$");
        assert_eq!(FiatCurrency::EUR.symbol(), "\u{20AC}");
        assert_eq!(FiatCurrency::JPY.symbol(), "\u{00A5}");
    }

    #[test]
    fn test_exchange_rate_stale() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let fresh_rate = ExchangeRate {
            currency: FiatCurrency::USD,
            price: 25.0,
            fetched_at_ms: now_ms,
        };
        assert!(!fresh_rate.is_stale());

        let stale_rate = ExchangeRate {
            currency: FiatCurrency::USD,
            price: 25.0,
            fetched_at_ms: now_ms - (16 * 60 * 1000), // 16 minutes ago
        };
        assert!(stale_rate.is_stale());
    }

    #[tokio::test]
    async fn tor_not_ready_does_not_trigger_rate_limit_cooldown() {
        // Tor enabled but not ready should fail closed. That is a local condition
        // (no request is made), so it must not set the rate-limit timer.
        let tor_dir = std::env::temp_dir().join("zbag-tor-test");
        let tor = Arc::new(zbag_tor::TorManager::new(
            zbag_tor::TorManagerConfig::new(tor_dir),
            TorState {
                enabled: true,
                status: TorStatus::Connecting,
                last_error: None,
            },
        ));

        let service = ExchangeRateService::new_with_tor(tor).expect("service init");
        assert_eq!(service.refresh_cooldown_secs(), 0);

        let res = service.fetch_rates(false).await;
        assert!(res.is_err());
        assert_eq!(service.refresh_cooldown_secs(), 0);
    }
}
