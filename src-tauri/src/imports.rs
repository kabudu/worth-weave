use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use csv::StringRecord;
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db;
use crate::error::{Result, WorthweaveError};
use crate::models::ImportResult;

const MAX_IMPORT_BYTES: u64 = 50 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 500_000;

#[derive(Debug)]
struct Event {
    source_id: String,
    event_type: &'static str,
    occurred_at: String,
    description: String,
    amount: Option<ExactValue>,
    currency: Option<String>,
    quantity: Option<ExactValue>,
    instrument_id: Option<String>,
    symbol: Option<String>,
    name: Option<String>,
    asset_class: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ExactValue {
    coefficient: String,
    scale: u32,
}

struct ParsedImport {
    start: NaiveDate,
    end: NaiveDate,
    events: Vec<Event>,
    warnings: Vec<String>,
    positions: Vec<PositionSnapshot>,
}

#[derive(Debug)]
struct PositionSnapshot {
    report_date: NaiveDate,
    instrument_id: String,
    quantity: ExactValue,
    symbol: Option<String>,
    name: Option<String>,
    asset_class: Option<String>,
    market_price: Option<ExactValue>,
    price_currency: Option<String>,
}

fn parse_date(value: &str, context: &str) -> Result<(NaiveDate, String)> {
    let value = value.trim();
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d;%H:%M:%S",
        "%Y-%m-%d;%H%M%S",
        "%Y-%m-%dT%H:%M:%S",
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Ok((
                parsed.date(),
                parsed.format("%Y-%m-%dT%H:%M:%S").to_string(),
            ));
        }
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|date| (date, format!("{date}T00:00:00")))
        .map_err(|_| WorthweaveError::Csv(format!("invalid date in {context}")))
}

fn decimal(value: Option<&str>, context: &str) -> Result<Option<Decimal>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    Decimal::from_str(value.replace(',', "").as_str())
        .map(Some)
        .map_err(|_| WorthweaveError::Csv(format!("invalid decimal in {context}")))
}

fn exact_value(value: Decimal, minimum_scale: Option<u32>) -> ExactValue {
    let mut value = value.normalize();
    if let Some(scale) = minimum_scale.filter(|scale| value.scale() < *scale) {
        value.rescale(scale);
    }
    ExactValue {
        coefficient: value.mantissa().to_string(),
        scale: value.scale(),
    }
}

fn currency_scale(currency: &str) -> u32 {
    match currency {
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        _ => 2,
    }
}

fn stable_id(prefix: &str, row: &StringRecord) -> String {
    let mut hash = Sha256::new();
    for value in row {
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    format!("{prefix}:{}", hex(&hash.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn field<'a>(
    row: &'a StringRecord,
    positions: &HashMap<&str, usize>,
    name: &str,
) -> Option<&'a str> {
    positions.get(name).and_then(|index| row.get(*index))
}

fn action_type(action: &str) -> &'static str {
    let action = action.to_lowercase();
    if action.contains("buy") {
        "buy"
    } else if action.contains("sell") {
        "sell"
    } else if action.contains("tax") || action.contains("withholding") {
        "tax"
    } else if action.contains("dividend") {
        "dividend"
    } else if action == "deposit" {
        "deposit"
    } else if action == "withdrawal" {
        "withdrawal"
    } else if action.contains("interest") {
        "interest"
    } else if action.contains("fee") {
        "fee"
    } else if action.contains("split") || action.contains("adjustment") {
        "corporate_action"
    } else {
        "other"
    }
}

fn parse_trading212(content: &[u8]) -> Result<ParsedImport> {
    let text = std::str::from_utf8(content)
        .map_err(|_| WorthweaveError::Csv("Trading 212 export must be UTF-8 CSV".into()))?
        .trim_start_matches('\u{feff}');
    let mut reader = csv::ReaderBuilder::new().from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| WorthweaveError::Csv(error.to_string()))?
        .clone();
    for required in ["Action", "Time", "ID"] {
        if !headers.iter().any(|header| header == required) {
            return Err(WorthweaveError::Csv(format!(
                "Trading 212 export is missing column: {required}"
            )));
        }
    }
    let positions: HashMap<&str, usize> = headers
        .iter()
        .enumerate()
        .map(|(index, name)| (name, index))
        .collect();
    let mut events = Vec::new();
    let mut dates = Vec::new();
    for (offset, record) in reader.records().enumerate() {
        let row =
            record.map_err(|error| WorthweaveError::Csv(format!("row {}: {error}", offset + 2)))?;
        let action = field(&row, &positions, "Action").unwrap_or("").trim();
        let (date, occurred_at) = parse_date(
            field(&row, &positions, "Time").unwrap_or(""),
            &format!("Time at row {}", offset + 2),
        )?;
        dates.push(date);
        let raw_id = field(&row, &positions, "ID").unwrap_or("").trim();
        let source = if raw_id.is_empty() {
            stable_id("t212", &row)
        } else {
            raw_id.to_owned()
        };
        let amount_decimal = decimal(
            field(&row, &positions, "Total"),
            &format!("Total at row {}", offset + 2),
        )?;
        let currency = field(&row, &positions, "Currency (Total)")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let amount =
            amount_decimal.map(|value| exact_value(value, currency.as_deref().map(currency_scale)));
        let quantity = decimal(
            field(&row, &positions, "No. of shares"),
            &format!("shares at row {}", offset + 2),
        )?
        .map(|value| exact_value(value, None));
        let instrument_id = field(&row, &positions, "ISIN")
            .or_else(|| field(&row, &positions, "Ticker"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let symbol = field(&row, &positions, "Ticker")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let name = field(&row, &positions, "Name")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        events.push(Event {
            source_id: format!("t212:{source}"),
            event_type: action_type(action),
            occurred_at,
            description: if action.is_empty() {
                "Trading 212 event".into()
            } else {
                action.into()
            },
            amount,
            currency,
            quantity,
            instrument_id,
            symbol,
            name,
            asset_class: None,
        });
    }
    let start =
        dates.iter().min().copied().ok_or_else(|| {
            WorthweaveError::Csv("Trading 212 export contains no data rows".into())
        })?;
    let end = dates.iter().max().copied().expect("non-empty dates");
    Ok(ParsedImport {
        start,
        end,
        events,
        warnings: Vec::new(),
        positions: Vec::new(),
    })
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn section(header: &[String]) -> Option<&'static str> {
    let has = |fields: &[&str]| {
        fields
            .iter()
            .all(|field| header.iter().any(|value| value == field))
    };
    if has(&[
        "ReportDate",
        "Quantity",
        "MarkPrice",
        "PositionValue",
        "CostBasisMoney",
    ]) {
        Some("open_positions")
    } else if has(&["TradeID", "BuySell", "TradeMoney"]) {
        Some("trades")
    } else if has(&["TransactionID", "Amount", "Type", "ActionID"]) {
        Some("cash_transactions")
    } else if has(&["ActionDescription", "ActionID", "TransactionID"]) {
        Some("corporate_actions")
    } else if has(&["TransferCompany", "CashTransfer", "TransactionID"]) {
        Some("transfers")
    } else if has(&["TaxDescription", "TaxAmount", "TradeID"]) {
        Some("transaction_fees")
    } else if has(&["TransactionType", "CommTax", "Basis", "TradeID"]) {
        Some("option_events")
    } else {
        None
    }
}

fn ibkr_type(section: &str, row: &HashMap<String, String>) -> &'static str {
    if section == "trades" {
        return if row
            .get("BuySell")
            .is_some_and(|value| value.eq_ignore_ascii_case("buy"))
        {
            "buy"
        } else {
            "sell"
        };
    }
    if matches!(section, "corporate_actions" | "option_events") {
        return "corporate_action";
    }
    if section == "transfers" {
        return "transfer";
    }
    if section == "transaction_fees" {
        return if row
            .get("TaxDescription")
            .is_some_and(|value| !value.trim().is_empty())
        {
            "tax"
        } else {
            "fee"
        };
    }
    let label = format!(
        "{} {}",
        row.get("Type").map_or("", String::as_str),
        row.get("Description").map_or("", String::as_str)
    )
    .to_lowercase();
    if label.contains("tax") || label.contains("withholding") {
        "tax"
    } else if label.contains("dividend") {
        "dividend"
    } else if label.contains("deposit") {
        "deposit"
    } else if label.contains("withdraw") {
        "withdrawal"
    } else if label.contains("interest") {
        "interest"
    } else if label.contains("fee") || label.contains("commission") {
        "fee"
    } else {
        "other"
    }
}

fn ibkr_explicit_instrument_id(row: &HashMap<String, String>) -> Option<String> {
    row.get("ISIN")
        .or_else(|| row.get("Conid"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn ibkr_symbol(row: &HashMap<String, String>) -> Option<String> {
    row.get("Symbol")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_uppercase())
}

fn parse_ibkr(content: &[u8]) -> Result<ParsedImport> {
    let text = std::str::from_utf8(content)
        .map_err(|_| WorthweaveError::Csv("IBKR export must be UTF-8 CSV".into()))?
        .trim_start_matches('\u{feff}');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut header: Option<Vec<String>> = None;
    let mut current_section = None;
    let mut ignored = false;
    let mut dates = Vec::new();
    let mut events = Vec::new();
    let mut positions = Vec::new();
    for (offset, record) in reader.records().enumerate() {
        let values =
            record.map_err(|error| WorthweaveError::Csv(format!("row {}: {error}", offset + 1)))?;
        if values.is_empty() {
            continue;
        }
        let is_header = values.get(0) == Some("ClientAccountID")
            || (values.get(0) == Some("CurrencyPrimary")
                && values.iter().any(|value| value == "SettlementPolicyMethod"));
        if is_header {
            let next: Vec<String> = values.iter().map(normalized).collect();
            current_section = section(&next);
            ignored |= current_section.is_none();
            header = Some(next);
            continue;
        }
        let names = header.as_ref().ok_or_else(|| {
            WorthweaveError::Csv(
                "IBKR export does not begin with a recognized section header".into(),
            )
        })?;
        let row: HashMap<String, String> = names
            .iter()
            .cloned()
            .zip(values.iter().map(str::to_owned))
            .collect();
        for field in ["DateTime", "Date", "ReportDate", "TradeDate", "DateOpened"] {
            if let Some(value) = row.get(field).filter(|value| value.starts_with("20")) {
                dates.push(parse_date(value, &format!("{field} at row {}", offset + 1))?.0);
                break;
            }
        }
        let Some(section) = current_section else {
            continue;
        };
        if section == "open_positions" {
            if row
                .get("LevelOfDetail")
                .is_some_and(|value| !value.is_empty() && !value.eq_ignore_ascii_case("summary"))
            {
                continue;
            }
            let symbol = ibkr_symbol(&row);
            let Some(instrument_id) = ibkr_explicit_instrument_id(&row)
                .or_else(|| symbol.as_ref().map(|value| format!("symbol:{value}")))
            else {
                continue;
            };
            let Some(report_date) = row.get("ReportDate").filter(|value| !value.is_empty()) else {
                continue;
            };
            let (report_date, _) =
                parse_date(report_date, &format!("ReportDate at row {}", offset + 1))?;
            if let Some(quantity) = decimal(
                row.get("Quantity").map(String::as_str),
                &format!("Quantity at row {}", offset + 1),
            )? {
                positions.push(PositionSnapshot {
                    report_date,
                    instrument_id,
                    quantity: exact_value(quantity, None),
                    symbol,
                    name: row
                        .get("Description")
                        .filter(|value| !value.is_empty())
                        .cloned(),
                    asset_class: row
                        .get("AssetClass")
                        .filter(|value| !value.is_empty())
                        .cloned(),
                    market_price: decimal(
                        row.get("MarkPrice").map(String::as_str),
                        &format!("MarkPrice at row {}", offset + 1),
                    )?
                    .map(|value| exact_value(value, None)),
                    price_currency: row
                        .get("CurrencyPrimary")
                        .map(|value| value.trim().to_uppercase())
                        .filter(|value| !value.is_empty()),
                });
            }
            continue;
        }
        let occurred_raw = ["DateTime", "Date", "ReportDate", "TradeDate"]
            .iter()
            .find_map(|field| row.get(*field).filter(|value| !value.is_empty()));
        let Some(occurred_raw) = occurred_raw else {
            continue;
        };
        let (_, occurred_at) =
            parse_date(occurred_raw, &format!("event date at row {}", offset + 1))?;
        let raw_id = ["TransactionID", "TradeID", "ActionID"]
            .iter()
            .find_map(|field| row.get(*field).filter(|value| !value.trim().is_empty()));
        let source = raw_id
            .cloned()
            .unwrap_or_else(|| stable_id("ibkr", &values));
        let amount_raw = [
            "Amount",
            "TradeMoney",
            "NetCash",
            "CashTransfer",
            "TaxAmount",
            "CommTax",
        ]
        .iter()
        .find_map(|field| {
            row.get(*field)
                .filter(|value| !value.trim().is_empty())
                .map(String::as_str)
        });
        let currency = row
            .get("CurrencyPrimary")
            .filter(|value| !value.is_empty())
            .cloned();
        let amount = decimal(amount_raw, &format!("amount at row {}", offset + 1))?
            .map(|value| exact_value(value, currency.as_deref().map(currency_scale)));
        let quantity = decimal(
            row.get("Quantity").map(String::as_str),
            &format!("Quantity at row {}", offset + 1),
        )?
        .filter(|value| !value.is_zero())
        .map(|value| exact_value(value, None));
        let description = ["Description", "ActionDescription", "Type"]
            .iter()
            .find_map(|field| row.get(*field).filter(|value| !value.is_empty()))
            .cloned()
            .unwrap_or_else(|| format!("IBKR {section} event"));
        events.push(Event {
            source_id: format!("ibkr:{section}:{source}"),
            event_type: ibkr_type(section, &row),
            occurred_at,
            description,
            amount,
            currency,
            quantity,
            instrument_id: ibkr_explicit_instrument_id(&row),
            symbol: ibkr_symbol(&row),
            name: row
                .get("Description")
                .filter(|value| !value.is_empty())
                .cloned(),
            asset_class: row
                .get("AssetClass")
                .filter(|value| !value.is_empty())
                .cloned(),
        });
    }
    let mut instrument_by_symbol = HashMap::new();
    for position in &positions {
        if let Some(symbol) = &position.symbol {
            instrument_by_symbol.insert(symbol.clone(), position.instrument_id.clone());
        }
    }
    for event in &events {
        if let (Some(symbol), Some(instrument_id)) = (&event.symbol, &event.instrument_id) {
            instrument_by_symbol
                .entry(symbol.clone())
                .or_insert_with(|| instrument_id.clone());
        }
    }
    for event in &mut events {
        if event.instrument_id.is_none() {
            event.instrument_id = event.symbol.as_ref().map(|symbol| {
                instrument_by_symbol
                    .get(symbol)
                    .cloned()
                    .unwrap_or_else(|| format!("symbol:{symbol}"))
            });
        }
    }

    let start =
        dates.iter().min().copied().ok_or_else(|| {
            WorthweaveError::Csv("IBKR export contains no dated data rows".into())
        })?;
    let end = dates.iter().max().copied().expect("non-empty dates");
    let warnings = if ignored {
        vec![
            "Worthweave used the transaction rows and ignored other sections of this statement."
                .into(),
        ]
    } else {
        Vec::new()
    };
    Ok(ParsedImport {
        start,
        end,
        events,
        warnings,
        positions,
    })
}

pub fn import_csv(
    connection: &mut Connection,
    account_id: &str,
    path: &Path,
    confirmed_account_type: &str,
) -> Result<ImportResult> {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("csv"))
    {
        return Err(WorthweaveError::UnsupportedFile);
    }
    let (broker, account_type) =
        db::account_identity(connection, account_id)?.ok_or(WorthweaveError::AccountNotFound)?;
    if account_type != confirmed_account_type {
        return Err(WorthweaveError::AccountTypeMismatch);
    }
    let mut content = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_IMPORT_BYTES + 1)
        .read_to_end(&mut content)?;
    if content.len() as u64 > MAX_IMPORT_BYTES {
        return Err(WorthweaveError::ImportTooLarge);
    }
    let digest = hex(&Sha256::digest(&content));
    let parsed = match broker.as_str() {
        "trading_212" => parse_trading212(&content)?,
        "ibkr" => parse_ibkr(&content)?,
        "robinhood" => return Err(WorthweaveError::UnsupportedBrokerImport),
        _ => return Err(WorthweaveError::UnsupportedBrokerImport),
    };
    if parsed.events.len().saturating_add(parsed.positions.len()) > MAX_IMPORT_ROWS {
        return Err(WorthweaveError::ImportRowLimit);
    }
    for event in &parsed.events {
        if event.source_id.chars().count() > 512
            || event.description.chars().count() > 4096
            || event
                .instrument_id
                .as_ref()
                .is_some_and(|value| value.chars().count() > 128)
        {
            return Err(WorthweaveError::Csv(
                "import contains an oversized identifier or description".into(),
            ));
        }
    }
    if parsed
        .positions
        .iter()
        .any(|position| position.instrument_id.chars().count() > 128)
    {
        return Err(WorthweaveError::Csv(
            "import contains an oversized instrument identifier".into(),
        ));
    }
    let existing_batch: Option<String> = connection
        .query_row(
            "SELECT id FROM import_batches WHERE account_id = ?1 AND content_sha256 = ?2",
            params![account_id, digest],
            |row| row.get(0),
        )
        .optional()?;
    let repairing = existing_batch.is_some();
    let new_events = parsed.events;
    let batch_id = existing_batch.unwrap_or_else(|| Uuid::new_v4().to_string());
    let transaction = connection.transaction()?;
    if !repairing {
        transaction.execute("INSERT INTO import_batches (id, account_id, original_filename, content_sha256, coverage_start, coverage_end) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![batch_id, account_id, path.file_name().and_then(|value| value.to_str()).unwrap_or("broker-export.csv"), digest, parsed.start.to_string(), parsed.end.to_string()])?;
    }
    let mut events_added = 0;
    for event in &new_events {
        if let Some(instrument_id) = &event.instrument_id {
            transaction.execute(
                "INSERT INTO instruments (id, symbol, name, isin, asset_class) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                   symbol=COALESCE(excluded.symbol, instruments.symbol),
                   name=COALESCE(excluded.name, instruments.name),
                   isin=COALESCE(excluded.isin, instruments.isin),
                   asset_class=COALESCE(excluded.asset_class, instruments.asset_class), updated_at=CURRENT_TIMESTAMP",
                params![instrument_id, event.symbol, event.name, instrument_id, event.asset_class],
            )?;
        }
        events_added += transaction.execute("INSERT OR IGNORE INTO events (id, account_id, import_batch_id, source_id, event_type, occurred_at, description, amount_coefficient, amount_scale, currency, quantity_coefficient, quantity_scale, instrument_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)", params![Uuid::new_v4().to_string(), account_id, batch_id, event.source_id, event.event_type, event.occurred_at, event.description, event.amount.as_ref().map(|value| &value.coefficient), event.amount.as_ref().map(|value| value.scale), event.currency, event.quantity.as_ref().map(|value| &value.coefficient), event.quantity.as_ref().map(|value| value.scale), event.instrument_id])?;
        if event.instrument_id.is_some() {
            transaction.execute(
                "UPDATE events
                 SET instrument_id=CASE
                   WHEN instrument_id IS NULL OR instrument_id LIKE 'symbol:%' THEN ?3
                   ELSE instrument_id
                 END
                 WHERE account_id=?1 AND source_id=?2",
                params![account_id, event.source_id, event.instrument_id],
            )?;
        }
    }
    for position in &parsed.positions {
        transaction.execute(
            "INSERT INTO instruments (id, symbol, name, isin, asset_class) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               symbol=COALESCE(excluded.symbol, instruments.symbol),
               name=COALESCE(excluded.name, instruments.name),
               isin=COALESCE(excluded.isin, instruments.isin),
               asset_class=COALESCE(excluded.asset_class, instruments.asset_class), updated_at=CURRENT_TIMESTAMP",
            params![
                position.instrument_id,
                position.symbol,
                position.name,
                position.instrument_id, position.asset_class
            ],
        )?;
        if let (Some(price), Some(currency)) = (&position.market_price, &position.price_currency) {
            transaction.execute(
                "INSERT INTO market_prices (instrument_id, price_coefficient, price_scale, currency, as_of, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'broker_import')
                 ON CONFLICT(instrument_id) DO UPDATE SET
                   price_coefficient=excluded.price_coefficient,
                   price_scale=excluded.price_scale,
                   currency=excluded.currency,
                   as_of=excluded.as_of,
                   source=excluded.source
                 WHERE excluded.as_of >= market_prices.as_of",
                params![
                    position.instrument_id,
                    price.coefficient,
                    price.scale,
                    currency,
                    position.report_date.to_string()
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO broker_position_snapshots (id, account_id, import_batch_id, report_date, instrument_id, quantity_coefficient, quantity_scale)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(account_id, report_date, instrument_id) DO UPDATE SET
               quantity_coefficient=excluded.quantity_coefficient,
               quantity_scale=excluded.quantity_scale",
            params![
                Uuid::new_v4().to_string(), account_id, batch_id,
                position.report_date.to_string(), position.instrument_id,
                position.quantity.coefficient, position.quantity.scale
            ],
        )?;
    }
    transaction.commit()?;
    let mut warnings = parsed.warnings;
    if repairing {
        warnings.push(
            "This file was already imported. Worthweave repaired missing investment links without duplicating transactions."
                .into(),
        );
    }
    Ok(ImportResult {
        batch_id,
        coverage_start: parsed.start.to_string(),
        coverage_end: parsed.end.to_string(),
        events_added,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv_files(directory: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        let Ok(entries) = std::fs::read_dir(directory) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(csv_files(&path));
            } else if path.extension().and_then(|value| value.to_str()) == Some("csv") {
                files.push(path);
            }
        }
        files
    }

    #[test]
    fn trading212_parser_preserves_decimal_values() {
        let parsed = parse_trading212(b"Action,Time,ISIN,Ticker,ID,No. of shares,Total,Currency (Total)\nMarket buy,2026-07-01 10:00:00,GB00TEST0001,TEST,T1,1.25,10.50,GBP\n").expect("valid export");
        assert_eq!(parsed.events[0].event_type, "buy");
        assert_eq!(
            parsed.events[0].amount,
            Some(ExactValue {
                coefficient: "1050".into(),
                scale: 2
            })
        );
        assert_eq!(
            parsed.events[0].quantity,
            Some(ExactValue {
                coefficient: "125".into(),
                scale: 2
            })
        );
    }

    #[test]
    fn ibkr_parser_normalizes_flex_headers() {
        let parsed = parse_ibkr(b"ClientAccountID,CurrencyPrimary,AccountType,DateOpened\nU1,GBP,Individual,2024-03-15\nClientAccountID,CurrencyPrimary,TradeID,Buy/Sell,TradeMoney,Date/Time,Quantity,NetCash,Description,ISIN\nU1,GBP,T1,BUY,100.00,2024-03-19;10:30:00,2,-101.00,Example,GB00TEST0001\n").expect("valid flex export");
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].event_type, "buy");
        assert_eq!(
            parsed.events[0].amount,
            Some(ExactValue {
                coefficient: "10000".into(),
                scale: 2
            })
        );
        assert_eq!(parsed.start.to_string(), "2024-03-15");
    }

    #[test]
    fn ibkr_parser_distinguishes_tax_from_fees() {
        let parsed = parse_ibkr(b"ClientAccountID,CurrencyPrimary,TaxDescription,TaxAmount,TradeID,Date\nU1,GBP,UK stamp duty,-2.50,T1,2026-07-01\n").expect("valid tax export");
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].event_type, "tax");
        assert_eq!(
            parsed.events[0].amount,
            Some(ExactValue {
                coefficient: "-250".into(),
                scale: 2
            })
        );
    }

    #[test]
    fn ibkr_parser_captures_summary_position_snapshots() {
        let parsed = parse_ibkr(b"ClientAccountID,CurrencyPrimary,ReportDate,Quantity,MarkPrice,PositionValue,CostBasisMoney,LevelOfDetail,ISIN,Conid\nU1,GBP,2026-07-10,2.5,10,25,20,Summary,GB00TEST0001,123\n").expect("valid position section");
        assert_eq!(parsed.positions.len(), 1);
        assert_eq!(parsed.positions[0].instrument_id, "GB00TEST0001");
        assert_eq!(
            parsed.positions[0].quantity,
            ExactValue {
                coefficient: "25".into(),
                scale: 1
            }
        );
    }

    #[test]
    fn ibkr_parser_links_symbol_only_trades_to_snapshot_identity() {
        let parsed = parse_ibkr(b"ClientAccountID,CurrencyPrimary,TradeID,Buy/Sell,TradeMoney,Date/Time,Quantity,NetCash,Description,Symbol,ISIN\nU1,USD,T1,BUY,20.00,2026-07-01;10:00:00,2,-20.00,Example,TEST,\nClientAccountID,CurrencyPrimary,ReportDate,Quantity,MarkPrice,PositionValue,CostBasisMoney,LevelOfDetail,Symbol,Description,ISIN,Conid,AssetClass\nU1,USD,2026-07-10,2,10,20,20,Summary,TEST,Example,US00TEST0001,123,STK\n").expect("valid flex export");
        assert_eq!(
            parsed.events[0].instrument_id.as_deref(),
            Some("US00TEST0001")
        );
        assert_eq!(
            parsed.positions[0]
                .market_price
                .as_ref()
                .map(|price| price.coefficient.as_str()),
            Some("10")
        );
        assert_eq!(parsed.positions[0].price_currency.as_deref(), Some("USD"));
    }

    #[test]
    fn ibkr_import_persists_reconcilable_positions_atomically() {
        let directory = tempfile::tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &crate::models::CreateAccountInput {
                broker: "ibkr".into(),
                jurisdiction: "GB".into(),
                account_type: "invest".into(),
                display_name: "IBKR Invest".into(),
            },
        )
        .expect("account");
        let path = directory.path().join("ibkr.csv");
        std::fs::write(
            &path,
            "ClientAccountID,CurrencyPrimary,TradeID,Buy/Sell,TradeMoney,Date/Time,Quantity,NetCash,Description,Symbol,ISIN,AssetClass\nU1,GBP,T1,BUY,20.00,2026-07-01;10:00:00,2,-20.00,Example holding,TEST,GB00TEST0001,STK\nClientAccountID,CurrencyPrimary,ReportDate,Quantity,MarkPrice,PositionValue,CostBasisMoney,LevelOfDetail,Symbol,Description,ISIN,Conid,AssetClass\nU1,GBP,2026-07-10,2,10,20,20,Summary,TEST,Example holding,GB00TEST0001,123,STK\n",
        )
        .expect("export");
        let result = import_csv(&mut connection, &account.id, &path, "invest").expect("import");
        assert_eq!(result.events_added, 1);
        let reconciliation =
            crate::projections::reconciliation(&connection).expect("reconciliation");
        assert_eq!(reconciliation.len(), 1);
        assert_eq!(reconciliation[0].status, "matched");
        connection
            .execute("UPDATE events SET instrument_id='symbol:TEST'", [])
            .expect("simulate legacy symbol fallback");
        let repaired =
            import_csv(&mut connection, &account.id, &path, "invest").expect("idempotent repair");
        assert_eq!(repaired.events_added, 0);
        assert!(
            repaired
                .warnings
                .iter()
                .any(|warning| warning.contains("repaired"))
        );
        let repaired_id: Option<String> = connection
            .query_row("SELECT instrument_id FROM events LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("repaired instrument id");
        assert_eq!(repaired_id.as_deref(), Some("GB00TEST0001"));
        let imported_price: (String, String) = connection
            .query_row(
                "SELECT price_coefficient, currency FROM market_prices WHERE instrument_id='GB00TEST0001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("broker price");
        assert_eq!(imported_price, ("10".into(), "GBP".into()));
    }

    #[test]
    fn robinhood_import_rejects_unvalidated_export_schemas() {
        let directory = tempfile::tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &crate::models::CreateAccountInput {
                broker: "robinhood".into(),
                jurisdiction: "US".into(),
                account_type: "individual_brokerage".into(),
                display_name: "Robinhood Individual".into(),
            },
        )
        .expect("account");
        let path = directory.path().join("robinhood.csv");
        std::fs::write(
            &path,
            "Activity Date,Trans Code,Instrument,Quantity,Amount\n2026-01-01,Buy,HOOD,1,-10.00\n",
        )
        .expect("export");
        assert!(matches!(
            import_csv(&mut connection, &account.id, &path, "individual_brokerage"),
            Err(WorthweaveError::UnsupportedBrokerImport)
        ));
    }

    #[test]
    fn local_broker_exports_parse_when_present() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("project root");
        for path in csv_files(&project_root.join(".dev").join("ibkr")) {
            let content = std::fs::read(&path).expect("read local IBKR export");
            let parsed =
                parse_ibkr(&content).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(
                parsed
                    .events
                    .iter()
                    .all(|event| !matches!(event.event_type, "buy" | "sell")
                        || event.instrument_id.is_some()),
                "{} contains a trade that could not be linked to an investment",
                path.display()
            );
        }
        for path in csv_files(&project_root.join(".dev").join("trading212")) {
            let content = std::fs::read(&path).expect("read local Trading 212 export");
            parse_trading212(&content)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        }
    }
}
