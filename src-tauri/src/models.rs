use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct PortfolioSummary {
    pub reporting_currency: String,
    pub account_count: i64,
    pub import_count: i64,
    pub data_status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub id: String,
    pub broker: String,
    pub account_type: String,
    pub display_name: String,
    pub base_currency: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSettings {
    pub reporting_currency: Option<String>,
    pub onboarding_complete: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSettingsInput {
    pub reporting_currency: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrencyOption {
    pub code: &'static str,
    pub name: &'static str,
    pub symbol: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAccountInput {
    pub broker: String,
    pub account_type: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub batch_id: String,
    pub coverage_start: String,
    pub coverage_end: String,
    pub events_added: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityEvent {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    pub broker: String,
    pub event_type: String,
    pub occurred_at: String,
    pub description: String,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub quantity: Option<String>,
    pub instrument_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Holding {
    pub account_id: String,
    pub account_name: String,
    pub broker: String,
    pub instrument_id: String,
    pub quantity: String,
    pub cost_basis: Option<String>,
    pub average_cost: Option<String>,
    pub currency: Option<String>,
    pub cost_basis_complete: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomeSummary {
    pub currency: String,
    pub dividends: String,
    pub interest: String,
    pub total: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetPriceInput {
    pub instrument_id: String,
    pub price: String,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetFxRateInput {
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceQuote {
    pub instrument_id: String,
    pub price: String,
    pub currency: String,
    pub as_of: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FxRate {
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: String,
    pub as_of: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValuedHolding {
    pub holding: Holding,
    pub price: Option<PriceQuote>,
    pub market_value: Option<String>,
    pub reporting_value: Option<String>,
    pub reporting_currency: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValuationSummary {
    pub reporting_currency: String,
    pub total_value: Option<String>,
    pub missing_price_count: usize,
    pub missing_fx_count: usize,
    pub holdings: Vec<ValuedHolding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortfolioSnapshot {
    pub id: String,
    pub captured_at: String,
    pub reporting_currency: String,
    pub total_value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AllocationSlice {
    pub label: String,
    pub value: String,
    pub percentage: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AllocationReport {
    pub reporting_currency: String,
    pub by_account: Vec<AllocationSlice>,
    pub by_currency: Vec<AllocationSlice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackupInput {
    pub path: String,
    pub password: String,
}
