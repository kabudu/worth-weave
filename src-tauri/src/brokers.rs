use std::io::Write;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::header::RETRY_AFTER;
use reqwest::{Client, StatusCode, Url};
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::Builder;
use uuid::Uuid;

use crate::error::{Result, WorthweaveError};
use crate::imports;
use crate::models::{BrokerConnectionStatus, BrokerSyncResult, ConnectTrading212Input};

const KEYCHAIN_SERVICE: &str = "com.worthweave.app.broker.trading212";
const LIVE_BASE: &str = "https://live.trading212.com/api/v0/";
const DEMO_BASE: &str = "https://demo.trading212.com/api/v0/";
const MAX_DOWNLOAD_BYTES: usize = 50 * 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
struct Credentials {
    api_key: String,
    api_secret: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountSummary {
    id: i64,
    currency: String,
}

pub(crate) struct SyncPlan {
    account_id: String,
    account_type: String,
    environment: String,
    pending_report: Option<String>,
    coverage_end: Option<String>,
    credentials: Credentials,
}

pub(crate) enum SyncFetch {
    Requested(String),
    Preparing {
        coverage_start: Option<String>,
        coverage_end: Option<String>,
    },
    Ready {
        csv: Vec<u8>,
        positions: Vec<Position>,
        coverage_start: String,
        coverage_end: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnqueuedReport {
    report_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    report_id: i64,
    status: String,
    download_link: Option<String>,
    time_from: Option<String>,
    time_to: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiInstrument {
    currency: Option<String>,
    isin: Option<String>,
    name: Option<String>,
    ticker: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletImpact {
    currency: Option<String>,
    current_value: Option<Value>,
    total_cost: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Position {
    current_price: Option<Value>,
    instrument: ApiInstrument,
    quantity: Value,
    wallet_impact: Option<WalletImpact>,
}

fn keychain_entry(account_id: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, account_id).map_err(|error| {
        WorthweaveError::BrokerSync(format!("macOS Keychain is unavailable: {error}"))
    })
}

fn base_url(environment: &str) -> Result<&'static str> {
    match environment {
        "live" => Ok(LIVE_BASE),
        "demo" => Ok(DEMO_BASE),
        _ => Err(WorthweaveError::BrokerSync(
            "Choose the Trading 212 live or practice environment".into(),
        )),
    }
}

fn client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("Worthweave/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| WorthweaveError::BrokerSync("Could not prepare the secure connection".into()))
}

async fn response<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T> {
    let response = request.send().await.map_err(|error| {
        if error.is_timeout() {
            WorthweaveError::BrokerSync(
                "Trading 212 took too long to respond. Your existing data is unchanged".into(),
            )
        } else {
            WorthweaveError::BrokerSync(
                "Trading 212 is currently unreachable. Your existing data is unchanged".into(),
            )
        }
    })?;
    match response.status() {
        StatusCode::UNAUTHORIZED => Err(WorthweaveError::BrokerSync(
            "Trading 212 did not accept this API key and secret".into(),
        )),
        StatusCode::FORBIDDEN => Err(WorthweaveError::BrokerSync(
            "The API key needs account, portfolio and history read permissions".into(),
        )),
        StatusCode::TOO_MANY_REQUESTS => {
            let retry_after_seconds = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| {
                    value.parse::<u64>().ok().or_else(|| {
                        DateTime::parse_from_rfc2822(value).ok().map(|at| {
                            (at.with_timezone(&Utc) - Utc::now()).num_seconds().max(1) as u64
                        })
                    })
                })
                .unwrap_or(60)
                .clamp(5, 15 * 60);
            Err(WorthweaveError::BrokerRateLimited {
                retry_after_seconds,
                message: "Trading 212 has temporarily limited requests. Worthweave will preserve your place and wait before trying again".into(),
            })
        }
        status if status.is_server_error() => Err(WorthweaveError::BrokerSync(
            "Trading 212 is temporarily unavailable. Your existing data is unchanged".into(),
        )),
        status if !status.is_success() => Err(WorthweaveError::BrokerSync(format!(
            "Trading 212 rejected the sync request ({status})"
        ))),
        _ => response.json().await.map_err(|_| {
            WorthweaveError::BrokerSync("Trading 212 returned an unreadable response".into())
        }),
    }
}

fn credentials(account_id: &str) -> Result<Credentials> {
    let stored = keychain_entry(account_id)?
        .get_password()
        .map_err(|error| match error {
            keyring::Error::NoEntry => {
                WorthweaveError::BrokerSync("Connect this Trading 212 account first".into())
            }
            other => WorthweaveError::BrokerSync(format!(
                "Could not read the connection from macOS Keychain: {other}"
            )),
        })?;
    serde_json::from_str(&stored)
        .map_err(|_| WorthweaveError::BrokerSync("The stored broker connection is invalid".into()))
}

fn ensure_trading212_account(connection: &Connection, account_id: &str) -> Result<String> {
    connection
        .query_row(
            "SELECT account_type FROM accounts WHERE id=?1 AND broker='trading_212'",
            [account_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            WorthweaveError::BrokerSync("Choose a Trading 212 Invest or ISA account".into())
        })
}

pub fn validate_account(connection: &Connection, account_id: &str) -> Result<()> {
    ensure_trading212_account(connection, account_id).map(|_| ())
}

pub fn record_error(
    connection: &Connection,
    account_id: &str,
    message: &str,
    retry_after_seconds: Option<u64>,
) -> Result<()> {
    let retry_after_at = retry_after_seconds
        .map(|seconds| (Utc::now() + chrono::Duration::seconds(seconds as i64)).to_rfc3339());
    connection.execute(
        "UPDATE broker_connections SET last_error=?2, retry_after_at=?3,
         updated_at=CURRENT_TIMESTAMP WHERE account_id=?1",
        params![account_id, message, retry_after_at],
    )?;
    Ok(())
}

fn sync_state(
    configured: bool,
    pending: bool,
    last_success: bool,
    last_error: bool,
) -> &'static str {
    if !configured {
        "disconnected"
    } else if last_error {
        "attention"
    } else if pending {
        "preparing"
    } else if last_success {
        "current"
    } else {
        "ready"
    }
}

pub fn statuses(connection: &Connection) -> Result<Vec<BrokerConnectionStatus>> {
    let mut statement = connection.prepare(
        "SELECT a.id, COALESCE(c.environment, 'live'), c.external_account_id,
                c.pending_report_id, c.last_success_at, c.last_error, c.retry_after_at
         FROM accounts a LEFT JOIN broker_connections c ON c.account_id=a.id
         WHERE a.broker='trading_212' ORDER BY a.created_at, a.id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;
    let mut output = Vec::new();
    for row in rows {
        let (
            account_id,
            environment,
            external_account_id,
            pending,
            last_success_at,
            last_error,
            retry_after_at,
        ) = row?;
        let configured = match keychain_entry(&account_id)?.get_password() {
            Ok(value) => !value.is_empty(),
            Err(keyring::Error::NoEntry) => false,
            Err(error) => {
                return Err(WorthweaveError::BrokerSync(format!(
                    "Could not read macOS Keychain: {error}"
                )));
            }
        };
        let sync_state = sync_state(
            configured,
            pending.is_some(),
            last_success_at.is_some(),
            last_error.is_some(),
        );
        output.push(BrokerConnectionStatus {
            account_id,
            configured,
            environment,
            external_account_id,
            last_success_at,
            last_error,
            retry_after_at,
            sync_state: sync_state.into(),
        });
    }
    Ok(output)
}

pub async fn verify_connection(input: &ConnectTrading212Input) -> Result<AccountSummary> {
    let key = input.api_key.trim();
    let secret = input.api_secret.trim();
    if key.is_empty() || secret.is_empty() || key.len() > 512 || secret.len() > 512 {
        return Err(WorthweaveError::BrokerSync(
            "Enter the complete Trading 212 API key and secret".into(),
        ));
    }
    let base = base_url(&input.environment)?;
    response(
        client()?
            .get(format!("{base}equity/account/summary"))
            .basic_auth(key, Some(secret)),
    )
    .await
}

pub fn save_connection(
    connection: &mut Connection,
    input: &ConnectTrading212Input,
    summary: AccountSummary,
) -> Result<BrokerConnectionStatus> {
    ensure_trading212_account(connection, &input.account_id)?;
    let key = input.api_key.trim();
    let secret = input.api_secret.trim();
    let currency = summary.currency.trim().to_uppercase();
    if currency.len() != 3 {
        return Err(WorthweaveError::BrokerSync(
            "Trading 212 returned an invalid account currency".into(),
        ));
    }
    let encoded = serde_json::to_string(&Credentials {
        api_key: key.into(),
        api_secret: secret.into(),
    })
    .map_err(|_| WorthweaveError::BrokerSync("Could not protect the connection".into()))?;
    keychain_entry(&input.account_id)?
        .set_password(&encoded)
        .map_err(|error| {
            WorthweaveError::BrokerSync(format!(
                "Could not save the connection in macOS Keychain: {error}"
            ))
        })?;
    connection.execute(
        "INSERT INTO broker_connections
         (account_id, provider, environment, external_account_id, last_error)
         VALUES (?1, 'trading_212', ?2, ?3, NULL)
         ON CONFLICT(account_id) DO UPDATE SET environment=excluded.environment,
           external_account_id=excluded.external_account_id, last_error=NULL,
           updated_at=CURRENT_TIMESTAMP",
        params![input.account_id, input.environment, summary.id.to_string()],
    )?;
    connection.execute(
        "UPDATE accounts SET base_currency=?2 WHERE id=?1",
        params![input.account_id, currency],
    )?;
    statuses(connection)?
        .into_iter()
        .find(|status| status.account_id == input.account_id)
        .ok_or(WorthweaveError::AccountNotFound)
}

pub fn disconnect(connection: &Connection, account_id: &str) -> Result<()> {
    ensure_trading212_account(connection, account_id)?;
    match keychain_entry(account_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(error) => {
            return Err(WorthweaveError::BrokerSync(format!(
                "Could not remove the connection from macOS Keychain: {error}"
            )));
        }
    }
    connection.execute(
        "DELETE FROM broker_connections WHERE account_id=?1",
        [account_id],
    )?;
    Ok(())
}

fn decimal(value: &Value, field: &str) -> Result<Decimal> {
    Decimal::from_str(&value.to_string()).map_err(|_| {
        WorthweaveError::BrokerSync(format!("Trading 212 returned an invalid {field}"))
    })
}

fn exact(value: Decimal) -> (String, u32) {
    let value = value.normalize();
    (value.mantissa().to_string(), value.scale())
}

fn save_positions(
    connection: &Connection,
    account_id: &str,
    batch_id: &str,
    positions: &[Position],
) -> Result<usize> {
    let report_date = Utc::now().date_naive().to_string();
    let mut updated = 0;
    for position in positions {
        let ticker = position.instrument.ticker.as_deref().unwrap_or("").trim();
        let instrument_id = position
            .instrument
            .isin
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(ticker);
        if instrument_id.is_empty() || instrument_id.len() > 128 {
            continue;
        }
        let quantity = decimal(&position.quantity, "position quantity")?;
        let (quantity_coefficient, quantity_scale) = exact(quantity);
        let price = position
            .current_price
            .as_ref()
            .map(|value| decimal(value, "position price"))
            .transpose()?;
        let cost_basis = position
            .wallet_impact
            .as_ref()
            .and_then(|impact| impact.total_cost.as_ref())
            .map(|value| decimal(value, "position cost"))
            .transpose()?;
        let current_value = position
            .wallet_impact
            .as_ref()
            .and_then(|impact| impact.current_value.as_ref())
            .map(|value| decimal(value, "position value"))
            .transpose()?;
        let value_currency = position
            .wallet_impact
            .as_ref()
            .and_then(|impact| impact.currency.as_deref())
            .map(str::to_uppercase);
        let price_currency = position
            .instrument
            .currency
            .as_deref()
            .map(str::to_uppercase);
        connection.execute(
            "INSERT INTO instruments (id, symbol, name, isin, asset_class)
             VALUES (?1, ?2, ?3, ?4, 'STK') ON CONFLICT(id) DO UPDATE SET
               symbol=COALESCE(instruments.symbol, excluded.symbol),
               name=COALESCE(instruments.name, excluded.name),
               isin=COALESCE(excluded.isin, instruments.isin), updated_at=CURRENT_TIMESTAMP",
            params![
                instrument_id,
                (!ticker.is_empty()).then_some(ticker),
                position.instrument.name,
                position.instrument.isin
            ],
        )?;
        if let (Some(price), Some(currency)) = (price, price_currency.as_deref()) {
            let (coefficient, scale) = exact(price);
            connection.execute(
                "INSERT INTO market_prices
                 (instrument_id, price_coefficient, price_scale, currency, as_of, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'trading_212_api')
                 ON CONFLICT(instrument_id) DO UPDATE SET price_coefficient=excluded.price_coefficient,
                   price_scale=excluded.price_scale, currency=excluded.currency,
                   as_of=excluded.as_of, source=excluded.source",
                params![instrument_id, coefficient, scale, currency, report_date],
            )?;
        }
        let cost = cost_basis.map(exact);
        let value = current_value.map(exact);
        connection.execute(
            "INSERT INTO broker_position_snapshots
             (id, account_id, import_batch_id, report_date, instrument_id,
              quantity_coefficient, quantity_scale, cost_basis_coefficient,
              cost_basis_scale, cost_basis_currency, position_value_coefficient,
              position_value_scale, position_value_currency)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(account_id, report_date, instrument_id) DO UPDATE SET
               import_batch_id=excluded.import_batch_id,
               quantity_coefficient=excluded.quantity_coefficient,
               quantity_scale=excluded.quantity_scale,
               cost_basis_coefficient=excluded.cost_basis_coefficient,
               cost_basis_scale=excluded.cost_basis_scale,
               cost_basis_currency=excluded.cost_basis_currency,
               position_value_coefficient=excluded.position_value_coefficient,
               position_value_scale=excluded.position_value_scale,
               position_value_currency=excluded.position_value_currency",
            params![
                Uuid::new_v4().to_string(),
                account_id,
                batch_id,
                report_date,
                instrument_id,
                quantity_coefficient,
                quantity_scale,
                cost.as_ref().map(|item| &item.0),
                cost.as_ref().map(|item| item.1),
                value_currency,
                value.as_ref().map(|item| &item.0),
                value.as_ref().map(|item| item.1),
                value_currency
            ],
        )?;
        updated += 1;
    }
    Ok(updated)
}

async fn download_csv(client: &Client, url: &str) -> Result<Vec<u8>> {
    let url = Url::parse(url)
        .map_err(|_| WorthweaveError::BrokerSync("The report download link is invalid".into()))?;
    if url.scheme() != "https" {
        return Err(WorthweaveError::BrokerSync(
            "Trading 212 returned an insecure report link".into(),
        ));
    }
    let response = client.get(url).send().await.map_err(|_| {
        WorthweaveError::BrokerSync("The Trading 212 report could not be downloaded".into())
    })?;
    if !response.status().is_success() {
        return Err(WorthweaveError::BrokerSync(
            "The Trading 212 report is not ready to download".into(),
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size as usize > MAX_DOWNLOAD_BYTES)
    {
        return Err(WorthweaveError::ImportTooLarge);
    }
    let bytes = response.bytes().await.map_err(|_| {
        WorthweaveError::BrokerSync("The Trading 212 report download was interrupted".into())
    })?;
    if bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err(WorthweaveError::ImportTooLarge);
    }
    Ok(bytes.to_vec())
}

pub fn prepare_sync(connection: &Connection, account_id: &str) -> Result<SyncPlan> {
    let account_type = ensure_trading212_account(connection, account_id)?;
    let (environment, pending_report, retry_after_at): (String, Option<String>, Option<String>) =
        connection
            .query_row(
                "SELECT environment, pending_report_id, retry_after_at
             FROM broker_connections WHERE account_id=?1",
                [account_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| WorthweaveError::BrokerSync("Connect this account first".into()))?;
    if let Some(retry_after_at) = retry_after_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
    {
        let remaining = (retry_after_at.with_timezone(&Utc) - Utc::now()).num_seconds();
        if remaining > 0 {
            return Err(WorthweaveError::BrokerRateLimited {
                retry_after_seconds: remaining as u64,
                message: "Trading 212 is still cooling down. No request was sent".into(),
            });
        }
    }
    let coverage_end: Option<String> = connection.query_row(
        "SELECT MAX(coverage_end) FROM import_batches WHERE account_id=?1",
        [account_id],
        |row| row.get(0),
    )?;
    Ok(SyncPlan {
        account_id: account_id.into(),
        account_type,
        environment,
        pending_report,
        coverage_end,
        credentials: credentials(account_id)?,
    })
}

pub async fn fetch_sync(plan: &SyncPlan) -> Result<SyncFetch> {
    let base = base_url(&plan.environment)?;
    let client = client()?;
    let Some(report_id) = plan.pending_report.as_deref() else {
        let from = plan
            .coverage_end
            .as_deref()
            .and_then(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .and_then(|date| date.pred_opt())
            .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2019, 1, 1).expect("valid date"));
        let request = serde_json::json!({
            "dataIncluded": {
                "includeDividends": true,
                "includeInterest": true,
                "includeOrders": true,
                "includeTransactions": true
            },
            "timeFrom": format!("{from}T00:00:00Z"),
            "timeTo": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
        });
        let queued: EnqueuedReport = response(
            client
                .post(format!("{base}equity/history/exports"))
                .basic_auth(
                    &plan.credentials.api_key,
                    Some(&plan.credentials.api_secret),
                )
                .json(&request),
        )
        .await?;
        return Ok(SyncFetch::Requested(queued.report_id.to_string()));
    };

    let reports: Vec<Report> = response(
        client
            .get(format!("{base}equity/history/exports"))
            .basic_auth(
                &plan.credentials.api_key,
                Some(&plan.credentials.api_secret),
            ),
    )
    .await?;
    let report = reports
        .into_iter()
        .find(|report| report.report_id.to_string() == report_id)
        .ok_or_else(|| WorthweaveError::BrokerSync("Trading 212 is preparing the report".into()))?;
    if report.status != "Finished" {
        if matches!(report.status.as_str(), "Failed" | "Canceled") {
            return Err(WorthweaveError::BrokerSync(
                "Trading 212 could not prepare the history report. Try again later".into(),
            ));
        }
        return Ok(SyncFetch::Preparing {
            coverage_start: report
                .time_from
                .map(|value| value.chars().take(10).collect()),
            coverage_end: report.time_to.map(|value| value.chars().take(10).collect()),
        });
    }
    let coverage_start = report
        .time_from
        .as_deref()
        .map(|value| value.chars().take(10).collect())
        .unwrap_or_else(|| "2019-01-01".into());
    let coverage_end = report
        .time_to
        .as_deref()
        .map(|value| value.chars().take(10).collect())
        .unwrap_or_else(|| Utc::now().date_naive().to_string());
    let download_link = report.download_link.ok_or_else(|| {
        WorthweaveError::BrokerSync("The completed Trading 212 report has no download link".into())
    })?;
    let csv = download_csv(&client, &download_link).await?;
    let positions: Vec<Position> =
        response(client.get(format!("{base}equity/positions")).basic_auth(
            &plan.credentials.api_key,
            Some(&plan.credentials.api_secret),
        ))
        .await?;
    Ok(SyncFetch::Ready {
        csv,
        positions,
        coverage_start,
        coverage_end,
    })
}

pub fn save_sync(
    connection: &mut Connection,
    plan: SyncPlan,
    fetched: SyncFetch,
) -> Result<BrokerSyncResult> {
    if let SyncFetch::Requested(report_id) = fetched {
        connection.execute(
            "UPDATE broker_connections SET pending_report_id=?2, last_error=NULL, retry_after_at=NULL,
             updated_at=CURRENT_TIMESTAMP WHERE account_id=?1",
            params![plan.account_id, report_id],
        )?;
        return Ok(BrokerSyncResult {
            account_id: plan.account_id,
            state: "preparing".into(),
            events_added: 0,
            positions_updated: 0,
            coverage_start: None,
            coverage_end: None,
            message: "Trading 212 is preparing your history. Worthweave will check again shortly"
                .into(),
        });
    }
    if let SyncFetch::Preparing {
        coverage_start,
        coverage_end,
    } = fetched
    {
        return Ok(BrokerSyncResult {
            account_id: plan.account_id,
            state: "preparing".into(),
            events_added: 0,
            positions_updated: 0,
            coverage_start,
            coverage_end,
            message:
                "Trading 212 is still preparing your history. Worthweave will check again shortly"
                    .into(),
        });
    }
    let SyncFetch::Ready {
        csv,
        positions,
        coverage_start: requested_start,
        coverage_end: requested_end,
    } = fetched
    else {
        unreachable!()
    };
    let mut file = Builder::new()
        .prefix("worthweave-t212-")
        .suffix(".csv")
        .tempfile()?;
    file.write_all(&csv)?;
    file.flush()?;
    let has_rows = csv::Reader::from_reader(csv.as_slice())
        .records()
        .next()
        .transpose()
        .map_err(|error| WorthweaveError::Csv(error.to_string()))?
        .is_some();
    let (batch_id, coverage_start, coverage_end, events_added) = if has_rows {
        let result = imports::import_csv(
            connection,
            &plan.account_id,
            file.path(),
            &plan.account_type,
        )?;
        (
            result.batch_id,
            result.coverage_start,
            result.coverage_end,
            result.events_added,
        )
    } else {
        let digest = Sha256::digest(&csv)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let existing: Option<String> = connection
            .query_row(
                "SELECT id FROM import_batches WHERE account_id=?1 AND content_sha256=?2",
                params![plan.account_id, digest],
                |row| row.get(0),
            )
            .optional()?;
        let batch_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        connection.execute(
            "INSERT OR IGNORE INTO import_batches
             (id, account_id, original_filename, content_sha256, coverage_start, coverage_end)
             VALUES (?1, ?2, 'trading-212-api-snapshot.csv', ?3, ?4, ?5)",
            params![
                batch_id,
                plan.account_id,
                digest,
                requested_start,
                requested_end
            ],
        )?;
        (batch_id, requested_start, requested_end, 0)
    };
    let transaction = connection.transaction()?;
    let positions_updated = save_positions(&transaction, &plan.account_id, &batch_id, &positions)?;
    transaction.execute(
        "UPDATE broker_connections SET pending_report_id=NULL, last_success_at=CURRENT_TIMESTAMP,
         last_error=NULL, retry_after_at=NULL, updated_at=CURRENT_TIMESTAMP WHERE account_id=?1",
        [&plan.account_id],
    )?;
    transaction.commit()?;
    Ok(BrokerSyncResult {
        account_id: plan.account_id,
        state: "complete".into(),
        events_added,
        positions_updated,
        coverage_start: Some(coverage_start),
        coverage_end: Some(coverage_end),
        message: "Trading 212 is up to date".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn only_known_environments_are_accepted() {
        assert_eq!(base_url("live").expect("live"), LIVE_BASE);
        assert_eq!(base_url("demo").expect("demo"), DEMO_BASE);
        assert!(base_url("https://example.com").is_err());
    }

    #[test]
    fn decimal_values_remain_exact() {
        assert_eq!(
            exact(decimal(&serde_json::json!(12.345), "test").expect("decimal")),
            ("12345".into(), 3)
        );
    }

    #[test]
    fn a_failed_finished_report_takes_precedence_over_preparing() {
        assert_eq!(sync_state(true, true, false, true), "attention");
        assert_eq!(sync_state(true, true, false, false), "preparing");
    }

    #[test]
    fn active_cooldown_prevents_another_broker_request() {
        let directory = tempdir().expect("temp directory");
        let connection = crate::db::open(&directory.path().join("test.db")).expect("database");
        let account = crate::db::create_account(
            &connection,
            &crate::models::CreateAccountInput {
                broker: "trading_212".into(),
                jurisdiction: "GB".into(),
                account_type: "invest".into(),
                display_name: "Trading 212 Invest".into(),
            },
        )
        .expect("account");
        connection
            .execute(
                "INSERT INTO broker_connections
                 (account_id, provider, environment, retry_after_at)
                 VALUES (?1, 'trading_212', 'live', ?2)",
                params![
                    account.id,
                    (Utc::now() + chrono::Duration::minutes(2)).to_rfc3339()
                ],
            )
            .expect("connection");

        let error = match prepare_sync(&connection, &account.id) {
            Ok(_) => panic!("cooldown should block"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            WorthweaveError::BrokerRateLimited {
                retry_after_seconds: 1..,
                ..
            }
        ));
    }

    #[test]
    fn position_price_keeps_instrument_currency_while_basis_keeps_account_currency() {
        let directory = tempdir().expect("temp directory");
        let connection = crate::db::open(&directory.path().join("test.db")).expect("database");
        let account = crate::db::create_account(
            &connection,
            &crate::models::CreateAccountInput {
                broker: "trading_212".into(),
                jurisdiction: "GB".into(),
                account_type: "invest".into(),
                display_name: "Trading 212 Invest".into(),
            },
        )
        .expect("account");
        connection
            .execute(
                "INSERT INTO import_batches
             (id, account_id, original_filename, content_sha256, coverage_start, coverage_end)
             VALUES ('batch', ?1, 'api.csv', 'digest', '2026-01-01', '2026-01-02')",
                [&account.id],
            )
            .expect("batch");
        let positions = vec![Position {
            current_price: Some(serde_json::json!(200.5)),
            instrument: ApiInstrument {
                currency: Some("USD".into()),
                isin: Some("US0378331005".into()),
                name: Some("Apple Inc.".into()),
                ticker: Some("AAPL_US_EQ".into()),
            },
            quantity: serde_json::json!(2),
            wallet_impact: Some(WalletImpact {
                currency: Some("GBP".into()),
                current_value: Some(serde_json::json!(310)),
                total_cost: Some(serde_json::json!(250)),
            }),
        }];
        assert_eq!(
            save_positions(&connection, &account.id, "batch", &positions).expect("positions"),
            1
        );
        let price_currency: String = connection
            .query_row(
                "SELECT currency FROM market_prices WHERE instrument_id='US0378331005'",
                [],
                |row| row.get(0),
            )
            .expect("price currency");
        let basis_currency: String = connection.query_row(
            "SELECT cost_basis_currency FROM broker_position_snapshots WHERE instrument_id='US0378331005'",
            [], |row| row.get(0),
        ).expect("basis currency");
        assert_eq!(price_currency, "USD");
        assert_eq!(basis_currency, "GBP");
    }
}
