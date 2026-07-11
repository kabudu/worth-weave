use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct PortfolioSummary {
    pub base_currency: &'static str,
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
    pub base_currency: &'static str,
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
