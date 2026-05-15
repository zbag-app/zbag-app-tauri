use rusqlite::{Connection, params};

use bagz_core::domain::{FiatCurrency, FiatDisplaySettings};

pub fn get_fiat_settings(conn: &Connection) -> rusqlite::Result<FiatDisplaySettings> {
    let mut stmt = conn.prepare(
        "SELECT enabled, currency, privacy_acknowledged FROM fiat_settings WHERE id = 1",
    )?;
    let mut rows = stmt.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(FiatDisplaySettings::default());
    };

    let enabled: i64 = row.get(0)?;
    let currency: String = row.get(1)?;
    let privacy_acknowledged: i64 = row.get(2)?;

    Ok(FiatDisplaySettings {
        enabled: enabled != 0,
        currency: parse_fiat_currency(&currency),
        privacy_acknowledged: privacy_acknowledged != 0,
    })
}

pub fn upsert_fiat_settings(
    conn: &Connection,
    settings: &FiatDisplaySettings,
    updated_at_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO fiat_settings (id, enabled, currency, privacy_acknowledged, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
           enabled = excluded.enabled,
           currency = excluded.currency,
           privacy_acknowledged = excluded.privacy_acknowledged,
           updated_at = excluded.updated_at",
        params![
            settings.enabled as i64,
            settings.currency.code(),
            settings.privacy_acknowledged as i64,
            updated_at_ms,
        ],
    )?;
    Ok(())
}

fn parse_fiat_currency(s: &str) -> FiatCurrency {
    match s {
        "USD" => FiatCurrency::USD,
        "EUR" => FiatCurrency::EUR,
        "GBP" => FiatCurrency::GBP,
        "CHF" => FiatCurrency::CHF,
        "CAD" => FiatCurrency::CAD,
        "AUD" => FiatCurrency::AUD,
        "JPY" => FiatCurrency::JPY,
        _ => FiatCurrency::USD,
    }
}
