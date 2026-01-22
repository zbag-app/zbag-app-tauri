use serde::{Deserialize, Serialize};

/// Supported fiat currencies for exchange rate conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FiatCurrency {
    #[default]
    USD,
    EUR,
    GBP,
    CHF,
    CAD,
    AUD,
    JPY,
}

impl FiatCurrency {
    /// Returns all supported fiat currencies.
    pub fn all() -> &'static [FiatCurrency] {
        &[
            FiatCurrency::USD,
            FiatCurrency::EUR,
            FiatCurrency::GBP,
            FiatCurrency::CHF,
            FiatCurrency::CAD,
            FiatCurrency::AUD,
            FiatCurrency::JPY,
        ]
    }

    /// Returns the currency code as a string (e.g., "USD").
    pub fn code(&self) -> &'static str {
        match self {
            FiatCurrency::USD => "USD",
            FiatCurrency::EUR => "EUR",
            FiatCurrency::GBP => "GBP",
            FiatCurrency::CHF => "CHF",
            FiatCurrency::CAD => "CAD",
            FiatCurrency::AUD => "AUD",
            FiatCurrency::JPY => "JPY",
        }
    }

    /// Returns the currency symbol for display.
    pub fn symbol(&self) -> &'static str {
        match self {
            FiatCurrency::USD => "$",
            FiatCurrency::EUR => "\u{20AC}",
            FiatCurrency::GBP => "\u{00A3}",
            FiatCurrency::CHF => "CHF",
            FiatCurrency::CAD => "C$",
            FiatCurrency::AUD => "A$",
            FiatCurrency::JPY => "\u{00A5}",
        }
    }
}

/// Exchange rate information for ZEC to fiat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeRate {
    /// The fiat currency for this rate.
    pub currency: FiatCurrency,
    /// Price of 1 ZEC in the fiat currency (e.g., 25.50 means 1 ZEC = $25.50).
    pub price: f64,
    /// When this rate was fetched (Unix timestamp in milliseconds).
    pub fetched_at_ms: i64,
}

impl ExchangeRate {
    /// Returns true if the rate is stale (older than 15 minutes).
    ///
    /// This uses wall clock time (`fetched_at_ms`) for staleness checks.
    /// It is suitable for contexts where only the `ExchangeRate` struct is
    /// available without access to the `ExchangeRateService`.
    ///
    /// Note: The `ExchangeRateService::is_cache_stale()` method uses monotonic
    /// time (`Instant`) and should be preferred when the service is available,
    /// as monotonic time is immune to system clock adjustments.
    pub fn is_stale(&self) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let age_ms = now_ms - self.fetched_at_ms;
        age_ms > 15 * 60 * 1000 // 15 minutes
    }
}

/// User settings for fiat display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiatDisplaySettings {
    /// Whether fiat display is enabled (default: false).
    pub enabled: bool,
    /// The selected fiat currency (default: USD).
    pub currency: FiatCurrency,
    /// Whether the user has acknowledged the privacy warning.
    pub privacy_acknowledged: bool,
}

impl Default for FiatDisplaySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            currency: FiatCurrency::USD,
            privacy_acknowledged: false,
        }
    }
}
