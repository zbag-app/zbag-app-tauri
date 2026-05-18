use serde::{Deserialize, Serialize};

use crate::domain::{ExchangeRate, FiatCurrency, FiatDisplaySettings};

// ============================================================================
// Get Fiat Settings
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetFiatSettingsRequest {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetFiatSettingsResponse {
    pub schema_version: u32,
    pub settings: FiatDisplaySettings,
}

// ============================================================================
// Set Fiat Settings
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetFiatSettingsRequest {
    pub schema_version: u32,
    /// Enable or disable fiat display.
    pub enabled: bool,
    /// Selected fiat currency.
    pub currency: FiatCurrency,
    /// User acknowledges the privacy implications of fetching exchange rates.
    /// Required when enabling fiat display.
    pub privacy_acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetFiatSettingsResponse {
    pub schema_version: u32,
    pub settings: FiatDisplaySettings,
}

// ============================================================================
// Get Exchange Rate
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetExchangeRateRequest {
    pub schema_version: u32,
    /// Force refresh even if cached rate is not stale (subject to rate limiting).
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetExchangeRateResponse {
    pub schema_version: u32,
    /// The exchange rate, if available and fiat display is enabled.
    pub rate: Option<ExchangeRate>,
    /// Whether the rate is stale (older than 15 minutes).
    pub is_stale: bool,
    /// Whether fiat display is enabled.
    pub fiat_enabled: bool,
    /// Seconds until next refresh is allowed (0 if refresh is allowed now).
    pub refresh_cooldown_secs: u32,
}
