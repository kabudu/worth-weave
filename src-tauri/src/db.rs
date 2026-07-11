use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::error::Result;
use crate::models::{Account, CreateAccountInput, PortfolioSummary};

pub struct AppState {
    pub connection: Mutex<Connection>,
}

pub fn open(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS accounts (
           id TEXT PRIMARY KEY NOT NULL,
           broker TEXT NOT NULL,
           account_type TEXT NOT NULL,
           external_id TEXT NOT NULL,
           display_name TEXT NOT NULL,
           base_currency TEXT NOT NULL DEFAULT 'GBP',
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           UNIQUE (broker, external_id)
         );
         CREATE TABLE IF NOT EXISTS import_batches (
           id TEXT PRIMARY KEY NOT NULL,
           account_id TEXT NOT NULL REFERENCES accounts(id),
           original_filename TEXT NOT NULL,
           content_sha256 TEXT NOT NULL,
           coverage_start TEXT,
           coverage_end TEXT,
           imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           UNIQUE (account_id, content_sha256)
         );
         CREATE TABLE IF NOT EXISTS events (
           id TEXT PRIMARY KEY NOT NULL,
           account_id TEXT NOT NULL REFERENCES accounts(id),
           import_batch_id TEXT NOT NULL REFERENCES import_batches(id),
           source_id TEXT NOT NULL,
           event_type TEXT NOT NULL,
           occurred_at TEXT NOT NULL,
           description TEXT NOT NULL,
           amount_coefficient TEXT,
           amount_scale INTEGER,
           currency TEXT,
           quantity_coefficient TEXT,
           quantity_scale INTEGER,
           instrument_id TEXT,
           UNIQUE (account_id, source_id)
         );",
    )?;
    Ok(connection)
}

pub fn summary(connection: &Connection) -> Result<PortfolioSummary> {
    let account_count =
        connection.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
    let import_count =
        connection.query_row("SELECT COUNT(*) FROM import_batches", [], |row| row.get(0))?;
    Ok(PortfolioSummary {
        base_currency: "GBP",
        account_count,
        import_count,
        data_status: if import_count == 0 {
            "awaiting_imports"
        } else {
            "partial"
        },
    })
}

pub fn accounts(connection: &Connection) -> Result<Vec<Account>> {
    let mut statement = connection.prepare(
        "SELECT id, broker, account_type, display_name FROM accounts ORDER BY created_at, id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Account {
            id: row.get(0)?,
            broker: row.get(1)?,
            account_type: row.get(2)?,
            display_name: row.get(3)?,
            base_currency: "GBP",
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn create_account(connection: &Connection, input: &CreateAccountInput) -> Result<Account> {
    if !matches!(input.broker.as_str(), "trading_212" | "ibkr") {
        return Err(crate::error::LedgerlyError::InvalidAccount(
            "unsupported broker".into(),
        ));
    }
    if !matches!(
        input.account_type.as_str(),
        "invest" | "stocks_and_shares_isa"
    ) {
        return Err(crate::error::LedgerlyError::InvalidAccount(
            "unsupported account type".into(),
        ));
    }
    if input.display_name.trim().is_empty() || input.display_name.chars().count() > 160 {
        return Err(crate::error::LedgerlyError::InvalidAccount(
            "account name must contain 1 to 160 characters".into(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    let external_id = format!("{}:{}:{}", input.broker, input.account_type, Uuid::new_v4());
    connection.execute(
        "INSERT INTO accounts (id, broker, account_type, external_id, display_name) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, input.broker, input.account_type, external_id, input.display_name.trim()],
    )?;
    connection
        .query_row(
            "SELECT id, broker, account_type, display_name FROM accounts WHERE id = ?1",
            [&id],
            |row| {
                Ok(Account {
                    id: row.get(0)?,
                    broker: row.get(1)?,
                    account_type: row.get(2)?,
                    display_name: row.get(3)?,
                    base_currency: "GBP",
                })
            },
        )
        .map_err(Into::into)
}

pub fn account_identity(
    connection: &Connection,
    account_id: &str,
) -> Result<Option<(String, String)>> {
    connection
        .query_row(
            "SELECT broker, account_type FROM accounts WHERE id = ?1",
            [account_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(Into::into)
}
