use chrono::{Months, Utc};
use rusqlite::{Connection, params};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::error::Result;

struct DemoHolding {
    account: &'static str,
    instrument: &'static str,
    symbol: &'static str,
    name: &'static str,
    asset_class: &'static str,
    sector: &'static str,
    geography: &'static str,
    quantity: &'static str,
    cost_basis: &'static str,
    price: &'static str,
    currency: &'static str,
}

const ACCOUNTS: [(&str, &str, &str, &str); 3] = [
    (
        "11111111-1111-4111-8111-111111111111",
        "trading_212",
        "stocks_and_shares_isa",
        "Everyday ISA",
    ),
    (
        "22222222-2222-4222-8222-222222222222",
        "ibkr",
        "invest",
        "Global Invest",
    ),
    (
        "33333333-3333-4333-8333-333333333333",
        "ibkr",
        "stocks_and_shares_isa",
        "Long-term ISA",
    ),
];

const HOLDINGS: [DemoHolding; 12] = [
    DemoHolding {
        account: ACCOUNTS[0].0,
        instrument: "IE00BK5BQT80",
        symbol: "VWRP",
        name: "Vanguard FTSE All-World ETF",
        asset_class: "ETF",
        sector: "Global equities",
        geography: "Global",
        quantity: "120",
        cost_basis: "10440",
        price: "110",
        currency: "GBP",
    },
    DemoHolding {
        account: ACCOUNTS[0].0,
        instrument: "GB0009895292",
        symbol: "AZN",
        name: "AstraZeneca",
        asset_class: "Equity",
        sector: "Healthcare",
        geography: "United Kingdom",
        quantity: "80",
        cost_basis: "7600",
        price: "135",
        currency: "GBP",
    },
    DemoHolding {
        account: ACCOUNTS[0].0,
        instrument: "GB00B10RZP78",
        symbol: "ULVR",
        name: "Unilever",
        asset_class: "Equity",
        sector: "Consumer staples",
        geography: "United Kingdom",
        quantity: "120",
        cost_basis: "4920",
        price: "48",
        currency: "GBP",
    },
    DemoHolding {
        account: ACCOUNTS[0].0,
        instrument: "GB0005603997",
        symbol: "LGEN",
        name: "Legal & General",
        asset_class: "Equity",
        sector: "Financial services",
        geography: "United Kingdom",
        quantity: "1000",
        cost_basis: "2250",
        price: "2.70",
        currency: "GBP",
    },
    DemoHolding {
        account: ACCOUNTS[1].0,
        instrument: "US5949181045",
        symbol: "MSFT",
        name: "Microsoft",
        asset_class: "Equity",
        sector: "Technology",
        geography: "United States",
        quantity: "15",
        cost_basis: "5700",
        price: "520",
        currency: "USD",
    },
    DemoHolding {
        account: ACCOUNTS[1].0,
        instrument: "US0378331005",
        symbol: "AAPL",
        name: "Apple",
        asset_class: "Equity",
        sector: "Technology",
        geography: "United States",
        quantity: "20",
        cost_basis: "3500",
        price: "245",
        currency: "USD",
    },
    DemoHolding {
        account: ACCOUNTS[1].0,
        instrument: "US67066G1040",
        symbol: "NVDA",
        name: "NVIDIA",
        asset_class: "Equity",
        sector: "Technology",
        geography: "United States",
        quantity: "12",
        cost_basis: "1320",
        price: "190",
        currency: "USD",
    },
    DemoHolding {
        account: ACCOUNTS[1].0,
        instrument: "US0846707026",
        symbol: "BRK.B",
        name: "Berkshire Hathaway",
        asset_class: "Equity",
        sector: "Financial services",
        geography: "United States",
        quantity: "4",
        cost_basis: "2320",
        price: "760",
        currency: "USD",
    },
    DemoHolding {
        account: ACCOUNTS[2].0,
        instrument: "GB00BDR05C01",
        symbol: "NG.",
        name: "National Grid",
        asset_class: "Equity",
        sector: "Utilities",
        geography: "United Kingdom",
        quantity: "500",
        cost_basis: "4550",
        price: "11",
        currency: "GBP",
    },
    DemoHolding {
        account: ACCOUNTS[2].0,
        instrument: "GB00BP6MXD84",
        symbol: "SHEL",
        name: "Shell",
        asset_class: "Equity",
        sector: "Energy",
        geography: "Global",
        quantity: "90",
        cost_basis: "2160",
        price: "29",
        currency: "GBP",
    },
    DemoHolding {
        account: ACCOUNTS[2].0,
        instrument: "GB00B2B0DG97",
        symbol: "REL",
        name: "RELX",
        asset_class: "Equity",
        sector: "Industrials",
        geography: "Global",
        quantity: "30",
        cost_basis: "930",
        price: "42",
        currency: "GBP",
    },
    DemoHolding {
        account: ACCOUNTS[2].0,
        instrument: "IE00B3WJKG14",
        symbol: "IITU",
        name: "iShares S&P 500 Information Technology ETF",
        asset_class: "ETF",
        sector: "Technology",
        geography: "United States",
        quantity: "50",
        cost_basis: "1200",
        price: "32",
        currency: "GBP",
    },
];

fn parts(value: Decimal) -> (String, u32) {
    let value = value.normalize();
    (value.mantissa().to_string(), value.scale())
}

fn decimal(value: &str) -> Decimal {
    Decimal::from_str(value).expect("static demo decimal")
}

pub fn seed(connection: &mut Connection) -> Result<()> {
    let account_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
    if account_count > 0 {
        return Ok(());
    }

    let today = Utc::now().date_naive();
    let history_start = today.checked_sub_months(Months::new(35)).unwrap_or(today);
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE app_settings SET reporting_currency='GBP', onboarding_complete=1, ai_onboarding_complete=1 WHERE id=1",
        [],
    )?;

    for (index, (id, broker, account_type, name)) in ACCOUNTS.iter().enumerate() {
        transaction.execute(
            "INSERT INTO accounts (id, broker, jurisdiction, account_type, external_id, display_name, base_currency)
             VALUES (?1, ?2, 'GB', ?3, ?4, ?5, 'GBP')",
            params![id, broker, account_type, format!("demo-{index}"), name],
        )?;
        transaction.execute(
            "INSERT INTO import_batches (id, account_id, original_filename, content_sha256, coverage_start, coverage_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                format!("aaaaaaaa-aaaa-4aaa-8aaa-{index:012}"),
                id,
                format!("{}-demo-portfolio.csv", broker.replace('_', "-")),
                format!("demo-content-{index}"),
                history_start.to_string(),
                today.to_string(),
            ],
        )?;
    }

    for holding in &HOLDINGS {
        transaction.execute(
            "INSERT INTO instruments (id, symbol, name, isin, asset_class, sector, geography)
             VALUES (?1, ?2, ?3, ?1, ?4, ?5, ?6)",
            params![
                holding.instrument,
                holding.symbol,
                holding.name,
                holding.asset_class,
                holding.sector,
                holding.geography
            ],
        )?;
        let price = decimal(holding.price);
        let (price_coefficient, price_scale) = parts(price);
        transaction.execute(
            "INSERT INTO market_prices (instrument_id, price_coefficient, price_scale, currency, as_of, source)
             VALUES (?1, ?2, ?3, ?4, ?5, 'demo_market_data')",
            params![holding.instrument, price_coefficient, price_scale, holding.currency, format!("{today}T16:30:00+00:00")],
        )?;
        let quantity = decimal(holding.quantity);
        let cost_basis = decimal(holding.cost_basis);
        let (amount_coefficient, amount_scale) = parts(cost_basis);
        let (quantity_coefficient, quantity_scale) = parts(quantity);
        let account_index = ACCOUNTS
            .iter()
            .position(|account| account.0 == holding.account)
            .unwrap_or_default();
        transaction.execute(
            "INSERT INTO events (id, account_id, import_batch_id, source_id, event_type, occurred_at, description,
              amount_coefficient, amount_scale, currency, quantity_coefficient, quantity_scale, instrument_id)
             VALUES (?1, ?2, ?3, ?4, 'buy', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                format!("event-buy-{}", holding.instrument),
                holding.account,
                format!("aaaaaaaa-aaaa-4aaa-8aaa-{account_index:012}"),
                format!("demo-buy-{}", holding.instrument),
                format!("{history_start}T10:00:00+00:00"),
                format!("Bought {} for the demo portfolio", holding.name),
                amount_coefficient,
                amount_scale,
                holding.currency,
                quantity_coefficient,
                quantity_scale,
                holding.instrument,
            ],
        )?;
    }

    for month in 0..36_u32 {
        let date = history_start
            .checked_add_months(Months::new(month))
            .unwrap_or(today)
            .min(today);
        let progress = Decimal::from(month) / Decimal::from(35);
        let wave = Decimal::from(i64::from((month % 7) as i32 - 3)) / Decimal::from(250);
        let factor = Decimal::new(72, 2) + progress * Decimal::new(28, 2) + wave;
        for holding in &HOLDINGS {
            let quantity = decimal(holding.quantity);
            let cost_basis = decimal(holding.cost_basis);
            let position_value = quantity * decimal(holding.price) * factor;
            let (quantity_coefficient, quantity_scale) = parts(quantity);
            let (cost_coefficient, cost_scale) = parts(cost_basis);
            let (value_coefficient, value_scale) = parts(position_value);
            let account_index = ACCOUNTS
                .iter()
                .position(|account| account.0 == holding.account)
                .unwrap_or_default();
            transaction.execute(
                "INSERT INTO broker_position_snapshots
                 (id, account_id, import_batch_id, report_date, instrument_id, quantity_coefficient, quantity_scale,
                  cost_basis_coefficient, cost_basis_scale, cost_basis_currency, position_value_coefficient,
                  position_value_scale, position_value_currency)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    format!("snapshot-{month}-{}", holding.instrument),
                    holding.account,
                    format!("aaaaaaaa-aaaa-4aaa-8aaa-{account_index:012}"),
                    date.to_string(),
                    holding.instrument,
                    quantity_coefficient,
                    quantity_scale,
                    cost_coefficient,
                    cost_scale,
                    holding.currency,
                    value_coefficient,
                    value_scale,
                    holding.currency,
                ],
            )?;
        }
        let (rate_coefficient, rate_scale) = parts(Decimal::new(74, 2));
        transaction.execute(
            "INSERT OR REPLACE INTO historical_fx_rates
             (base_currency, quote_currency, rate_date, rate_coefficient, rate_scale, source)
             VALUES ('USD', 'GBP', ?1, ?2, ?3, 'demo_ecb')",
            params![date.to_string(), rate_coefficient, rate_scale],
        )?;
    }

    let (fx_coefficient, fx_scale) = parts(Decimal::new(74, 2));
    transaction.execute(
        "INSERT INTO fx_rates (base_currency, quote_currency, rate_coefficient, rate_scale, as_of, source)
         VALUES ('USD', 'GBP', ?1, ?2, ?3, 'demo_ecb')",
        params![fx_coefficient, fx_scale, format!("{today}T16:00:00+00:00")],
    )?;

    for (index, (amount, instrument)) in [
        ("86.40", "GB0009895292"),
        ("54.00", "GB00B10RZP78"),
        ("112.50", "GB0005603997"),
        ("42.75", "GB00BP6MXD84"),
    ]
    .iter()
    .enumerate()
    {
        let holding = HOLDINGS
            .iter()
            .find(|holding| holding.instrument == *instrument)
            .expect("demo income instrument");
        let (coefficient, scale) = parts(decimal(amount));
        let account_index = ACCOUNTS
            .iter()
            .position(|account| account.0 == holding.account)
            .unwrap_or_default();
        transaction.execute(
            "INSERT INTO events (id, account_id, import_batch_id, source_id, event_type, occurred_at, description,
              amount_coefficient, amount_scale, currency, instrument_id)
             VALUES (?1, ?2, ?3, ?4, 'dividend', ?5, ?6, ?7, ?8, 'GBP', ?9)",
            params![
                format!("event-income-{index}"),
                holding.account,
                format!("aaaaaaaa-aaaa-4aaa-8aaa-{account_index:012}"),
                format!("demo-income-{index}"),
                format!("{today}T08:00:00+00:00"),
                format!("Dividend from {}", holding.name),
                coefficient,
                scale,
                holding.instrument,
            ],
        )?;
    }

    transaction.commit()?;
    Ok(())
}
