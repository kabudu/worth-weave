use std::str::FromStr;

use chrono::Utc;
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
        Ok((PriceQuote { instrument_id: instrument_id.into(), price: value.to_string(), currency, as_of, source }, value))
    }).transpose()
}

fn fx(connection: &Connection, base: &str, quote: &str) -> Result<Option<Decimal>> {
    if base == quote {
        return Ok(Some(Decimal::ONE));
    }
    let direct: Option<(String, u32)> = connection.query_row(
        "SELECT rate_coefficient, rate_scale FROM fx_rates WHERE base_currency=?1 AND quote_currency=?2",
        params![base, quote], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    if let Some((coefficient, scale)) = direct {
        return Ok(Some(from_parts(coefficient, scale)?));
    }
    let inverse: Option<(String, u32)> = connection.query_row(
        "SELECT rate_coefficient, rate_scale FROM fx_rates WHERE base_currency=?1 AND quote_currency=?2",
        params![quote, base], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    inverse
        .map(|(coefficient, scale)| from_parts(coefficient, scale).map(|rate| Decimal::ONE / rate))
        .transpose()
}

pub fn valuation(connection: &Connection) -> Result<ValuationSummary> {
    let reporting_currency = db::settings(connection)?
        .reporting_currency
        .unwrap_or_else(|| "GBP".into());
    let mut total = Decimal::ZERO;
    let mut missing_price_count = 0;
    let mut missing_fx_count = 0;
    let mut valued = Vec::new();
    for holding in projections::holdings(connection)? {
        let quantity = Decimal::from_str(&holding.quantity)
            .map_err(|_| LedgerlyError::InvalidMarketData("holding quantity is invalid".into()))?;
        let quote = price(connection, &holding.instrument_id)?;
        let (price_quote, market_value, reporting_value) = if let Some((price_quote, price)) = quote
        {
            let market_value = quantity * price;
            if let Some(rate) = fx(connection, &price_quote.currency, &reporting_currency)? {
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
        valued.push(ValuedHolding {
            holding,
            price: price_quote,
            market_value,
            reporting_value,
            reporting_currency: reporting_currency.clone(),
        });
    }
    let total_value =
        (missing_price_count == 0 && missing_fx_count == 0).then(|| total.normalize().to_string());
    Ok(ValuationSummary {
        reporting_currency,
        total_value,
        missing_price_count,
        missing_fx_count,
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
