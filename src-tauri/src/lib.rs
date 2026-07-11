mod db;
mod error;
mod imports;
mod models;
mod projections;

use db::AppState;
use error::{LedgerlyError, Result};
use models::{
    Account, ActivityEvent, AppSettings, CreateAccountInput, CurrencyOption, Holding, ImportResult,
    IncomeSummary, PortfolioSummary, UpdateSettingsInput,
};
use tauri::{Manager, State};

fn with_connection<T>(
    state: &State<'_, AppState>,
    operation: impl FnOnce(&mut rusqlite::Connection) -> Result<T>,
) -> Result<T> {
    let mut connection = state
        .connection
        .lock()
        .map_err(|_| LedgerlyError::StateUnavailable)?;
    operation(&mut connection)
}

#[tauri::command]
fn portfolio_summary(state: State<'_, AppState>) -> Result<PortfolioSummary> {
    with_connection(&state, |connection| db::summary(connection))
}

#[tauri::command]
fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>> {
    with_connection(&state, |connection| db::accounts(connection))
}

#[tauri::command]
fn create_account(input: CreateAccountInput, state: State<'_, AppState>) -> Result<Account> {
    with_connection(&state, |connection| db::create_account(connection, &input))
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<AppSettings> {
    with_connection(&state, |connection| db::settings(connection))
}

#[tauri::command]
fn list_currencies() -> &'static [CurrencyOption] {
    db::currencies()
}

#[tauri::command]
fn update_settings(input: UpdateSettingsInput, state: State<'_, AppState>) -> Result<AppSettings> {
    with_connection(&state, |connection| db::update_settings(connection, &input))
}

#[tauri::command]
fn list_activity(limit: Option<u32>, state: State<'_, AppState>) -> Result<Vec<ActivityEvent>> {
    with_connection(&state, |connection| {
        projections::activity(connection, limit.unwrap_or(100))
    })
}

#[tauri::command]
fn list_holdings(state: State<'_, AppState>) -> Result<Vec<Holding>> {
    with_connection(&state, |connection| projections::holdings(connection))
}

#[tauri::command]
fn income_summary(state: State<'_, AppState>) -> Result<Vec<IncomeSummary>> {
    with_connection(&state, |connection| projections::income(connection))
}

#[tauri::command]
fn import_broker_file(
    account_id: String,
    file_path: String,
    confirmed_account_type: String,
    state: State<'_, AppState>,
) -> Result<ImportResult> {
    with_connection(&state, |connection| {
        imports::import_csv(
            connection,
            &account_id,
            std::path::Path::new(&file_path),
            &confirmed_account_type,
        )
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|_| LedgerlyError::DataDirectoryUnavailable)?;
            std::fs::create_dir_all(&data_dir)?;
            let connection = db::open(&data_dir.join("worthweave.db"))?;
            app.manage(AppState {
                connection: std::sync::Mutex::new(connection),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            portfolio_summary,
            list_accounts,
            create_account,
            get_settings,
            list_currencies,
            update_settings,
            list_activity,
            list_holdings,
            income_summary,
            import_broker_file
        ])
        .run(tauri::generate_context!())
        .expect("Worthweave failed to start");
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn database_starts_empty_and_persists_accounts() {
        let directory = tempdir().expect("temp directory");
        let connection = db::open(&directory.path().join("worthweave.db")).expect("database");

        let initial = db::summary(&connection).expect("summary");
        assert_eq!(initial.account_count, 0);
        assert_eq!(initial.reporting_currency, "GBP");
        assert_eq!(initial.data_status, "awaiting_imports");
        assert!(
            !db::settings(&connection)
                .expect("settings")
                .onboarding_complete
        );

        let updated = db::update_settings(
            &connection,
            &UpdateSettingsInput {
                reporting_currency: "eur".into(),
            },
        )
        .expect("update settings");
        assert_eq!(updated.reporting_currency.as_deref(), Some("EUR"));
        assert!(updated.onboarding_complete);
        assert!(
            db::update_settings(
                &connection,
                &UpdateSettingsInput {
                    reporting_currency: "BTC".into(),
                },
            )
            .is_err()
        );

        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "trading_212".into(),
                account_type: "stocks_and_shares_isa".into(),
                display_name: "Trading 212 ISA".into(),
            },
        )
        .expect("create account");

        assert_eq!(account.base_currency, "GBP");
        assert_eq!(db::accounts(&connection).expect("accounts").len(), 1);
    }

    #[test]
    fn imported_events_drive_exact_holdings_activity_and_income() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "trading_212".into(),
                account_type: "stocks_and_shares_isa".into(),
                display_name: "ISA".into(),
            },
        )
        .expect("create account");
        let export = directory.path().join("history.csv");
        std::fs::write(
            &export,
            "Action,Time,ISIN,Ticker,ID,No. of shares,Total,Currency (Total)\n\
             Market buy,2026-01-01 10:00:00,GB00TEST0001,TEST,B1,10,100.00,GBP\n\
             Market sell,2026-02-01 10:00:00,GB00TEST0001,TEST,S1,4,60.00,GBP\n\
             Dividend,2026-03-01 10:00:00,GB00TEST0001,TEST,D1,,5.00,GBP\n",
        )
        .expect("write export");
        imports::import_csv(
            &mut connection,
            &account.id,
            &export,
            "stocks_and_shares_isa",
        )
        .expect("import");

        let holdings = projections::holdings(&connection).expect("holdings");
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].quantity, "6");
        assert_eq!(holdings[0].cost_basis.as_deref(), Some("60"));
        assert_eq!(holdings[0].average_cost.as_deref(), Some("10"));
        assert_eq!(
            projections::activity(&connection, 100)
                .expect("activity")
                .len(),
            3
        );
        let income = projections::income(&connection).expect("income");
        assert_eq!(income[0].dividends, "5");
        assert_eq!(income[0].total, "5");
    }

    #[test]
    fn partial_history_never_invents_cost_basis() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "trading_212".into(),
                account_type: "invest".into(),
                display_name: "Partial".into(),
            },
        )
        .expect("create account");
        let export = directory.path().join("partial.csv");
        std::fs::write(
            &export,
            "Action,Time,ISIN,Ticker,ID,No. of shares,Total,Currency (Total)\n\
             Market sell,2026-02-01 10:00:00,GB00TEST0001,TEST,S1,4,60.00,GBP\n",
        )
        .expect("write export");
        imports::import_csv(&mut connection, &account.id, &export, "invest").expect("import");

        let holdings = projections::holdings(&connection).expect("holdings");
        assert_eq!(holdings[0].quantity, "-4");
        assert!(!holdings[0].cost_basis_complete);
        assert!(holdings[0].cost_basis.is_none());
    }
}
