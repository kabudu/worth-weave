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
