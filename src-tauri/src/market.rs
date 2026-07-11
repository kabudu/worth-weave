use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;

use crate::db;
use crate::error::{LedgerlyError, Result};
use crate::models::{
    AllocationReport, AllocationSlice, FxRate, PortfolioSnapshot, PriceQuote, SetFxRateInput,
    SetPriceInput, ValuationSummary, ValuedHolding,
};
use crate::projections;
use std::collections::BTreeMap;
use uuid::Uuid;

fn parse_positive(value: &str, field: &str) -> Result<Decimal> {
    Decimal::from_str(value.trim())
        .ok()
        .filter(|value| *value > Decimal::ZERO)
        .ok_or_else(|| {
            LedgerlyError::InvalidMarketData(format!("{field} must be a positive decimal"))
        })
}

fn parts(value: Decimal) -> (String, u32) {
    let value = value.normalize();
    (value.mantissa().to_string(), value.scale())
}

fn from_parts(coefficient: String, scale: u32) -> Result<Decimal> {
    let coefficient = coefficient
        .parse::<i128>()
        .map_err(|_| LedgerlyError::InvalidMarketData("stored decimal is invalid".into()))?;
    Ok(Decimal::from_i128_with_scale(coefficient, scale))
}

fn stale(as_of: &str, max_age_hours: i64) -> bool {
    DateTime::parse_from_rfc3339(as_of)
        .map(|value| {
            Utc::now()
                .signed_duration_since(value.with_timezone(&Utc))
                .num_hours()
                > max_age_hours
        })
        .unwrap_or(true)
}

fn validate_currency(currency: &str) -> Result<String> {
    let currency = currency.trim().to_uppercase();
    if db::CURRENCIES
        .iter()
        .any(|candidate| candidate.code == currency)
    {
        Ok(currency)
    } else {
        Err(LedgerlyError::InvalidMarketData(
            "unsupported currency".into(),
        ))
    }
}

pub fn set_price(connection: &Connection, input: &SetPriceInput) -> Result<PriceQuote> {
    let instrument_id = input.instrument_id.trim();
    if instrument_id.is_empty() || instrument_id.chars().count() > 128 {
        return Err(LedgerlyError::InvalidMarketData(
            "instrument identifier is invalid".into(),
        ));
    }
    let currency = validate_currency(&input.currency)?;
    let price = parse_positive(&input.price, "price")?;
    let (coefficient, scale) = parts(price);
    let as_of = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO market_prices (instrument_id, price_coefficient, price_scale, currency, as_of, source)
         VALUES (?1, ?2, ?3, ?4, ?5, 'manual')
         ON CONFLICT(instrument_id) DO UPDATE SET price_coefficient=excluded.price_coefficient,
         price_scale=excluded.price_scale, currency=excluded.currency, as_of=excluded.as_of, source=excluded.source",
        params![instrument_id, coefficient, scale, currency, as_of],
    )?;
    Ok(PriceQuote {
        instrument_id: instrument_id.into(),
        price: price.to_string(),
        currency,
        as_of,
        source: "manual".into(),
        stale: false,
    })
}

pub fn set_fx_rate(connection: &Connection, input: &SetFxRateInput) -> Result<FxRate> {
    let base_currency = validate_currency(&input.base_currency)?;
    let quote_currency = validate_currency(&input.quote_currency)?;
    if base_currency == quote_currency {
        return Err(LedgerlyError::InvalidMarketData(
            "FX currencies must differ".into(),
        ));
    }
    let rate = parse_positive(&input.rate, "FX rate")?;
    let (coefficient, scale) = parts(rate);
    let as_of = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO fx_rates (base_currency, quote_currency, rate_coefficient, rate_scale, as_of, source)
         VALUES (?1, ?2, ?3, ?4, ?5, 'manual')
         ON CONFLICT(base_currency, quote_currency) DO UPDATE SET rate_coefficient=excluded.rate_coefficient,
         rate_scale=excluded.rate_scale, as_of=excluded.as_of, source=excluded.source",
        params![base_currency, quote_currency, coefficient, scale, as_of],
    )?;
    Ok(FxRate {
        base_currency,
        quote_currency,
        rate: rate.to_string(),
        as_of,
        source: "manual".into(),
    })
}

fn price(connection: &Connection, instrument_id: &str) -> Result<Option<(PriceQuote, Decimal)>> {
    connection.query_row(
        "SELECT price_coefficient, price_scale, currency, as_of, source FROM market_prices WHERE instrument_id = ?1",
        [instrument_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?)),
    ).optional()?.map(|(coefficient, scale, currency, as_of, source)| {
        let value = from_parts(coefficient, scale)?;
        let is_stale = stale(&as_of, 36);
        Ok((PriceQuote { instrument_id: instrument_id.into(), price: value.to_string(), currency, as_of, source, stale: is_stale }, value))
    }).transpose()
}

fn fx(connection: &Connection, base: &str, quote: &str) -> Result<Option<(Decimal, bool)>> {
    if base == quote {
        return Ok(Some((Decimal::ONE, false)));
    }
    let direct: Option<(String, u32, String)> = connection.query_row(
        "SELECT rate_coefficient, rate_scale, as_of FROM fx_rates WHERE base_currency=?1 AND quote_currency=?2",
        params![base, quote], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional()?;
    if let Some((coefficient, scale, as_of)) = direct {
        return Ok(Some((from_parts(coefficient, scale)?, stale(&as_of, 48))));
    }
    let inverse: Option<(String, u32, String)> = connection.query_row(
        "SELECT rate_coefficient, rate_scale, as_of FROM fx_rates WHERE base_currency=?1 AND quote_currency=?2",
        params![quote, base], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional()?;
    inverse
        .map(|(coefficient, scale, as_of)| {
            from_parts(coefficient, scale).map(|rate| (Decimal::ONE / rate, stale(&as_of, 48)))
        })
        .transpose()
}

pub fn valuation(connection: &Connection) -> Result<ValuationSummary> {
    let reporting_currency = db::settings(connection)?
        .reporting_currency
        .unwrap_or_else(|| "GBP".into());
    let mut total = Decimal::ZERO;
    let mut missing_price_count = 0;
    let mut missing_fx_count = 0;
    let mut stale_price_count = 0;
    let mut stale_fx_count = 0;
    let mut total_gain = Decimal::ZERO;
    let mut gain_complete = true;
    let mut valued = Vec::new();
    for holding in projections::holdings(connection)? {
        let quantity = Decimal::from_str(&holding.quantity)
            .map_err(|_| LedgerlyError::InvalidMarketData("holding quantity is invalid".into()))?;
        let quote = price(connection, &holding.instrument_id)?;
        let (price_quote, market_value, reporting_value) = if let Some((price_quote, price)) = quote
        {
            if price_quote.stale {
                stale_price_count += 1;
            }
            let market_value = quantity * price;
            if let Some((rate, is_stale)) =
                fx(connection, &price_quote.currency, &reporting_currency)?
            {
                if is_stale {
                    stale_fx_count += 1;
                }
                let reporting_value = market_value * rate;
                total += reporting_value;
                (
                    Some(price_quote),
                    Some(market_value.normalize().to_string()),
                    Some(reporting_value.normalize().to_string()),
                )
            } else {
                missing_fx_count += 1;
                (
                    Some(price_quote),
                    Some(market_value.normalize().to_string()),
                    None,
                )
            }
        } else {
            missing_price_count += 1;
            (None, None, None)
        };
        let reporting_cost_basis = if holding.cost_basis_complete {
            match (&holding.cost_basis, &holding.currency) {
                (Some(cost), Some(currency)) => {
                    let cost = Decimal::from_str(cost).map_err(|_| {
                        LedgerlyError::InvalidMarketData("holding cost basis is invalid".into())
                    })?;
                    if let Some((rate, _)) = fx(connection, currency, &reporting_currency)? {
                        Some((cost * rate).normalize().to_string())
                    } else {
                        gain_complete = false;
                        None
                    }
                }
                _ => {
                    gain_complete = false;
                    None
                }
            }
        } else {
            gain_complete = false;
            None
        };
        let gain_loss = match (&reporting_value, &reporting_cost_basis) {
            (Some(value), Some(cost)) => {
                let gain = Decimal::from_str(value).unwrap_or_default()
                    - Decimal::from_str(cost).unwrap_or_default();
                total_gain += gain;
                Some(gain.normalize().to_string())
            }
            _ => {
                gain_complete = false;
                None
            }
        };
        valued.push(ValuedHolding {
            holding,
            price: price_quote,
            market_value,
            reporting_value,
            reporting_currency: reporting_currency.clone(),
            reporting_cost_basis,
            gain_loss,
        });
    }
    let total_value =
        (missing_price_count == 0 && missing_fx_count == 0).then(|| total.normalize().to_string());
    Ok(ValuationSummary {
        reporting_currency,
        total_value,
        missing_price_count,
        missing_fx_count,
        stale_price_count,
        stale_fx_count,
        total_gain_loss: gain_complete.then(|| total_gain.normalize().to_string()),
        holdings: valued,
    })
}

pub fn capture_snapshot(connection: &Connection) -> Result<PortfolioSnapshot> {
    let valuation = valuation(connection)?;
    let total = valuation.total_value.ok_or_else(|| {
        LedgerlyError::InvalidMarketData(
            "a snapshot requires prices and FX rates for every open holding".into(),
        )
    })?;
    let total_decimal = Decimal::from_str(&total)
        .map_err(|_| LedgerlyError::InvalidMarketData("valuation total is invalid".into()))?;
    let (coefficient, scale) = parts(total_decimal);
    let snapshot = PortfolioSnapshot {
        id: Uuid::new_v4().to_string(),
        captured_at: Utc::now().to_rfc3339(),
        reporting_currency: valuation.reporting_currency,
        total_value: total,
    };
    connection.execute(
        "INSERT INTO portfolio_snapshots (id, captured_at, reporting_currency, total_coefficient, total_scale)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![snapshot.id, snapshot.captured_at, snapshot.reporting_currency, coefficient, scale],
    )?;
    Ok(snapshot)
}

pub fn snapshots(connection: &Connection) -> Result<Vec<PortfolioSnapshot>> {
    let mut statement = connection.prepare(
        "SELECT id, captured_at, reporting_currency, total_coefficient, total_scale
         FROM portfolio_snapshots ORDER BY captured_at, id",
    )?;
    let rows = statement.query_map([], |row| {
        let coefficient: String = row.get(3)?;
        let scale: u32 = row.get(4)?;
        let total_value = coefficient
            .parse::<i128>()
            .map(|value| {
                Decimal::from_i128_with_scale(value, scale)
                    .normalize()
                    .to_string()
            })
            .unwrap_or_default();
        Ok(PortfolioSnapshot {
            id: row.get(0)?,
            captured_at: row.get(1)?,
            reporting_currency: row.get(2)?,
            total_value,
        })
    })?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

pub fn allocation(connection: &Connection) -> Result<AllocationReport> {
    let valuation = valuation(connection)?;
    let total = valuation
        .total_value
        .as_deref()
        .ok_or_else(|| {
            LedgerlyError::InvalidMarketData(
                "allocation requires a complete portfolio valuation".into(),
            )
        })
        .and_then(|value| {
            Decimal::from_str(value)
                .map_err(|_| LedgerlyError::InvalidMarketData("valuation total is invalid".into()))
        })?;
    if total <= Decimal::ZERO {
        return Err(LedgerlyError::InvalidMarketData(
            "allocation requires a positive portfolio value".into(),
        ));
    }
    let mut accounts: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut currencies: BTreeMap<String, Decimal> = BTreeMap::new();
    for item in valuation.holdings {
        let value = Decimal::from_str(item.reporting_value.as_deref().unwrap_or("0"))
            .map_err(|_| LedgerlyError::InvalidMarketData("holding valuation is invalid".into()))?;
        *accounts.entry(item.holding.account_name).or_default() += value;
        if let Some(price) = item.price {
            *currencies.entry(price.currency).or_default() += value;
        }
    }
    let slices = |values: BTreeMap<String, Decimal>| {
        values
            .into_iter()
            .map(|(label, value)| AllocationSlice {
                label,
                value: value.normalize().to_string(),
                percentage: ((value / total) * Decimal::ONE_HUNDRED)
                    .round_dp(2)
                    .normalize()
                    .to_string(),
            })
            .collect()
    };
    Ok(AllocationReport {
        reporting_currency: valuation.reporting_currency,
        by_account: slices(accounts),
        by_currency: slices(currencies),
    })
}
