use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use csv::StringRecord;
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db;
use crate::error::{LedgerlyError, Result};
use crate::models::ImportResult;

const MAX_IMPORT_BYTES: u64 = 50 * 1024 * 1024;

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
        .map_err(|_| LedgerlyError::Csv(format!("invalid date in {context}")))
}

fn decimal(value: Option<&str>, context: &str) -> Result<Option<Decimal>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    Decimal::from_str(value.replace(',', "").as_str())
        .map(Some)
        .map_err(|_| LedgerlyError::Csv(format!("invalid decimal in {context}")))
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
        .map_err(|_| LedgerlyError::Csv("Trading 212 export must be UTF-8 CSV".into()))?
        .trim_start_matches('\u{feff}');
    let mut reader = csv::ReaderBuilder::new().from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| LedgerlyError::Csv(error.to_string()))?
        .clone();
    for required in ["Action", "Time", "ID"] {
        if !headers.iter().any(|header| header == required) {
            return Err(LedgerlyError::Csv(format!(
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
            record.map_err(|error| LedgerlyError::Csv(format!("row {}: {error}", offset + 2)))?;
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
        });
    }
    let start = dates
        .iter()
        .min()
        .copied()
        .ok_or_else(|| LedgerlyError::Csv("Trading 212 export contains no data rows".into()))?;
    let end = dates.iter().max().copied().expect("non-empty dates");
    Ok(ParsedImport {
        start,
        end,
        events,
        warnings: Vec::new(),
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
    if has(&["TradeID", "BuySell", "TradeMoney"]) {
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
        return "fee";
    }
    let label = format!(
        "{} {}",
        row.get("Type").map_or("", String::as_str),
        row.get("Description").map_or("", String::as_str)
    )
    .to_lowercase();
    if label.contains("dividend") {
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

fn parse_ibkr(content: &[u8]) -> Result<ParsedImport> {
    let text = std::str::from_utf8(content)
        .map_err(|_| LedgerlyError::Csv("IBKR export must be UTF-8 CSV".into()))?
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
    for (offset, record) in reader.records().enumerate() {
        let values =
            record.map_err(|error| LedgerlyError::Csv(format!("row {}: {error}", offset + 1)))?;
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
            LedgerlyError::Csv("IBKR export does not begin with a recognized section header".into())
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
        let amount_raw = ["Amount", "NetCash", "CashTransfer", "TaxAmount", "CommTax"]
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
            instrument_id: row
                .get("ISIN")
                .or_else(|| row.get("Conid"))
                .filter(|value| !value.is_empty())
                .cloned(),
        });
    }
    let start = dates
        .iter()
        .min()
        .copied()
        .ok_or_else(|| LedgerlyError::Csv("IBKR export contains no dated data rows".into()))?;
    let end = dates.iter().max().copied().expect("non-empty dates");
    let warnings = if ignored {
        vec!["Non-transaction statement sections were retained for coverage detection but not ledger events.".into()]
    } else {
        Vec::new()
    };
    Ok(ParsedImport {
        start,
        end,
        events,
        warnings,
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
        return Err(LedgerlyError::UnsupportedFile);
    }
    if std::fs::metadata(path)?.len() > MAX_IMPORT_BYTES {
        return Err(LedgerlyError::ImportTooLarge);
    }
    let (broker, account_type) =
        db::account_identity(connection, account_id)?.ok_or(LedgerlyError::AccountNotFound)?;
    if account_type != confirmed_account_type {
        return Err(LedgerlyError::AccountTypeMismatch);
    }
    let content = std::fs::read(path)?;
    let digest = hex(&Sha256::digest(&content));
    let duplicate: Option<String> = connection
        .query_row(
            "SELECT id FROM import_batches WHERE account_id = ?1 AND content_sha256 = ?2",
            params![account_id, digest],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Err(LedgerlyError::DuplicateImport);
    }
    let parsed = if broker == "trading_212" {
        parse_trading212(&content)?
    } else {
        parse_ibkr(&content)?
    };
    let existing: HashSet<String> = {
        let mut statement =
            connection.prepare("SELECT source_id FROM events WHERE account_id = ?1")?;
        statement
            .query_map([account_id], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?
    };
    let new_events: Vec<_> = parsed
        .events
        .into_iter()
        .filter(|event| !existing.contains(&event.source_id))
        .collect();
    let batch_id = Uuid::new_v4().to_string();
    let transaction = connection.transaction()?;
    transaction.execute("INSERT INTO import_batches (id, account_id, original_filename, content_sha256, coverage_start, coverage_end) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![batch_id, account_id, path.file_name().and_then(|value| value.to_str()).unwrap_or("broker-export.csv"), digest, parsed.start.to_string(), parsed.end.to_string()])?;
    for event in &new_events {
        transaction.execute("INSERT INTO events (id, account_id, import_batch_id, source_id, event_type, occurred_at, description, amount_coefficient, amount_scale, currency, quantity_coefficient, quantity_scale, instrument_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)", params![Uuid::new_v4().to_string(), account_id, batch_id, event.source_id, event.event_type, event.occurred_at, event.description, event.amount.as_ref().map(|value| &value.coefficient), event.amount.as_ref().map(|value| value.scale), event.currency, event.quantity.as_ref().map(|value| &value.coefficient), event.quantity.as_ref().map(|value| value.scale), event.instrument_id])?;
    }
    transaction.commit()?;
    Ok(ImportResult {
        batch_id,
        coverage_start: parsed.start.to_string(),
        coverage_end: parsed.end.to_string(),
        events_added: new_events.len(),
        warnings: parsed.warnings,
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
        assert_eq!(parsed.start.to_string(), "2024-03-15");
    }

    #[test]
    fn local_broker_exports_parse_when_present() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("project root");
        for path in csv_files(&project_root.join(".dev").join("ibkr")) {
            let content = std::fs::read(&path).expect("read local IBKR export");
            parse_ibkr(&content).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        }
        for path in csv_files(&project_root.join(".dev").join("trading212")) {
            let content = std::fs::read(&path).expect("read local Trading 212 export");
            parse_trading212(&content)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        }
    }
}
