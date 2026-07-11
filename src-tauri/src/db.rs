use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::error::Result;
use crate::models::{
    Account, AppSettings, CreateAccountInput, CurrencyOption, PortfolioSummary, UpdateSettingsInput,
};

pub const CURRENCIES: &[CurrencyOption] = &[
    CurrencyOption {
        code: "GBP",
        name: "British pound",
        symbol: "£",
    },
    CurrencyOption {
        code: "USD",
        name: "US dollar",
        symbol: "$",
    },
    CurrencyOption {
        code: "EUR",
        name: "Euro",
        symbol: "€",
    },
    CurrencyOption {
        code: "CHF",
        name: "Swiss franc",
        symbol: "CHF",
    },
    CurrencyOption {
        code: "JPY",
        name: "Japanese yen",
        symbol: "¥",
    },
    CurrencyOption {
        code: "CAD",
        name: "Canadian dollar",
        symbol: "C$",
    },
    CurrencyOption {
        code: "AUD",
        name: "Australian dollar",
        symbol: "A$",
    },
    CurrencyOption {
        code: "NZD",
        name: "New Zealand dollar",
        symbol: "NZ$",
    },
    CurrencyOption {
        code: "HKD",
        name: "Hong Kong dollar",
        symbol: "HK$",
    },
    CurrencyOption {
        code: "SGD",
        name: "Singapore dollar",
        symbol: "S$",
    },
    CurrencyOption {
        code: "SEK",
        name: "Swedish krona",
        symbol: "kr",
    },
    CurrencyOption {
        code: "NOK",
        name: "Norwegian krone",
        symbol: "kr",
    },
    CurrencyOption {
        code: "DKK",
        name: "Danish krone",
        symbol: "kr",
    },
    CurrencyOption {
        code: "PLN",
        name: "Polish złoty",
        symbol: "zł",
    },
    CurrencyOption {
        code: "CZK",
        name: "Czech koruna",
        symbol: "Kč",
    },
    CurrencyOption {
        code: "INR",
        name: "Indian rupee",
        symbol: "₹",
    },
    CurrencyOption {
        code: "ZAR",
        name: "South African rand",
        symbol: "R",
    },
];

pub struct AppState {
    pub connection: Mutex<Connection>,
}

pub fn open(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS app_settings (
           id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
           reporting_currency TEXT,
           onboarding_complete INTEGER NOT NULL DEFAULT 0 CHECK (onboarding_complete IN (0, 1)),
           ai_onboarding_complete INTEGER NOT NULL DEFAULT 0 CHECK (ai_onboarding_complete IN (0, 1)),
           ai_runtime TEXT,
           ai_model TEXT,
           ai_endpoint TEXT,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         INSERT OR IGNORE INTO app_settings (id) VALUES (1);
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
         CREATE TABLE IF NOT EXISTS instruments (
           id TEXT PRIMARY KEY NOT NULL,
           symbol TEXT,
           name TEXT,
           isin TEXT,
           asset_class TEXT,
           sector TEXT,
           geography TEXT,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
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
         );
         CREATE TABLE IF NOT EXISTS market_prices (
           instrument_id TEXT PRIMARY KEY NOT NULL,
           price_coefficient TEXT NOT NULL,
           price_scale INTEGER NOT NULL,
           currency TEXT NOT NULL,
           as_of TEXT NOT NULL,
           source TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS fx_rates (
           base_currency TEXT NOT NULL,
           quote_currency TEXT NOT NULL,
           rate_coefficient TEXT NOT NULL,
           rate_scale INTEGER NOT NULL,
           as_of TEXT NOT NULL,
           source TEXT NOT NULL,
           PRIMARY KEY (base_currency, quote_currency)
         );
         CREATE TABLE IF NOT EXISTS portfolio_snapshots (
           id TEXT PRIMARY KEY NOT NULL,
           captured_at TEXT NOT NULL,
           reporting_currency TEXT NOT NULL,
           total_coefficient TEXT NOT NULL,
           total_scale INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS broker_position_snapshots (
           id TEXT PRIMARY KEY NOT NULL,
           account_id TEXT NOT NULL REFERENCES accounts(id),
           import_batch_id TEXT NOT NULL REFERENCES import_batches(id),
           report_date TEXT NOT NULL,
           instrument_id TEXT NOT NULL,
           quantity_coefficient TEXT NOT NULL,
           quantity_scale INTEGER NOT NULL,
           UNIQUE (account_id, report_date, instrument_id)
         );
         CREATE INDEX IF NOT EXISTS idx_events_projection
           ON events (account_id, instrument_id, event_type, occurred_at, id);
         CREATE INDEX IF NOT EXISTS idx_events_activity ON events (occurred_at DESC, id DESC);
         CREATE INDEX IF NOT EXISTS idx_import_batches_account ON import_batches (account_id, imported_at);
         CREATE INDEX IF NOT EXISTS idx_broker_positions_latest ON broker_position_snapshots (account_id, report_date DESC);",
    )?;
    for (column, definition) in [
        (
            "ai_onboarding_complete",
            "INTEGER NOT NULL DEFAULT 0 CHECK (ai_onboarding_complete IN (0, 1))",
        ),
        ("ai_runtime", "TEXT"),
        ("ai_model", "TEXT"),
        ("ai_endpoint", "TEXT"),
    ] {
        let exists = {
            let mut statement = connection.prepare("PRAGMA table_info(app_settings)")?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .iter()
                .any(|name| name == column)
        };
        if !exists {
            connection.execute_batch(&format!(
                "ALTER TABLE app_settings ADD COLUMN {column} {definition}"
            ))?;
        }
    }
    for (column, definition) in [
        ("asset_class", "TEXT"),
        ("sector", "TEXT"),
        ("geography", "TEXT"),
    ] {
        let exists = {
            let mut statement = connection.prepare("PRAGMA table_info(instruments)")?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .iter()
                .any(|name| name == column)
        };
        if !exists {
            connection.execute_batch(&format!(
                "ALTER TABLE instruments ADD COLUMN {column} {definition}"
            ))?;
        }
    }
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           version INTEGER PRIMARY KEY NOT NULL,
           name TEXT NOT NULL,
           applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         INSERT OR IGNORE INTO schema_migrations (version, name) VALUES
           (1, 'initial_local_ledger'),
           (2, 'adaptive_ai_settings'),
           (3, 'broker_reconciliation_and_instruments'),
           (4, 'instrument_classification'),
           (5, 'reporting_indexes');
         PRAGMA user_version = 5;",
    )?;
    Ok(connection)
}

pub const SCHEMA_VERSION: i64 = 5;

#[cfg(test)]
pub fn schema_version(connection: &Connection) -> Result<i64> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(Into::into)
}

pub fn update_instrument_metadata(
    connection: &Connection,
    input: &crate::models::UpdateInstrumentMetadataInput,
) -> Result<()> {
    let clean = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let asset_class = clean(&input.asset_class);
    let sector = clean(&input.sector);
    let geography = clean(&input.geography);
    if [asset_class.as_ref(), sector.as_ref(), geography.as_ref()]
        .into_iter()
        .flatten()
        .any(|value| value.chars().count() > 80)
    {
        return Err(crate::error::LedgerlyError::InvalidSettings(
            "instrument classification must be 80 characters or fewer".into(),
        ));
    }
    let changed = connection.execute(
        "UPDATE instruments SET asset_class=?2, sector=?3, geography=?4, updated_at=CURRENT_TIMESTAMP WHERE id=?1",
        params![input.instrument_id, asset_class, sector, geography],
    )?;
    if changed == 0 {
        return Err(crate::error::LedgerlyError::InvalidSettings(
            "instrument does not exist".into(),
        ));
    }
    Ok(())
}

pub fn summary(connection: &Connection) -> Result<PortfolioSummary> {
    let account_count =
        connection.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
    let import_count =
        connection.query_row("SELECT COUNT(*) FROM import_batches", [], |row| row.get(0))?;
    Ok(PortfolioSummary {
        reporting_currency: settings(connection)?
            .reporting_currency
            .unwrap_or_else(|| "GBP".into()),
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
        "SELECT id, broker, account_type, display_name, base_currency FROM accounts ORDER BY created_at, id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Account {
            id: row.get(0)?,
            broker: row.get(1)?,
            account_type: row.get(2)?,
            display_name: row.get(3)?,
            base_currency: row.get(4)?,
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
            "SELECT id, broker, account_type, display_name, base_currency FROM accounts WHERE id = ?1",
            [&id],
            |row| {
                Ok(Account {
                    id: row.get(0)?,
                    broker: row.get(1)?,
                    account_type: row.get(2)?,
                    display_name: row.get(3)?,
                    base_currency: row.get(4)?,
                })
            },
        )
        .map_err(Into::into)
}

pub fn currencies() -> &'static [CurrencyOption] {
    CURRENCIES
}

pub fn settings(connection: &Connection) -> Result<AppSettings> {
    connection
        .query_row(
            "SELECT reporting_currency, onboarding_complete, ai_onboarding_complete, ai_runtime, ai_model, ai_endpoint FROM app_settings WHERE id = 1",
            [],
            |row| {
                Ok(AppSettings {
                    reporting_currency: row.get(0)?,
                    onboarding_complete: row.get::<_, i64>(1)? == 1,
                    ai_onboarding_complete: row.get::<_, i64>(2)? == 1,
                    ai_runtime: row.get(3)?,
                    ai_model: row.get(4)?,
                    ai_endpoint: row.get(5)?,
                })
            },
        )
        .map_err(Into::into)
}

pub fn save_ai_settings(
    connection: &Connection,
    input: &crate::models::SaveAiSettingsInput,
) -> Result<AppSettings> {
    connection.execute(
        "UPDATE app_settings SET ai_onboarding_complete=1, ai_runtime=?1, ai_model=?2, ai_endpoint=?3, updated_at=CURRENT_TIMESTAMP WHERE id=1",
        params![input.runtime, input.model, input.endpoint],
    )?;
    settings(connection)
}

pub fn update_settings(
    connection: &Connection,
    input: &UpdateSettingsInput,
) -> Result<AppSettings> {
    let currency = input.reporting_currency.trim().to_uppercase();
    if !CURRENCIES
        .iter()
        .any(|candidate| candidate.code == currency)
    {
        return Err(crate::error::LedgerlyError::InvalidSettings(
            "unsupported reporting currency".into(),
        ));
    }
    connection.execute(
        "UPDATE app_settings SET reporting_currency = ?1, onboarding_complete = 1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
        [&currency],
    )?;
    settings(connection)
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
