mod ai;
mod backup;
mod db;
mod error;
mod imports;
mod market;
mod models;
mod projections;

use db::AppState;
use error::{Result, WorthweaveError};
use models::{
    Account, ActivityEvent, AiRecommendation, AllocationReport, AppSettings, BackupInput,
    CreateAccountInput, CurrencyOption, ExplainPortfolioInput, FxRate, FxRefreshResult, Holding,
    ImportResult, IncomeSummary, MassiveProviderStatus, MassiveRefreshResult, PortfolioExplanation,
    PortfolioSnapshot, PortfolioSummary, PriceQuote, ReconciliationItem, SaveAiSettingsInput,
    SaveMassiveApiKeyInput, SetFxRateInput, SetPriceInput, TotalReturnAttribution,
    UpdateInstrumentMetadataInput, UpdateSettingsInput, ValuationSummary,
};
use tauri::{AppHandle, Emitter, Manager, State};

fn with_connection<T>(
    state: &State<'_, AppState>,
    operation: impl FnOnce(&mut rusqlite::Connection) -> Result<T>,
) -> Result<T> {
    let mut connection = state
        .connection
        .lock()
        .map_err(|_| WorthweaveError::StateUnavailable)?;
    operation(&mut connection)
}

async fn with_connection_blocking<T, F>(app: AppHandle, operation: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut rusqlite::Connection) -> Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        with_connection(&state, operation)
    })
    .await
    .map_err(|_| WorthweaveError::StateUnavailable)?
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
fn ai_recommendation() -> AiRecommendation {
    ai::recommendation()
}

#[tauri::command]
async fn setup_recommended_ai(state: State<'_, AppState>) -> Result<AppSettings> {
    let recommendation = ai::recommendation();
    let install_target = recommendation.clone();
    tauri::async_runtime::spawn_blocking(move || ai::install(&install_target))
        .await
        .map_err(|error| {
            WorthweaveError::InvalidSettings(format!("AI setup task failed: {error}"))
        })??;
    let input = SaveAiSettingsInput {
        runtime: Some(recommendation.runtime.into()),
        model: Some(recommendation.model),
        endpoint: Some(recommendation.endpoint.into()),
    };
    with_connection(&state, |connection| {
        db::save_ai_settings(connection, &input)
    })
}

#[tauri::command]
fn skip_ai_setup(state: State<'_, AppState>) -> Result<AppSettings> {
    with_connection(&state, |connection| {
        db::save_ai_settings(
            connection,
            &SaveAiSettingsInput {
                runtime: None,
                model: None,
                endpoint: None,
            },
        )
    })
}

#[tauri::command]
async fn explain_portfolio(
    input: ExplainPortfolioInput,
    state: State<'_, AppState>,
) -> Result<PortfolioExplanation> {
    let (runtime, endpoint, model, analytics) = with_connection(&state, |connection| {
        let settings = db::settings(connection)?;
        let endpoint = settings.ai_endpoint.ok_or_else(|| {
            WorthweaveError::LocalAi("local AI is not configured in Settings".into())
        })?;
        let model = settings.ai_model.ok_or_else(|| {
            WorthweaveError::LocalAi("local AI is not configured in Settings".into())
        })?;
        let runtime = settings.ai_runtime.ok_or_else(|| {
            WorthweaveError::LocalAi("local AI is not configured in Settings".into())
        })?;
        let analytics = serde_json::json!({
            "valuation": market::valuation(connection)?,
            "total_return_attribution": market::total_return_attribution(connection)?,
            "allocation": market::allocation(connection).ok(),
            "reconciliation": projections::reconciliation(connection)?,
            "income": projections::income(connection)?,
            "snapshots": market::snapshots(connection)?,
        });
        Ok((runtime, endpoint, model, analytics.to_string()))
    })?;
    ai::explain(&runtime, &endpoint, &model, &input.question, &analytics).await
}

#[tauri::command]
fn list_activity(limit: Option<u32>, state: State<'_, AppState>) -> Result<Vec<ActivityEvent>> {
    with_connection(&state, |connection| {
        projections::activity(connection, limit.unwrap_or(100))
    })
}

#[tauri::command]
async fn list_holdings(app: AppHandle) -> Result<Vec<Holding>> {
    with_connection_blocking(app, |connection| projections::holdings(connection)).await
}

#[tauri::command]
fn income_summary(state: State<'_, AppState>) -> Result<Vec<IncomeSummary>> {
    with_connection(&state, |connection| projections::income(connection))
}

#[tauri::command]
async fn portfolio_reconciliation(app: AppHandle) -> Result<Vec<ReconciliationItem>> {
    with_connection_blocking(app, |connection| projections::reconciliation(connection)).await
}

#[tauri::command]
async fn portfolio_performance_history(
    scope: Option<String>,
    app: AppHandle,
) -> Result<crate::models::PerformanceHistory> {
    let scope = scope.unwrap_or_else(|| "all".into());
    with_connection_blocking(app, move |connection| {
        projections::performance_history(connection, &scope)
    })
    .await
}

#[tauri::command]
fn set_market_price(input: SetPriceInput, state: State<'_, AppState>) -> Result<PriceQuote> {
    with_connection(&state, |connection| market::set_price(connection, &input))
}

#[tauri::command]
fn set_fx_rate(input: SetFxRateInput, state: State<'_, AppState>) -> Result<FxRate> {
    with_connection(&state, |connection| market::set_fx_rate(connection, &input))
}

#[tauri::command]
async fn refresh_fx_rates(state: State<'_, AppState>) -> Result<FxRefreshResult> {
    let historical_plan =
        with_connection(&state, |connection| market::historical_fx_plan(connection))?;
    let historical = if let Some(plan) = historical_plan.as_ref() {
        market::fetch_ecb_historical_rates(plan).await?
    } else {
        Vec::new()
    };
    let reference = market::fetch_ecb_reference_rates().await?;
    with_connection(&state, |connection| {
        let mut result = market::save_ecb_reference_rates(connection, &reference)?;
        result.rates_saved += market::save_ecb_historical_rates(connection, &historical)?;
        Ok(result)
    })
}

#[tauri::command]
fn massive_provider_status() -> Result<MassiveProviderStatus> {
    market::massive_provider_status()
}

#[tauri::command]
fn save_massive_api_key(input: SaveMassiveApiKeyInput) -> Result<MassiveProviderStatus> {
    market::save_massive_api_key(&input.api_key)
}

#[tauri::command]
fn remove_massive_api_key() -> Result<MassiveProviderStatus> {
    market::remove_massive_api_key()
}

#[tauri::command]
async fn refresh_massive_prices(state: State<'_, AppState>) -> Result<MassiveRefreshResult> {
    let candidates = with_connection(&state, |connection| market::massive_candidates(connection))?;
    let observations = market::fetch_massive_prices(candidates).await?;
    with_connection(&state, |connection| {
        market::save_massive_prices(connection, observations)
    })
}

#[tauri::command]
async fn refresh_portfolio_history(
    state: State<'_, AppState>,
) -> Result<crate::models::HistoryRefreshResult> {
    let candidates = with_connection(&state, |connection| market::history_candidates(connection))?;
    let requested = candidates.len();
    let observations = market::fetch_historical_prices(candidates).await?;
    with_connection(&state, |connection| {
        market::save_historical_prices(connection, observations, requested)
    })
}

#[tauri::command]
fn update_instrument_metadata(
    input: UpdateInstrumentMetadataInput,
    state: State<'_, AppState>,
) -> Result<()> {
    with_connection(&state, |connection| {
        db::update_instrument_metadata(connection, &input)
    })
}

#[tauri::command]
async fn portfolio_valuation(app: AppHandle) -> Result<ValuationSummary> {
    with_connection_blocking(app, |connection| market::valuation(connection)).await
}

#[tauri::command]
async fn portfolio_total_return(app: AppHandle) -> Result<TotalReturnAttribution> {
    with_connection_blocking(app, |connection| {
        market::total_return_attribution(connection)
    })
    .await
}

#[tauri::command]
fn capture_portfolio_snapshot(state: State<'_, AppState>) -> Result<PortfolioSnapshot> {
    with_connection(&state, |connection| market::capture_snapshot(connection))
}

#[tauri::command]
fn list_portfolio_snapshots(state: State<'_, AppState>) -> Result<Vec<PortfolioSnapshot>> {
    with_connection(&state, |connection| market::snapshots(connection))
}

#[tauri::command]
async fn portfolio_allocation(app: AppHandle) -> Result<AllocationReport> {
    with_connection_blocking(app, |connection| market::allocation(connection)).await
}

#[tauri::command]
fn create_encrypted_backup(input: BackupInput, state: State<'_, AppState>) -> Result<()> {
    with_connection(&state, |connection| {
        backup::create(
            connection,
            std::path::Path::new(&input.path),
            input.password,
        )
    })
}

#[tauri::command]
fn restore_encrypted_backup(input: BackupInput, state: State<'_, AppState>) -> Result<()> {
    with_connection(&state, |connection| {
        backup::restore(
            connection,
            std::path::Path::new(&input.path),
            input.password,
        )
    })
}

#[tauri::command]
fn export_portfolio_json(path: String, state: State<'_, AppState>) -> Result<()> {
    with_connection(&state, |connection| {
        backup::export_json(connection, std::path::Path::new(&path))
    })
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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{MenuBuilder, SubmenuBuilder};
                let application = SubmenuBuilder::new(app, "Worthweave")
                    .text("about-worthweave", "About Worthweave")
                    .separator()
                    .services()
                    .separator()
                    .hide()
                    .hide_others()
                    .separator()
                    .quit()
                    .build()?;
                let edit = SubmenuBuilder::new(app, "Edit")
                    .undo()
                    .redo()
                    .separator()
                    .cut()
                    .copy()
                    .paste()
                    .select_all()
                    .build()?;
                let window = SubmenuBuilder::new(app, "Window")
                    .minimize()
                    .maximize()
                    .fullscreen()
                    .separator()
                    .close_window()
                    .build()?;
                let menu = MenuBuilder::new(app)
                    .items(&[&application, &edit, &window])
                    .build()?;
                app.set_menu(menu)?;
            }
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|_| WorthweaveError::DataDirectoryUnavailable)?;
            std::fs::create_dir_all(&data_dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700))?;
            }
            let connection = db::open(&data_dir.join("worthweave.db"))?;
            app.manage(AppState {
                connection: std::sync::Mutex::new(connection),
            });
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "about-worthweave" {
                let _ = app.emit("open-about-worthweave", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            portfolio_summary,
            list_accounts,
            create_account,
            get_settings,
            list_currencies,
            update_settings,
            ai_recommendation,
            setup_recommended_ai,
            skip_ai_setup,
            explain_portfolio,
            list_activity,
            list_holdings,
            income_summary,
            portfolio_reconciliation,
            portfolio_performance_history,
            set_market_price,
            set_fx_rate,
            refresh_fx_rates,
            massive_provider_status,
            save_massive_api_key,
            remove_massive_api_key,
            refresh_massive_prices,
            refresh_portfolio_history,
            update_instrument_metadata,
            portfolio_valuation,
            portfolio_total_return,
            capture_portfolio_snapshot,
            list_portfolio_snapshots,
            portfolio_allocation,
            create_encrypted_backup,
            restore_encrypted_backup,
            export_portfolio_json,
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(directory.path().join("worthweave.db"))
                .expect("database metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0);
        }

        let initial = db::summary(&connection).expect("summary");
        assert_eq!(
            db::schema_version(&connection).expect("schema version"),
            db::SCHEMA_VERSION
        );
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration history");
        assert_eq!(migration_count, db::SCHEMA_VERSION);
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
                jurisdiction: "GB".into(),
                account_type: "stocks_and_shares_isa".into(),
                display_name: "Trading 212 ISA".into(),
            },
        )
        .expect("create account");

        assert_eq!(account.base_currency, "GBP");
        assert_eq!(account.jurisdiction, "GB");
        assert_eq!(db::accounts(&connection).expect("accounts").len(), 1);
    }

    #[test]
    fn robinhood_accounts_are_region_aware() {
        let directory = tempdir().expect("temp directory");
        let connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let roth = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "robinhood".into(),
                jurisdiction: "US".into(),
                account_type: "roth_ira".into(),
                display_name: "Robinhood Roth IRA".into(),
            },
        )
        .expect("US Roth IRA");
        assert_eq!(roth.base_currency, "USD");
        assert_eq!(roth.jurisdiction, "US");
        assert!(
            db::create_account(
                &connection,
                &CreateAccountInput {
                    broker: "robinhood".into(),
                    jurisdiction: "GB".into(),
                    account_type: "roth_ira".into(),
                    display_name: "Invalid UK Roth".into(),
                },
            )
            .is_err()
        );
    }

    #[test]
    fn legacy_accounts_migrate_to_gb_jurisdiction() {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("worthweave.db");
        {
            let connection = rusqlite::Connection::open(&path).expect("legacy database");
            connection
                .execute_batch(
                    "CREATE TABLE accounts (
                    id TEXT PRIMARY KEY NOT NULL,
                    broker TEXT NOT NULL,
                    account_type TEXT NOT NULL,
                    external_id TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    base_currency TEXT NOT NULL DEFAULT 'GBP',
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE (broker, external_id)
                 );
                 INSERT INTO accounts (id, broker, account_type, external_id, display_name)
                 VALUES ('legacy', 'ibkr', 'invest', 'ibkr:invest:legacy', 'Legacy IBKR');",
                )
                .expect("legacy schema");
        }
        let connection = db::open(&path).expect("migrated database");
        let accounts = db::accounts(&connection).expect("accounts");
        assert_eq!(accounts[0].jurisdiction, "GB");
        assert_eq!(
            db::schema_version(&connection).expect("schema version"),
            db::SCHEMA_VERSION
        );
    }

    #[test]
    fn imported_events_drive_exact_holdings_activity_and_income() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "trading_212".into(),
                jurisdiction: "GB".into(),
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
        assert_eq!(holdings[0].symbol.as_deref(), Some("TEST"));
        db::update_instrument_metadata(
            &connection,
            &UpdateInstrumentMetadataInput {
                instrument_id: "GB00TEST0001".into(),
                asset_class: Some("Equity".into()),
                sector: Some("Technology".into()),
                geography: Some("United Kingdom".into()),
            },
        )
        .expect("instrument metadata");
        assert!(
            projections::reconciliation(&connection)
                .expect("reconciliation")
                .is_empty()
        );
        let batch_id: String = connection
            .query_row("SELECT id FROM import_batches LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("batch");
        connection.execute(
            "INSERT INTO broker_position_snapshots (id, account_id, import_batch_id, report_date, instrument_id, quantity_coefficient, quantity_scale) VALUES (?1, ?2, ?3, '2026-03-31', 'GB00TEST0001', '6', 0)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), account.id, batch_id],
        ).expect("broker snapshot");
        assert_eq!(
            projections::reconciliation(&connection).expect("reconciliation")[0].status,
            "matched"
        );
        assert_eq!(
            projections::activity(&connection, 100)
                .expect("activity")
                .len(),
            3
        );
        let income = projections::income(&connection).expect("income");
        assert_eq!(income[0].dividends, "5");
        assert_eq!(income[0].total, "5");

        let unavailable = market::valuation(&connection).expect("unavailable valuation");
        assert_eq!(unavailable.missing_price_count, 1);
        assert!(unavailable.total_value.is_none());
        market::set_price(
            &connection,
            &SetPriceInput {
                instrument_id: "GB00TEST0001".into(),
                price: "20".into(),
                currency: "USD".into(),
            },
        )
        .expect("price");
        market::set_fx_rate(
            &connection,
            &SetFxRateInput {
                base_currency: "USD".into(),
                quote_currency: "GBP".into(),
                rate: "0.8".into(),
            },
        )
        .expect("FX rate");
        let valuation = market::valuation(&connection).expect("valuation");
        assert_eq!(valuation.total_value.as_deref(), Some("96"));
        assert_eq!(valuation.missing_price_count, 0);
        assert_eq!(valuation.missing_fx_count, 0);
        assert_eq!(valuation.stale_price_count, 0);
        assert_eq!(valuation.stale_fx_count, 0);
        assert_eq!(valuation.total_gain_loss.as_deref(), Some("36"));
        assert_eq!(valuation.holdings[0].gain_loss.as_deref(), Some("36"));
        let fx_partial = market::total_return_attribution(&connection).expect("FX attribution");
        assert_eq!(fx_partial.status, "partial");
        assert_eq!(fx_partial.realized_gain_loss.as_deref(), Some("20"));
        assert_eq!(fx_partial.unrealized_gain_loss.as_deref(), Some("36"));
        assert_eq!(fx_partial.dividends.as_deref(), Some("5"));
        assert_eq!(fx_partial.attributed_subtotal.as_deref(), Some("61"));
        assert!(fx_partial.fx_impact.is_none());
        assert!(fx_partial.total_return.is_none());

        market::set_price(
            &connection,
            &SetPriceInput {
                instrument_id: "GB00TEST0001".into(),
                price: "16".into(),
                currency: "GBP".into(),
            },
        )
        .expect("GBP price");
        let attribution = market::total_return_attribution(&connection).expect("attribution");
        assert_eq!(attribution.status, "complete");
        assert_eq!(attribution.coverage_start.as_deref(), Some("2026-01-01"));
        assert_eq!(attribution.coverage_end.as_deref(), Some("2026-03-01"));
        assert_eq!(attribution.realized_gain_loss.as_deref(), Some("20"));
        assert_eq!(attribution.unrealized_gain_loss.as_deref(), Some("36"));
        assert_eq!(attribution.dividends.as_deref(), Some("5"));
        assert_eq!(attribution.interest.as_deref(), Some("0"));
        assert_eq!(attribution.fees.as_deref(), Some("0"));
        assert_eq!(attribution.taxes.as_deref(), Some("0"));
        assert_eq!(attribution.fx_impact.as_deref(), Some("0"));
        assert_eq!(attribution.total_return.as_deref(), Some("61"));
        let snapshot = market::capture_snapshot(&connection).expect("snapshot");
        assert_eq!(snapshot.total_value, "96");
        assert_eq!(market::snapshots(&connection).expect("snapshots").len(), 1);
        let allocation = market::allocation(&connection).expect("allocation");
        assert_eq!(allocation.by_account[0].value, "96");
        assert_eq!(allocation.by_account[0].percentage, "100");
        assert_eq!(allocation.by_platform[0].label, "trading_212");
        assert_eq!(allocation.by_asset_class[0].label, "Equity");
        assert_eq!(allocation.by_sector[0].label, "Technology");
        assert_eq!(allocation.by_geography[0].label, "United Kingdom");
    }

    #[test]
    fn latest_broker_snapshot_controls_quantity_and_supplies_cost_basis() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "ibkr".into(),
                jurisdiction: "GB".into(),
                account_type: "invest".into(),
                display_name: "IBKR Invest".into(),
            },
        )
        .expect("create account");
        let export = directory.path().join("history.csv");
        std::fs::write(
            &export,
            "ClientAccountID,CurrencyPrimary,TradeID,Buy/Sell,TradeMoney,Date/Time,Quantity,NetCash,Description,Symbol,ISIN,AssetClass\n\
             U1,GBP,T1,BUY,20.00,2026-07-01;10:00:00,2,-20.00,Example,TEST,GB00TEST0001,STK\n\
             ClientAccountID,CurrencyPrimary,ReportDate,Quantity,MarkPrice,PositionValue,CostBasisMoney,LevelOfDetail,Symbol,Description,ISIN,Conid,AssetClass\n\
             U1,GBP,2026-07-10,3,10,30,25,Summary,TEST,Example,GB00TEST0001,123,STK\n",
        )
        .expect("write export");
        imports::import_csv(&mut connection, &account.id, &export, "invest").expect("import");

        let holdings = projections::holdings(&connection).expect("holdings");
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].quantity, "3");
        assert!(holdings[0].cost_basis_complete);
        assert_eq!(holdings[0].cost_basis.as_deref(), Some("25"));
        assert_eq!(
            holdings[0].average_cost.as_deref(),
            Some("8.333333333333333333333333333")
        );
        let reconciliation = projections::reconciliation(&connection).expect("reconciliation");
        assert_eq!(reconciliation[0].status, "broker_basis");
        assert_eq!(reconciliation[0].difference.as_deref(), Some("-1"));
    }

    #[test]
    fn trading212_splits_adjust_quantity_and_partial_values_are_explicit() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "trading_212".into(),
                jurisdiction: "GB".into(),
                account_type: "stocks_and_shares_isa".into(),
                display_name: "Trading 212 ISA".into(),
            },
        )
        .expect("create account");
        let export = directory.path().join("history.csv");
        std::fs::write(
            &export,
            "Action,Time,ISIN,Ticker,ID,No. of shares,Total,Currency (Total)\n\
             Market buy,2025-01-01 10:00:00,US00SPLIT001,SPLT,B1,1128,100.00,GBP\n\
             Stock split close,2025-03-18 07:41:35,US00SPLIT001,SPLT,C1,1128,,GBP\n\
             Stock split open,2025-03-18 07:41:35,US00SPLIT001,SPLT,O1,22.56,,GBP\n\
             Market buy,2025-04-01 10:00:00,US00NOPRICE1,NOPR,B2,2,20.00,GBP\n",
        )
        .expect("write export");
        imports::import_csv(
            &mut connection,
            &account.id,
            &export,
            "stocks_and_shares_isa",
        )
        .expect("import");
        assert_eq!(
            connection
                .query_row(
                    "SELECT numerator || '/' || denominator FROM corporate_action_adjustments WHERE instrument_id='US00SPLIT001' AND source='broker_import'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("normalized imported split"),
            "1/50"
        );
        connection.execute(
            "INSERT INTO historical_prices (instrument_id, price_date, price_coefficient, price_scale, currency, source) VALUES ('US00SPLIT001', '2025-01-02', '2', 0, 'GBP', 'test')",
            [],
        ).expect("historical price");
        let history = projections::performance_history(&connection, "all").expect("history");
        assert_eq!(history.coverage, "market_reconstructed");
        assert_eq!(history.points[0].value, "2256");
        let holdings = projections::holdings(&connection).expect("holdings");
        let split = holdings
            .iter()
            .find(|holding| holding.instrument_id == "US00SPLIT001")
            .expect("split holding");
        assert_eq!(split.quantity, "22.56");
        assert_eq!(split.cost_basis.as_deref(), Some("100"));

        market::set_price(
            &connection,
            &SetPriceInput {
                instrument_id: "US00SPLIT001".into(),
                price: "5".into(),
                currency: "GBP".into(),
            },
        )
        .expect("price");
        let valuation = market::valuation(&connection).expect("partial valuation");
        assert_eq!(valuation.total_value.as_deref(), Some("112.8"));
        assert!(!valuation.valuation_complete);
        assert_eq!(valuation.valued_holding_count, 1);
        assert_eq!(valuation.missing_price_count, 1);
        assert_eq!(valuation.missing_fx_count, 0);
        assert!(market::capture_snapshot(&connection).is_err());
        assert!(market::allocation(&connection).is_err());
    }

    #[test]
    fn trading212_disposals_without_acquisition_history_do_not_create_short_holdings() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "trading_212".into(),
                jurisdiction: "GB".into(),
                account_type: "stocks_and_shares_isa".into(),
                display_name: "Trading 212 ISA".into(),
            },
        )
        .expect("create account");
        let export = directory.path().join("history.csv");
        std::fs::write(
            &export,
            "Action,Time,ISIN,Ticker,ID,No. of shares,Total,Currency (Total)\nMarket sell,2024-02-21 19:36:16,US54948X1090,LUCD,S1,1.56989332,1.54,GBP\nLimit sell,2024-12-16 20:58:38,US54948X1090,LUCD,S2,11,6.97,GBP\n",
        ).expect("write export");
        imports::import_csv(
            &mut connection,
            &account.id,
            &export,
            "stocks_and_shares_isa",
        )
        .expect("import");
        assert!(
            projections::holdings(&connection)
                .expect("holdings")
                .is_empty()
        );
    }

    #[test]
    fn partial_history_never_invents_cost_basis() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        let account = db::create_account(
            &connection,
            &CreateAccountInput {
                broker: "trading_212".into(),
                jurisdiction: "GB".into(),
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

        assert!(
            projections::holdings(&connection)
                .expect("holdings")
                .is_empty()
        );
    }

    #[test]
    fn encrypted_backup_round_trip_restores_validated_database() {
        let directory = tempdir().expect("temp directory");
        let mut connection = db::open(&directory.path().join("worthweave.db")).expect("database");
        db::update_settings(
            &connection,
            &UpdateSettingsInput {
                reporting_currency: "EUR".into(),
            },
        )
        .expect("settings");
        let path = directory.path().join("portfolio.worthweave-age");
        backup::create(&connection, &path, "a strong test password".into()).expect("backup");
        assert!(
            !std::fs::read(&path)
                .expect("encrypted bytes")
                .starts_with(b"SQLite format 3\0")
        );
        db::update_settings(
            &connection,
            &UpdateSettingsInput {
                reporting_currency: "GBP".into(),
            },
        )
        .expect("mutate settings");
        backup::restore(&mut connection, &path, "a strong test password".into()).expect("restore");
        assert_eq!(
            db::settings(&connection)
                .expect("restored settings")
                .reporting_currency
                .as_deref(),
            Some("EUR")
        );
        assert!(backup::restore(&mut connection, &path, "incorrect password".into()).is_err());
        let export_path = directory.path().join("portfolio.json");
        backup::export_json(&connection, &export_path).expect("JSON export");
        let exported: serde_json::Value =
            serde_json::from_slice(&std::fs::read(export_path).expect("exported JSON"))
                .expect("valid JSON");
        assert_eq!(exported["format"], "worthweave-portfolio-export");
        assert_eq!(exported["version"], 1);
    }
}
