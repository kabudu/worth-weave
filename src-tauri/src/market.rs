use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{StreamExt, stream};
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;

use crate::db;
use crate::error::{Result, WorthweaveError};
use crate::models::{
    AllocationReport, AllocationSlice, FxRate, FxRefreshResult, PortfolioSnapshot, PriceQuote,
    SetFxRateInput, SetPriceInput, TotalReturnAttribution, ValuationSummary, ValuedHolding,
};
use crate::projections;
use serde_json::Value;
use uuid::Uuid;

const ECB_DAILY_RATES_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
const MAX_ECB_RESPONSE_BYTES: usize = 256 * 1024;
const MASSIVE_API_BASE: &str = "https://api.massive.com";
const MASSIVE_KEYCHAIN_SERVICE: &str = "app.worthweave.portfolio";
const MASSIVE_KEYCHAIN_ACCOUNT: &str = "massive-api-key";
const YAHOO_CHART_BASE: &str = "https://query1.finance.yahoo.com/v8/finance/chart";

#[derive(Debug, Clone)]
pub(crate) struct HistoryCandidate {
    instrument_id: String,
    yahoo_symbol: String,
}

#[derive(Debug, Clone)]
pub(crate) struct HistoryObservation {
    instrument_id: String,
    date: String,
    price: Decimal,
    currency: String,
}

pub fn history_candidates(connection: &Connection) -> Result<Vec<HistoryCandidate>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT i.id, i.symbol, i.isin
         FROM instruments i JOIN events e ON e.instrument_id=i.id
         WHERE i.symbol IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM historical_prices hp WHERE hp.instrument_id=i.id AND date(hp.fetched_at)=date('now'))
         ORDER BY i.symbol LIMIT 300",
    )?;
    let rows = statement.query_map([], |row| {
        let instrument_id: String = row.get(0)?;
        let symbol: String = row.get(1)?;
        let isin: Option<String> = row.get(2)?;
        let yahoo_symbol = if isin.as_deref().is_some_and(|value| value.starts_with("GB")) {
            format!("{symbol}.L")
        } else {
            symbol
        };
        Ok(HistoryCandidate {
            instrument_id,
            yahoo_symbol,
        })
    })?;
    Ok(rows
        .filter_map(std::result::Result::ok)
        .filter(|candidate| valid_massive_symbol(&candidate.yahoo_symbol))
        .collect())
}

async fn fetch_yahoo_candidate(
    client: &reqwest::Client,
    candidate: HistoryCandidate,
) -> Vec<HistoryObservation> {
    let url = format!(
        "{YAHOO_CHART_BASE}/{}?period1=0&period2={}&interval=1d&events=history",
        candidate.yahoo_symbol,
        Utc::now().timestamp()
    );
    let Ok(response) = client
        .get(url)
        .header("User-Agent", "Worthweave/0.1 portfolio-history")
        .send()
        .await
    else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(json) = response.json::<Value>().await else {
        return Vec::new();
    };
    let Some(result) = json.pointer("/chart/result/0") else {
        return Vec::new();
    };
    let timestamps = result
        .get("timestamp")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let closes = result
        .pointer("/indicators/quote/0/close")
        .or_else(|| result.pointer("/indicators/adjclose/0/adjclose"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut currency = result
        .pointer("/meta/currency")
        .and_then(Value::as_str)
        .unwrap_or("USD")
        .to_uppercase();
    let divisor = if currency == "GBP" && candidate.yahoo_symbol.ends_with(".L") {
        Decimal::from(100)
    } else {
        Decimal::ONE
    };
    if currency == "GBX" || currency == "GBPENCE" {
        currency = "GBP".into();
    }
    timestamps
        .into_iter()
        .zip(closes)
        .filter_map(|(timestamp, close)| {
            let timestamp = timestamp.as_i64()?;
            let price = Decimal::from_str(&close.as_f64()?.to_string()).ok()? / divisor;
            let date = DateTime::from_timestamp(timestamp, 0)?
                .format("%Y-%m-%d")
                .to_string();
            (price > Decimal::ZERO).then(|| HistoryObservation {
                instrument_id: candidate.instrument_id.clone(),
                date,
                price,
                currency: currency.clone(),
            })
        })
        .collect()
}

pub async fn fetch_historical_prices(
    candidates: Vec<HistoryCandidate>,
) -> Result<Vec<HistoryObservation>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(18))
        .connect_timeout(Duration::from_secs(6))
        .build()
        .map_err(|_| {
            WorthweaveError::InvalidMarketData("could not prepare historical market data".into())
        })?;
    let observations = stream::iter(candidates.into_iter().map(|candidate| {
        let client = client.clone();
        async move { fetch_yahoo_candidate(&client, candidate).await }
    }))
    .buffer_unordered(4)
    .collect::<Vec<_>>()
    .await;
    Ok(observations.into_iter().flatten().collect())
}

pub fn save_historical_prices(
    connection: &Connection,
    observations: Vec<HistoryObservation>,
    requested: usize,
) -> Result<crate::models::HistoryRefreshResult> {
    let available = observations
        .iter()
        .map(|item| item.instrument_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let mut updated = 0;
    let transaction = connection.unchecked_transaction()?;
    for item in observations {
        let (coefficient, scale) = parts(item.price);
        updated += transaction.execute(
            "INSERT INTO historical_prices (instrument_id, price_date, price_coefficient, price_scale, currency, source)
             VALUES (?1, ?2, ?3, ?4, ?5, 'yahoo_chart')
             ON CONFLICT(instrument_id, price_date) DO UPDATE SET price_coefficient=excluded.price_coefficient, price_scale=excluded.price_scale, currency=excluded.currency, source=excluded.source, fetched_at=CURRENT_TIMESTAMP",
            params![item.instrument_id, item.date, coefficient, scale, item.currency],
        )?;
    }
    transaction.commit()?;
    Ok(crate::models::HistoryRefreshResult {
        requested,
        updated,
        unavailable: requested.saturating_sub(available),
        source: "Yahoo Finance fallback",
    })
}

#[derive(Debug, Clone)]
pub(crate) struct MassiveCandidate {
    pub instrument_id: String,
    pub symbol: String,
}

#[derive(Debug, Clone)]
pub(crate) enum MassiveObservation {
    Price {
        candidate: MassiveCandidate,
        price: Decimal,
        as_of: String,
    },
    Delisted(String),
    ForeignInactiveMatch(String),
    NotFound(String),
    Unsupported(String),
    Failed(String),
}

fn massive_key_entry() -> Result<keyring::Entry> {
    keyring::Entry::new(MASSIVE_KEYCHAIN_SERVICE, MASSIVE_KEYCHAIN_ACCOUNT).map_err(|e| {
        WorthweaveError::InvalidMarketData(format!("could not access macOS Keychain: {e}"))
    })
}

pub fn massive_provider_status() -> Result<crate::models::MassiveProviderStatus> {
    let configured = match massive_key_entry()?.get_password() {
        Ok(value) => !value.trim().is_empty(),
        Err(keyring::Error::NoEntry) => false,
        Err(e) => {
            return Err(WorthweaveError::InvalidMarketData(format!(
                "could not read Massive API key from macOS Keychain: {e}"
            )));
        }
    };
    Ok(crate::models::MassiveProviderStatus { configured })
}

pub fn save_massive_api_key(api_key: &str) -> Result<crate::models::MassiveProviderStatus> {
    let api_key = api_key.trim();
    if api_key.len() < 16 || api_key.len() > 512 || api_key.chars().any(char::is_whitespace) {
        return Err(WorthweaveError::InvalidMarketData(
            "Massive API key is invalid".into(),
        ));
    }
    massive_key_entry()?.set_password(api_key).map_err(|e| {
        WorthweaveError::InvalidMarketData(format!(
            "could not save Massive API key in macOS Keychain: {e}"
        ))
    })?;
    Ok(crate::models::MassiveProviderStatus { configured: true })
}

pub fn remove_massive_api_key() -> Result<crate::models::MassiveProviderStatus> {
    match massive_key_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            Ok(crate::models::MassiveProviderStatus { configured: false })
        }
        Err(e) => Err(WorthweaveError::InvalidMarketData(format!(
            "could not remove Massive API key from macOS Keychain: {e}"
        ))),
    }
}

pub fn massive_candidates(connection: &Connection) -> Result<Vec<MassiveCandidate>> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for holding in projections::holdings(connection)? {
        let Some(symbol) = holding.symbol else {
            continue;
        };
        if !seen.insert(holding.instrument_id.clone()) {
            continue;
        }
        let source: Option<String> = connection
            .query_row(
                "SELECT source FROM market_prices WHERE instrument_id=?1",
                [&holding.instrument_id],
                |row| row.get(0),
            )
            .optional()?;
        if source
            .as_deref()
            .is_none_or(|source| source == "massive_snapshot")
        {
            candidates.push(MassiveCandidate {
                instrument_id: holding.instrument_id,
                symbol,
            });
        }
    }
    candidates.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    candidates.truncate(250);
    Ok(candidates)
}

fn valid_massive_symbol(symbol: &str) -> bool {
    !symbol.is_empty()
        && symbol.len() <= 32
        && symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

fn massive_snapshot_price(ticker: Option<&Value>) -> Option<Decimal> {
    ["/lastTrade/p", "/day/c", "/prevDay/c"]
        .into_iter()
        .filter_map(|path| ticker?.pointer(path)?.as_number())
        .filter_map(|number| Decimal::from_str(&number.to_string()).ok())
        .find(|price| *price > Decimal::ZERO)
}

async fn massive_json(client: &reqwest::Client, key: &str, url: String) -> Result<Option<Value>> {
    for attempt in 0..3 {
        let response = match client.get(&url).bearer_auth(key).send().await {
            Ok(response) => response,
            Err(error) if attempt < 2 && (error.is_timeout() || error.is_connect()) => {
                tokio::time::sleep(Duration::from_millis(250 * (1 << attempt))).await;
                continue;
            }
            Err(_) => {
                return Err(WorthweaveError::InvalidMarketData(
                    "Massive is temporarily unavailable".into(),
                ));
            }
        };
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if attempt < 2
            && (response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                || response.status().is_server_error())
        {
            tokio::time::sleep(Duration::from_millis(500 * (1 << attempt))).await;
            continue;
        }
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(WorthweaveError::InvalidMarketData(
                "Massive did not accept the configured API key".into(),
            ));
        }
        let response = response.error_for_status().map_err(|_| {
            WorthweaveError::InvalidMarketData(
                "Massive market data is temporarily unavailable".into(),
            )
        })?;
        return Ok(Some(response.json().await.map_err(|_| {
            WorthweaveError::InvalidMarketData("Massive returned an unreadable response".into())
        })?));
    }
    Err(WorthweaveError::InvalidMarketData(
        "Massive is temporarily unavailable".into(),
    ))
}

pub async fn fetch_massive_prices(
    candidates: Vec<MassiveCandidate>,
) -> Result<Vec<MassiveObservation>> {
    let key = massive_key_entry()?.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => {
            WorthweaveError::InvalidMarketData("Configure a Massive API key first".into())
        }
        other => WorthweaveError::InvalidMarketData(format!(
            "could not read Massive API key from macOS Keychain: {other}"
        )),
    })?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Worthweave/0.1")
        .build()
        .map_err(|e| {
            WorthweaveError::InvalidMarketData(format!("could not prepare Massive request: {e}"))
        })?;
    let requests = candidates.into_iter().map(|candidate| {
        let client = client.clone();
        let key = key.clone();
        async move {
            let fallback = candidate.symbol.trim().to_uppercase();
            fetch_massive_candidate(&client, &key, candidate)
                .await
                .unwrap_or(MassiveObservation::Failed(fallback))
        }
    });
    Ok(stream::iter(requests).buffer_unordered(4).collect().await)
}

async fn fetch_massive_candidate(
    client: &reqwest::Client,
    key: &str,
    candidate: MassiveCandidate,
) -> Result<MassiveObservation> {
    let symbol = candidate.symbol.trim().to_uppercase();
    if !valid_massive_symbol(&symbol) {
        return Ok(MassiveObservation::Unsupported(candidate.symbol));
    }
    let url = format!("{MASSIVE_API_BASE}/v2/snapshot/locale/us/markets/stocks/tickers/{symbol}");
    let snapshot = massive_json(client, key, url).await?;
    let ticker = snapshot.as_ref().and_then(|v| v.get("ticker"));
    if let Some(price) = massive_snapshot_price(ticker) {
        let millis = ticker
            .and_then(|t| t.get("updated"))
            .and_then(Value::as_i64)
            .filter(|value| *value > 0)
            .map(|n| {
                if n > 10_000_000_000_000 {
                    n / 1_000_000
                } else {
                    n
                }
            });
        let as_of = millis
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .unwrap_or_else(Utc::now)
            .to_rfc3339();
        return Ok(MassiveObservation::Price {
            candidate,
            price,
            as_of,
        });
    }
    let reference = massive_json(
        client,
        key,
        format!("{MASSIVE_API_BASE}/v3/reference/tickers?ticker={symbol}&active=false&limit=10"),
    )
    .await?;
    let delisted = reference
        .as_ref()
        .and_then(|v| v.get("results"))
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("ticker")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.eq_ignore_ascii_case(&symbol))
            })
        });
    Ok(if delisted && candidate.instrument_id.starts_with("US") {
        MassiveObservation::Delisted(symbol)
    } else if delisted {
        MassiveObservation::ForeignInactiveMatch(symbol)
    } else {
        MassiveObservation::NotFound(symbol)
    })
}

pub fn save_massive_prices(
    connection: &Connection,
    observations: Vec<MassiveObservation>,
) -> Result<crate::models::MassiveRefreshResult> {
    let requested = observations.len();
    let transaction = connection.unchecked_transaction()?;
    let (
        mut prices_saved,
        mut delisted,
        mut foreign_inactive_matches,
        mut not_found,
        mut unsupported,
        mut failed,
    ) = (0, vec![], vec![], vec![], vec![], vec![]);
    for observation in observations {
        match observation {
            MassiveObservation::Price {
                candidate,
                price,
                as_of,
            } => {
                let (coefficient, scale) = parts(price);
                prices_saved += transaction.execute(
                    "INSERT INTO market_prices (instrument_id, price_coefficient, price_scale, currency, as_of, source)
                     VALUES (?1, ?2, ?3, 'USD', ?4, 'massive_snapshot')
                     ON CONFLICT(instrument_id) DO UPDATE SET price_coefficient=excluded.price_coefficient, price_scale=excluded.price_scale,
                     currency=excluded.currency, as_of=excluded.as_of, source=excluded.source WHERE market_prices.source='massive_snapshot' AND excluded.as_of >= market_prices.as_of",
                    params![candidate.instrument_id, coefficient, scale, as_of])?;
            }
            MassiveObservation::Delisted(s) => delisted.push(s),
            MassiveObservation::ForeignInactiveMatch(s) => foreign_inactive_matches.push(s),
            MassiveObservation::NotFound(s) => not_found.push(s),
            MassiveObservation::Unsupported(s) => unsupported.push(s),
            MassiveObservation::Failed(s) => failed.push(s),
        }
    }
    transaction.commit()?;
    Ok(crate::models::MassiveRefreshResult {
        requested,
        prices_saved,
        delisted,
        foreign_inactive_matches,
        not_found,
        unsupported,
        failed,
    })
}

pub(crate) struct EcbReferenceRates {
    as_of: String,
    rates_per_eur: BTreeMap<String, Decimal>,
}

fn parse_positive(value: &str, field: &str) -> Result<Decimal> {
    Decimal::from_str(value.trim())
        .ok()
        .filter(|value| *value > Decimal::ZERO)
        .ok_or_else(|| {
            WorthweaveError::InvalidMarketData(format!("{field} must be a positive decimal"))
        })
}

fn parts(value: Decimal) -> (String, u32) {
    let value = value.normalize();
    (value.mantissa().to_string(), value.scale())
}

fn from_parts(coefficient: String, scale: u32) -> Result<Decimal> {
    let coefficient = coefficient
        .parse::<i128>()
        .map_err(|_| WorthweaveError::InvalidMarketData("stored decimal is invalid".into()))?;
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
        Err(WorthweaveError::InvalidMarketData(
            "unsupported currency".into(),
        ))
    }
}

pub fn set_price(connection: &Connection, input: &SetPriceInput) -> Result<PriceQuote> {
    let instrument_id = input.instrument_id.trim();
    if instrument_id.is_empty() || instrument_id.chars().count() > 128 {
        return Err(WorthweaveError::InvalidMarketData(
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
        return Err(WorthweaveError::InvalidMarketData(
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

fn parse_ecb_reference_rates(xml: &str) -> Result<EcbReferenceRates> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut date = None;
    let mut rates_per_eur = BTreeMap::from([("EUR".to_owned(), Decimal::ONE)]);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Empty(element))
            | Ok(quick_xml::events::Event::Start(element))
                if element.name().as_ref().ends_with(b"Cube") =>
            {
                let mut currency = None;
                let mut rate = None;
                for attribute in element.attributes().with_checks(false) {
                    let attribute = attribute.map_err(|_| {
                        WorthweaveError::InvalidMarketData(
                            "ECB exchange-rate response contains invalid XML attributes".into(),
                        )
                    })?;
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Explicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|_| {
                            WorthweaveError::InvalidMarketData(
                                "ECB exchange-rate response contains invalid text".into(),
                            )
                        })?
                        .into_owned();
                    match attribute.key.as_ref() {
                        b"time" => date = Some(value),
                        b"currency" => currency = Some(value),
                        b"rate" => rate = Some(value),
                        _ => {}
                    }
                }
                if let (Some(currency), Some(rate)) = (currency, rate) {
                    rates_per_eur.insert(currency, parse_positive(&rate, "ECB rate")?);
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                return Err(WorthweaveError::InvalidMarketData(
                    "ECB exchange-rate response is not valid XML".into(),
                ));
            }
        }
    }
    let date = date.ok_or_else(|| {
        WorthweaveError::InvalidMarketData(
            "ECB exchange-rate response does not contain a publication date".into(),
        )
    })?;
    if rates_per_eur.len() < 2 {
        return Err(WorthweaveError::InvalidMarketData(
            "ECB exchange-rate response does not contain reference rates".into(),
        ));
    }
    Ok(EcbReferenceRates {
        as_of: format!("{date}T16:00:00+00:00"),
        rates_per_eur,
    })
}

pub(crate) async fn fetch_ecb_reference_rates() -> Result<EcbReferenceRates> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| {
            WorthweaveError::InvalidMarketData(format!(
                "could not prepare the ECB exchange-rate request: {error}"
            ))
        })?;
    let response = client
        .get(ECB_DAILY_RATES_URL)
        .header(reqwest::header::USER_AGENT, "Worthweave/0.1")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| {
            WorthweaveError::InvalidMarketData(format!(
                "could not download ECB reference rates: {error}"
            ))
        })?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ECB_RESPONSE_BYTES as u64)
    {
        return Err(WorthweaveError::InvalidMarketData(
            "ECB exchange-rate response is unexpectedly large".into(),
        ));
    }
    let bytes = response.bytes().await.map_err(|error| {
        WorthweaveError::InvalidMarketData(format!("could not read ECB reference rates: {error}"))
    })?;
    if bytes.len() > MAX_ECB_RESPONSE_BYTES {
        return Err(WorthweaveError::InvalidMarketData(
            "ECB exchange-rate response is unexpectedly large".into(),
        ));
    }
    let xml = std::str::from_utf8(&bytes).map_err(|_| {
        WorthweaveError::InvalidMarketData("ECB exchange-rate response is not valid UTF-8".into())
    })?;
    parse_ecb_reference_rates(xml)
}

pub(crate) fn save_ecb_reference_rates(
    connection: &Connection,
    reference: &EcbReferenceRates,
) -> Result<FxRefreshResult> {
    let reporting_currency = db::settings(connection)?
        .reporting_currency
        .unwrap_or_else(|| "GBP".into());
    let quote_per_eur = reference
        .rates_per_eur
        .get(&reporting_currency)
        .ok_or_else(|| {
            WorthweaveError::InvalidMarketData(format!(
                "ECB does not publish a {reporting_currency} reference rate"
            ))
        })?;
    let transaction = connection.unchecked_transaction()?;
    let mut rates_saved = 0;
    for currency in db::CURRENCIES {
        if currency.code == reporting_currency {
            continue;
        }
        let Some(base_per_eur) = reference.rates_per_eur.get(currency.code) else {
            continue;
        };
        let rate = quote_per_eur / base_per_eur;
        let (coefficient, scale) = parts(rate);
        rates_saved += transaction.execute(
            "INSERT INTO fx_rates (base_currency, quote_currency, rate_coefficient, rate_scale, as_of, source)
             VALUES (?1, ?2, ?3, ?4, ?5, 'ecb_reference')
             ON CONFLICT(base_currency, quote_currency) DO UPDATE SET
               rate_coefficient=excluded.rate_coefficient,
               rate_scale=excluded.rate_scale,
               as_of=excluded.as_of,
               source=excluded.source
             WHERE fx_rates.source='ecb_reference' AND excluded.as_of >= fx_rates.as_of",
            params![
                currency.code,
                reporting_currency,
                coefficient,
                scale,
                reference.as_of
            ],
        )?;
    }
    transaction.commit()?;
    Ok(FxRefreshResult {
        as_of: reference.as_of.clone(),
        rates_saved,
        source: "European Central Bank",
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

pub(crate) fn convert_amount(
    connection: &Connection,
    amount: Decimal,
    base: &str,
    quote: &str,
) -> Result<Option<Decimal>> {
    Ok(fx(connection, base, quote)?.map(|(rate, _)| amount * rate))
}

pub fn valuation(connection: &Connection) -> Result<ValuationSummary> {
    let reporting_currency = db::settings(connection)?
        .reporting_currency
        .unwrap_or_else(|| "GBP".into());
    let mut total = Decimal::ZERO;
    let mut missing_price_count = 0;
    let mut missing_fx_pairs = BTreeSet::new();
    let mut valued_holding_count = 0;
    let mut stale_price_count = 0;
    let mut stale_fx_count = 0;
    let mut total_gain = Decimal::ZERO;
    let mut gain_complete = true;
    let mut valued = Vec::new();
    for holding in projections::holdings(connection)? {
        let quantity = Decimal::from_str(&holding.quantity).map_err(|_| {
            WorthweaveError::InvalidMarketData("holding quantity is invalid".into())
        })?;
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
                valued_holding_count += 1;
                (
                    Some(price_quote),
                    Some(market_value.normalize().to_string()),
                    Some(reporting_value.normalize().to_string()),
                )
            } else {
                missing_fx_pairs.insert((price_quote.currency.clone(), reporting_currency.clone()));
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
                        WorthweaveError::InvalidMarketData("holding cost basis is invalid".into())
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
    let missing_fx_count = missing_fx_pairs.len();
    let valuation_complete = missing_price_count == 0 && missing_fx_count == 0;
    let total_value = (valued_holding_count > 0).then(|| total.normalize().to_string());
    Ok(ValuationSummary {
        reporting_currency,
        total_value,
        valuation_complete,
        valued_holding_count,
        missing_price_count,
        missing_fx_count,
        stale_price_count,
        stale_fx_count,
        total_gain_loss: gain_complete.then(|| total_gain.normalize().to_string()),
        holdings: valued,
    })
}

#[derive(Default)]
struct AttributionPosition {
    quantity: Decimal,
    cost_basis: Decimal,
    complete: bool,
}

fn add_converted(
    connection: &Connection,
    total: &mut Decimal,
    value: Decimal,
    currency: &str,
    reporting_currency: &str,
) -> Result<bool> {
    if let Some((rate, _)) = fx(connection, currency, reporting_currency)? {
        *total += value * rate;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn from_optional_parts(coefficient: Option<String>, scale: Option<u32>) -> Result<Option<Decimal>> {
    coefficient
        .zip(scale)
        .map(|(coefficient, scale)| from_parts(coefficient, scale))
        .transpose()
}

pub fn total_return_attribution(connection: &Connection) -> Result<TotalReturnAttribution> {
    let reporting_currency = db::settings(connection)?
        .reporting_currency
        .unwrap_or_else(|| "GBP".into());
    let valuation = valuation(connection)?;
    let coverage: (Option<String>, Option<String>) = connection.query_row(
        "SELECT MIN(coverage_start), MAX(coverage_end) FROM import_batches",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut statement = connection.prepare(
        "SELECT account_id, instrument_id, event_type, amount_coefficient, amount_scale,
                currency, quantity_coefficient, quantity_scale
         FROM events ORDER BY occurred_at, id",
    )?;
    let mut rows = statement.query([])?;
    let mut positions: BTreeMap<(String, String, String), AttributionPosition> = BTreeMap::new();
    let mut realized = Decimal::ZERO;
    let mut dividends = Decimal::ZERO;
    let mut interest = Decimal::ZERO;
    let mut fees = Decimal::ZERO;
    let mut taxes = Decimal::ZERO;
    let mut realized_complete = true;
    let mut cash_complete = true;
    let mut unclassified_event_count = 0usize;
    let mut foreign_activity = valuation.holdings.iter().any(|item| {
        item.price
            .as_ref()
            .is_some_and(|price| price.currency != reporting_currency)
            || item
                .holding
                .currency
                .as_ref()
                .is_some_and(|currency| currency != &reporting_currency)
    });

    while let Some(row) = rows.next()? {
        let event_type: String = row.get(2)?;
        let amount = from_optional_parts(row.get(3)?, row.get(4)?)?.map(|value| value.abs());
        let currency: Option<String> = row.get(5)?;
        if currency
            .as_deref()
            .is_some_and(|value| value != reporting_currency)
        {
            foreign_activity = true;
        }
        if matches!(event_type.as_str(), "buy" | "sell") {
            let (Some(instrument_id), Some(currency), Some(amount), Some(quantity)) = (
                row.get::<_, Option<String>>(1)?,
                currency,
                amount,
                from_optional_parts(row.get(6)?, row.get(7)?)?.map(|value| value.abs()),
            ) else {
                realized_complete = false;
                continue;
            };
            let account_id: String = row.get(0)?;
            if quantity.is_zero() {
                realized_complete = false;
                continue;
            }
            let position = positions
                .entry((account_id, instrument_id, currency.clone()))
                .or_insert_with(|| AttributionPosition {
                    complete: true,
                    ..Default::default()
                });
            if event_type == "buy" {
                position.quantity += quantity;
                position.cost_basis += amount;
            } else if quantity > position.quantity || !position.complete {
                position.complete = false;
                realized_complete = false;
            } else {
                let disposed_basis = position.cost_basis / position.quantity * quantity;
                if !add_converted(
                    connection,
                    &mut realized,
                    amount - disposed_basis,
                    &currency,
                    &reporting_currency,
                )? {
                    realized_complete = false;
                }
                position.quantity -= quantity;
                position.cost_basis -= disposed_basis;
            }
            continue;
        }
        if event_type == "corporate_action"
            || (event_type == "transfer" && row.get::<_, Option<String>>(1)?.is_some())
            || (event_type == "other" && amount.is_some())
        {
            unclassified_event_count += 1;
            if event_type != "other" {
                realized_complete = false;
            } else {
                cash_complete = false;
            }
            continue;
        }
        if !matches!(event_type.as_str(), "dividend" | "interest" | "fee" | "tax") {
            continue;
        }
        let (Some(amount), Some(currency)) = (amount, currency) else {
            cash_complete = false;
            continue;
        };
        let target = match event_type.as_str() {
            "dividend" => &mut dividends,
            "interest" => &mut interest,
            "fee" => &mut fees,
            "tax" => &mut taxes,
            _ => unreachable!(),
        };
        if !add_converted(connection, target, amount, &currency, &reporting_currency)? {
            cash_complete = false;
        }
    }

    let unrealized = valuation
        .total_gain_loss
        .as_deref()
        .and_then(|value| Decimal::from_str(value).ok());
    let components_complete = realized_complete && cash_complete && unrealized.is_some();
    let subtotal = components_complete
        .then(|| realized + unrealized.unwrap_or_default() + dividends + interest - fees - taxes);
    let fx_complete = !foreign_activity;
    let total_return = (components_complete && fx_complete).then(|| subtotal.unwrap_or_default());
    let mut notes = Vec::new();
    if coverage.0.is_none() {
        notes.push("Import your account history to calculate your investment return.".into());
    }
    if !realized_complete {
        notes.push("Realised gains are unavailable where transaction history, quantities, amounts, or exchange rates are incomplete.".into());
    }
    if unrealized.is_none() {
        notes.push("To calculate gains on investments you still own, add your full account history, current prices and any missing exchange rates.".into());
    }
    if !cash_complete {
        notes.push(
            "Some income, fees or taxes could not be converted into your main currency.".into(),
        );
    }
    if unclassified_event_count > 0 {
        notes.push(format!(
            "{unclassified_event_count} account event(s) need checking before Worthweave can calculate your full return."
        ));
    }
    if foreign_activity {
        notes.push("Some investments use another currency. Add exchange rates for the transaction dates before Worthweave can show your full return.".into());
    }
    if coverage.0.is_some() {
        notes.push("These figures only cover the history you imported. Earlier activity may change the amount invested and gains from investments you sold.".into());
    }
    let status = if coverage.0.is_none() {
        "unavailable"
    } else if total_return.is_some() {
        "complete"
    } else {
        "partial"
    };
    Ok(TotalReturnAttribution {
        reporting_currency,
        coverage_start: coverage.0,
        coverage_end: coverage.1,
        status,
        realized_gain_loss: realized_complete.then(|| realized.normalize().to_string()),
        unrealized_gain_loss: unrealized.map(|value| value.normalize().to_string()),
        dividends: cash_complete.then(|| dividends.normalize().to_string()),
        interest: cash_complete.then(|| interest.normalize().to_string()),
        fees: cash_complete.then(|| fees.normalize().to_string()),
        taxes: cash_complete.then(|| taxes.normalize().to_string()),
        fx_impact: fx_complete.then(|| "0".into()),
        attributed_subtotal: subtotal.map(|value| value.normalize().to_string()),
        total_return: total_return.map(|value| value.normalize().to_string()),
        notes,
    })
}

pub fn capture_snapshot(connection: &Connection) -> Result<PortfolioSnapshot> {
    let valuation = valuation(connection)?;
    if !valuation.valuation_complete {
        return Err(WorthweaveError::InvalidMarketData(
            "add current prices and exchange rates for every investment before saving today’s value"
                .into(),
        ));
    }
    let total = valuation.total_value.ok_or_else(|| {
        WorthweaveError::InvalidMarketData(
            "add current prices and exchange rates for every investment before saving today’s value".into(),
        )
    })?;
    let total_decimal = Decimal::from_str(&total)
        .map_err(|_| WorthweaveError::InvalidMarketData("valuation total is invalid".into()))?;
    let (coefficient, scale) = parts(total_decimal);
    let snapshot = PortfolioSnapshot {
        id: Uuid::new_v4().to_string(),
        captured_at: Utc::now().to_rfc3339(),
        reporting_currency: valuation.reporting_currency,
        total_value: total,
    };
    connection.execute(
        "DELETE FROM portfolio_snapshots WHERE substr(captured_at, 1, 10) = substr(?1, 1, 10)",
        [&snapshot.captured_at],
    )?;
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
    if !valuation.valuation_complete {
        return Err(WorthweaveError::InvalidMarketData(
            "allocation requires a complete portfolio valuation".into(),
        ));
    }
    let total = valuation
        .total_value
        .as_deref()
        .ok_or_else(|| {
            WorthweaveError::InvalidMarketData(
                "allocation requires a complete portfolio valuation".into(),
            )
        })
        .and_then(|value| {
            Decimal::from_str(value).map_err(|_| {
                WorthweaveError::InvalidMarketData("valuation total is invalid".into())
            })
        })?;
    if total <= Decimal::ZERO {
        return Err(WorthweaveError::InvalidMarketData(
            "allocation requires a positive portfolio value".into(),
        ));
    }
    let mut accounts: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut currencies: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut platforms: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut asset_classes: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut sectors: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut geographies: BTreeMap<String, Decimal> = BTreeMap::new();
    for item in valuation.holdings {
        let value =
            Decimal::from_str(item.reporting_value.as_deref().unwrap_or("0")).map_err(|_| {
                WorthweaveError::InvalidMarketData("holding valuation is invalid".into())
            })?;
        *accounts.entry(item.holding.account_name).or_default() += value;
        *platforms.entry(item.holding.broker.clone()).or_default() += value;
        *asset_classes
            .entry(
                item.holding
                    .asset_class
                    .clone()
                    .unwrap_or_else(|| "Unclassified".into()),
            )
            .or_default() += value;
        *sectors
            .entry(
                item.holding
                    .sector
                    .clone()
                    .unwrap_or_else(|| "Unclassified".into()),
            )
            .or_default() += value;
        *geographies
            .entry(
                item.holding
                    .geography
                    .clone()
                    .unwrap_or_else(|| "Unclassified".into()),
            )
            .or_default() += value;
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
        by_platform: slices(platforms),
        by_asset_class: slices(asset_classes),
        by_sector: slices(sectors),
        by_geography: slices(geographies),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn massive_snapshot_falls_back_from_zero_day_to_previous_close() {
        let response = serde_json::json!({
            "ticker": {
                "lastTrade": null,
                "day": { "c": 0 },
                "prevDay": { "c": 315.32 }
            }
        });
        let price = massive_snapshot_price(response.get("ticker")).expect("previous close");
        assert_eq!(price.to_string(), "315.32");
    }

    #[test]
    fn only_us_isins_can_be_classified_as_delisted_by_massive() {
        assert!("US0378331005".starts_with("US"));
        assert!(!"GB00B24CGK77".starts_with("US"));
        assert!(!"VGG7223M1005".starts_with("US"));
    }

    const ECB_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
      <gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
        <Cube><Cube time="2026-07-10"><Cube currency="USD" rate="1.1430"/><Cube currency="GBP" rate="0.85155"/></Cube></Cube>
      </gesmes:Envelope>"#;

    #[test]
    fn ecb_reference_rates_are_crossed_exactly_and_do_not_replace_manual_overrides() {
        let reference = parse_ecb_reference_rates(ECB_FIXTURE).expect("ECB fixture");
        assert_eq!(reference.as_of, "2026-07-10T16:00:00+00:00");
        assert_eq!(
            reference.rates_per_eur.get("USD"),
            Some(&Decimal::from_str("1.1430").expect("USD rate"))
        );

        let directory = tempfile::tempdir().expect("temp directory");
        let connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let first = save_ecb_reference_rates(&connection, &reference).expect("save ECB rates");
        assert_eq!(first.rates_saved, 2);
        let expected = Decimal::from_str("0.85155").expect("GBP rate")
            / Decimal::from_str("1.1430").expect("USD rate");
        assert_eq!(
            fx(&connection, "USD", "GBP")
                .expect("stored FX")
                .map(|(rate, _)| rate),
            Some(expected)
        );

        set_fx_rate(
            &connection,
            &SetFxRateInput {
                base_currency: "USD".into(),
                quote_currency: "GBP".into(),
                rate: "0.75".into(),
            },
        )
        .expect("manual override");
        save_ecb_reference_rates(&connection, &reference).expect("refresh ECB rates");
        assert_eq!(
            fx(&connection, "USD", "GBP")
                .expect("manual FX")
                .map(|(rate, _)| rate),
            Some(Decimal::from_str("0.75").expect("manual rate"))
        );
    }
}
