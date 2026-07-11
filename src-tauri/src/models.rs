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
