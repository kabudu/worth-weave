use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::models::{
    ActivityEvent, Holding, IncomeSummary, PerformanceHistory, PerformancePoint, ReconciliationItem,
};

fn exact(coefficient: Option<String>, scale: Option<u32>) -> Option<Decimal> {
    let coefficient = coefficient?.parse::<i128>().ok()?;
    Some(Decimal::from_i128_with_scale(coefficient, scale?))
}

pub fn activity(connection: &Connection, limit: u32) -> Result<Vec<ActivityEvent>> {
    let limit = limit.clamp(1, 500);
    let mut statement = connection.prepare(
        "SELECT e.id, e.account_id, a.display_name, a.broker, e.event_type, e.occurred_at,
                e.description, e.amount_coefficient, e.amount_scale, e.currency,
                e.quantity_coefficient, e.quantity_scale, e.instrument_id, i.symbol, i.name
         FROM events e JOIN accounts a ON a.id = e.account_id
         LEFT JOIN instruments i ON i.id = e.instrument_id
         ORDER BY e.occurred_at DESC, e.id DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([limit], |row| {
        let amount = exact(row.get(7)?, row.get(8)?).map(|value| value.to_string());
        let quantity = exact(row.get(10)?, row.get(11)?).map(|value| value.to_string());
        Ok(ActivityEvent {
            id: row.get(0)?,
            account_id: row.get(1)?,
            account_name: row.get(2)?,
            broker: row.get(3)?,
            event_type: row.get(4)?,
            occurred_at: row.get(5)?,
            description: row.get(6)?,
            amount,
            currency: row.get(9)?,
            quantity,
            instrument_id: row.get(12)?,
            symbol: row.get(13)?,
            instrument_name: row.get(14)?,
        })
    })?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

#[derive(Default)]
struct Position {
    account_name: String,
    broker: String,
    quantity: Decimal,
    cost_basis: Decimal,
    currency: Option<String>,
    basis_complete: bool,
    symbol: Option<String>,
    name: Option<String>,
    asset_class: Option<String>,
    sector: Option<String>,
    geography: Option<String>,
    applied_adjustments: BTreeSet<&'static str>,
}

// Some Trading 212 activity exports omit corporate-action rows while mixing
// pre-action purchases with post-action sales. Keep verified adjustments here
// until the broker supplies them in its export format.
const VERIFIED_QUANTITY_ADJUSTMENTS: [(&str, &str, &str, i64, i64); 10] = [
    (
        "castor-2021-reverse-split",
        "MHY1146L2082",
        "2021-05-28",
        1,
        10,
    ),
    (
        "comsovereign-2023-reverse-split",
        "US2056504010",
        "2023-02-10",
        1,
        100,
    ),
    (
        "contextlogic-2023-reverse-split",
        "US21078F1093",
        "2023-04-12",
        1,
        30,
    ),
    (
        "cinovec-2023-reverse-split",
        "US1724063086",
        "2023-06-09",
        1,
        20,
    ),
    (
        "hepion-2023-reverse-split",
        "US4268974015",
        "2023-05-11",
        1,
        20,
    ),
    (
        "pavmed-2023-reverse-split",
        "US70387R5028",
        "2023-12-07",
        1,
        15,
    ),
    (
        "bimi-2022-reverse-split",
        "US05552Q3011",
        "2022-12-12",
        1,
        10,
    ),
    (
        "bini-2025-june-reverse-split",
        "US62526P8775",
        "2025-06-02",
        1,
        100,
    ),
    (
        "bini-2025-august-reverse-split",
        "US62526P8775",
        "2025-08-04",
        1,
        250,
    ),
    (
        "bini-2025-september-reverse-split",
        "US62526P8775",
        "2025-09-22",
        1,
        250,
    ),
];

fn apply_verified_quantity_adjustments(
    instrument_id: &str,
    occurred_at: &str,
    position: &mut Position,
) {
    for (id, adjusted_instrument, effective_date, numerator, denominator) in
        VERIFIED_QUANTITY_ADJUSTMENTS
    {
        if instrument_id == adjusted_instrument
            && occurred_at >= effective_date
            && position.applied_adjustments.insert(id)
        {
            position.quantity = (position.quantity * Decimal::from(numerator)
                / Decimal::from(denominator))
            .round_dp(8);
        }
    }
}

fn is_position_corporate_action(event_type: &str, description: &str) -> bool {
    if event_type == "corporate_action" {
        return true;
    }
    let description = description.to_ascii_lowercase();
    event_type == "other"
        && (description.contains("split") || description.contains("cusip/isin change"))
}

fn predecessor_instrument_id(description: &str, current_instrument_id: &str) -> Option<String> {
    description
        .split(|character: char| !character.is_ascii_alphanumeric())
        .find(|candidate| {
            candidate.len() == 12
                && *candidate != current_instrument_id
                && candidate.as_bytes()[..2]
                    .iter()
                    .all(u8::is_ascii_alphabetic)
                && candidate.as_bytes()[2..]
                    .iter()
                    .all(u8::is_ascii_alphanumeric)
        })
        .map(str::to_owned)
}

fn ledger_holdings(connection: &Connection) -> Result<Vec<Holding>> {
    let mut statement = connection.prepare(
        "SELECT e.account_id, a.display_name, a.broker, e.instrument_id, e.event_type,
                e.amount_coefficient, e.amount_scale, e.currency,
                e.quantity_coefficient, e.quantity_scale, i.symbol, i.name,
                i.asset_class, i.sector, i.geography, e.description, e.occurred_at
         FROM events e JOIN accounts a ON a.id = e.account_id
         LEFT JOIN instruments i ON i.id=e.instrument_id
         WHERE e.instrument_id IS NOT NULL AND (
             e.event_type IN ('buy', 'sell', 'transfer', 'corporate_action')
             OR (e.event_type='other' AND (lower(e.description) LIKE '%split%' OR lower(e.description) LIKE '%cusip/isin change%'))
         )
         ORDER BY e.occurred_at,
           CASE WHEN e.event_type='corporate_action'
                  OR (e.event_type='other' AND (lower(e.description) LIKE '%split%' OR lower(e.description) LIKE '%cusip/isin change%'))
             THEN CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL)
             ELSE 0 END,
           e.id",
    )?;
    let mut rows = statement.query([])?;
    let mut positions: BTreeMap<(String, String, String), Position> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let account_id: String = row.get(0)?;
        let account_name: String = row.get(1)?;
        let broker: String = row.get(2)?;
        let instrument_id: String = row.get(3)?;
        let event_type: String = row.get(4)?;
        let amount = exact(row.get(5)?, row.get(6)?).unwrap_or_default().abs();
        let raw_quantity = exact(row.get(8)?, row.get(9)?).unwrap_or_default();
        let quantity = raw_quantity.abs();
        let currency: Option<String> = row.get(7)?;
        let symbol: Option<String> = row.get(10)?;
        let name: Option<String> = row.get(11)?;
        let asset_class: Option<String> = row.get(12)?;
        let sector: Option<String> = row.get(13)?;
        let geography: Option<String> = row.get(14)?;
        let description: String = row.get(15)?;
        let occurred_at: String = row.get(16)?;
        if quantity.is_zero() {
            continue;
        }
        let is_corporate_action = is_position_corporate_action(&event_type, &description);
        if is_corporate_action
            && let Some(predecessor_id) = predecessor_instrument_id(&description, &instrument_id)
        {
            let predecessor_key = (
                account_id.clone(),
                predecessor_id,
                currency.clone().unwrap_or_default(),
            );
            if let Some(mut predecessor) = positions.remove(&predecessor_key) {
                // IBKR's action row reports the exact post-action quantity. Carry the
                // economic cost history to the new identifier, but trust that quantity
                // rather than trying to infer fractional-share treatment.
                predecessor.quantity = quantity;
                predecessor.symbol = symbol.clone().or(predecessor.symbol);
                predecessor.name = name.clone().or(predecessor.name);
                predecessor.asset_class = asset_class.clone().or(predecessor.asset_class);
                positions.insert(
                    (
                        account_id.clone(),
                        instrument_id.clone(),
                        currency.clone().unwrap_or_default(),
                    ),
                    predecessor,
                );
                continue;
            }
        }
        let position = positions
            .entry((
                account_id,
                instrument_id.clone(),
                currency.clone().unwrap_or_default(),
            ))
            .or_insert_with(|| Position {
                account_name,
                broker,
                currency,
                basis_complete: true,
                symbol,
                name,
                asset_class,
                sector,
                geography,
                ..Position::default()
            });
        apply_verified_quantity_adjustments(&instrument_id, &occurred_at, position);
        if is_corporate_action {
            let description = description.to_lowercase();
            if description.contains("stock split open") {
                position.quantity += quantity;
            } else if description.contains("stock split close") {
                if quantity > position.quantity {
                    position.basis_complete = false;
                }
                position.quantity -= quantity;
            } else if description.contains("split") || description.contains("cusip/isin change") {
                // A standalone IBKR action row contains the resulting quantity even when
                // the predecessor activity falls outside the export. It is authoritative
                // for quantity, but cannot establish historical cost by itself.
                position.quantity = quantity;
                position.basis_complete = false;
            } else {
                position.basis_complete = false;
            }
        } else if event_type == "transfer" {
            position.quantity += raw_quantity;
            // Transfers establish quantity but generally do not carry acquisition cost in
            // the activity section. The latest broker snapshot can still supply that basis.
            position.basis_complete = false;
        } else if event_type == "buy" {
            position.quantity += quantity;
            position.cost_basis += amount;
        } else {
            if quantity > position.quantity {
                if position.broker == "trading_212" {
                    position.quantity = Decimal::ZERO;
                } else {
                    position.quantity -= quantity;
                }
                position.cost_basis = Decimal::ZERO;
                position.basis_complete = false;
            } else {
                if !position.quantity.is_zero() {
                    position.cost_basis -= (position.cost_basis / position.quantity) * quantity;
                }
                position.quantity -= quantity;
            }
        }
    }
    for ((_, instrument_id, _), position) in &mut positions {
        apply_verified_quantity_adjustments(instrument_id, "9999-12-31", position);
    }
    Ok(positions
        .into_iter()
        .filter(|(_, position)| !position.quantity.is_zero())
        .map(|((account_id, instrument_id, _), position)| {
            let average_cost = if position.quantity.is_zero() || !position.basis_complete {
                None
            } else {
                Some(
                    (position.cost_basis / position.quantity)
                        .normalize()
                        .to_string(),
                )
            };
            Holding {
                account_id,
                account_name: position.account_name,
                broker: position.broker,
                instrument_id,
                symbol: position.symbol,
                name: position.name,
                asset_class: position.asset_class,
                sector: position.sector,
                geography: position.geography,
                quantity: position.quantity.normalize().to_string(),
                cost_basis: (position.basis_complete && position.currency.is_some())
                    .then(|| position.cost_basis.normalize().to_string()),
                average_cost,
                currency: position.currency,
                cost_basis_complete: position.basis_complete,
            }
        })
        .collect())
}

pub fn holdings(connection: &Connection) -> Result<Vec<Holding>> {
    let ledger = ledger_holdings(connection)?;
    let mut ledger_by_instrument: BTreeMap<(String, String), Holding> = ledger
        .into_iter()
        .map(|holding| {
            (
                (holding.account_id.clone(), holding.instrument_id.clone()),
                holding,
            )
        })
        .collect();
    let mut accounts_with_snapshots = std::collections::BTreeSet::new();
    let mut current = Vec::new();
    let mut statement = connection.prepare(
        "SELECT p.account_id, p.instrument_id, p.quantity_coefficient, p.quantity_scale,
                a.display_name, a.broker, i.symbol, i.name, i.asset_class, i.sector, i.geography,
                p.cost_basis_coefficient, p.cost_basis_scale, p.cost_basis_currency
         FROM broker_position_snapshots p
         JOIN accounts a ON a.id=p.account_id
         LEFT JOIN instruments i ON i.id=p.instrument_id
         JOIN (
           SELECT account_id, MAX(report_date) AS report_date
           FROM broker_position_snapshots GROUP BY account_id
         ) latest ON latest.account_id=p.account_id AND latest.report_date=p.report_date
         ORDER BY a.display_name, p.instrument_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            exact(row.get(2)?, row.get(3)?).unwrap_or_default(),
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, Option<String>>(10)?,
            exact(row.get(11)?, row.get(12)?),
            row.get::<_, Option<String>>(13)?,
        ))
    })?;
    for row in rows {
        let (
            account_id,
            instrument_id,
            quantity,
            account_name,
            broker,
            symbol,
            name,
            asset_class,
            sector,
            geography,
            broker_cost_basis,
            broker_cost_currency,
        ) = row?;
        accounts_with_snapshots.insert(account_id.clone());
        if quantity.is_zero() {
            continue;
        }
        let key = (account_id.clone(), instrument_id.clone());
        if let Some(mut holding) = ledger_by_instrument.remove(&key) {
            if let (Some(cost_basis), Some(currency)) = (broker_cost_basis, broker_cost_currency) {
                holding.cost_basis = Some(cost_basis.normalize().to_string());
                holding.average_cost =
                    (!quantity.is_zero()).then(|| (cost_basis / quantity).normalize().to_string());
                holding.currency = Some(currency);
                holding.cost_basis_complete = true;
            } else if holding.quantity.parse::<Decimal>().ok() != Some(quantity) {
                holding.cost_basis = None;
                holding.average_cost = None;
                holding.cost_basis_complete = false;
            }
            holding.quantity = quantity.normalize().to_string();
            current.push(holding);
        } else {
            current.push(Holding {
                account_id,
                account_name,
                broker,
                instrument_id,
                symbol,
                name,
                asset_class,
                sector,
                geography,
                quantity: quantity.normalize().to_string(),
                cost_basis: broker_cost_basis.map(|value| value.normalize().to_string()),
                average_cost: broker_cost_basis.and_then(|value| {
                    (!quantity.is_zero()).then(|| (value / quantity).normalize().to_string())
                }),
                currency: broker_cost_currency,
                cost_basis_complete: broker_cost_basis.is_some(),
            });
        }
    }
    current.extend(
        ledger_by_instrument
            .into_values()
            .filter(|holding| !accounts_with_snapshots.contains(&holding.account_id)),
    );
    current.sort_by(|left, right| {
        left.account_name
            .cmp(&right.account_name)
            .then(left.instrument_id.cmp(&right.instrument_id))
    });
    Ok(current)
}

pub fn income(connection: &Connection) -> Result<Vec<IncomeSummary>> {
    let mut statement = connection.prepare(
        "SELECT e.event_type, e.amount_coefficient, e.amount_scale, e.currency
         FROM events e WHERE e.event_type IN ('dividend', 'interest') AND e.currency IS NOT NULL",
    )?;
    let mut rows = statement.query([])?;
    let mut totals: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let event_type: String = row.get(0)?;
        let amount = exact(row.get(1)?, row.get(2)?).unwrap_or_default().abs();
        let currency: String = row.get(3)?;
        let entry = totals.entry(currency).or_default();
        if event_type == "dividend" {
            entry.0 += amount;
        } else {
            entry.1 += amount;
        }
    }
    Ok(totals
        .into_iter()
        .map(|(currency, (dividends, interest))| IncomeSummary {
            currency,
            dividends: dividends.normalize().to_string(),
            interest: interest.normalize().to_string(),
            total: (dividends + interest).normalize().to_string(),
        })
        .collect())
}

pub fn performance_history(connection: &Connection, scope: &str) -> Result<PerformanceHistory> {
    let reporting_currency = crate::db::settings(connection)?
        .reporting_currency
        .unwrap_or_else(|| "GBP".into());
    let signature: String = connection.query_row(
        "SELECT printf('history-v5|%s|%s|%s|%s|%s|%s|%s',
           ?1,
           (SELECT COUNT(*) || ':' || COALESCE(MAX(imported_at),'') FROM import_batches),
           (SELECT COUNT(*) || ':' || COALESCE(MAX(fetched_at),'') FROM historical_prices),
           (SELECT COUNT(*) || ':' || COALESCE(MAX(as_of),'') FROM market_prices),
           (SELECT COUNT(*) || ':' || COALESCE(MAX(as_of),'') FROM fx_rates),
           (SELECT COUNT(*) || ':' || COALESCE(MAX(report_date),'') FROM broker_position_snapshots),
           date('now'))",
        [reporting_currency.as_str()],
        |row| row.get(0),
    )?;
    let cached_payload: Option<String> = connection
        .query_row(
            "SELECT payload FROM performance_history_cache WHERE scope=?1 AND signature=?2",
            params![scope, signature],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(payload) = cached_payload
        && let Ok(cached) = serde_json::from_str::<CachedPerformanceHistory>(&payload)
    {
        return Ok(PerformanceHistory {
            reporting_currency: cached.reporting_currency,
            scope: cached.scope,
            coverage: coverage_label(&cached.coverage),
            points: cached.points,
        });
    }
    let (filter, value) = if let Some(account_id) = scope.strip_prefix("account:") {
        (" AND p.account_id = ?1", Some(account_id))
    } else if let Some(broker) = scope.strip_prefix("broker:") {
        (" AND a.broker = ?1", Some(broker))
    } else {
        ("", None)
    };
    let sql = format!(
        "SELECT p.report_date, p.position_value_coefficient, p.position_value_scale, p.position_value_currency
         FROM broker_position_snapshots p JOIN accounts a ON a.id=p.account_id
         WHERE p.position_value_coefficient IS NOT NULL{filter}
         ORDER BY p.report_date"
    );
    let mut statement = connection.prepare(&sql)?;
    let mut totals: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut missing_conversion = false;
    let mut reconstructed = false;
    let mut consume = |row: &rusqlite::Row<'_>| -> rusqlite::Result<()> {
        let date: String = row.get(0)?;
        let coefficient: Option<String> = row.get(1)?;
        let scale: Option<u32> = row.get(2)?;
        let currency: Option<String> = row.get(3)?;
        if let (Some(amount), Some(currency)) = (exact(coefficient, scale), currency) {
            match crate::market::convert_amount(connection, amount, &currency, &reporting_currency)
            {
                Ok(Some(converted)) => *totals.entry(date).or_default() += converted,
                _ => missing_conversion = true,
            }
        }
        Ok(())
    };
    if let Some(value) = value {
        let mut rows = statement.query([value])?;
        while let Some(row) = rows.next()? {
            consume(row)?;
        }
    } else {
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            consume(row)?;
        }
    }
    let history_start: String = connection.query_row(
        "SELECT COALESCE(MIN(substr(occurred_at,1,10)), date('now')) FROM events WHERE quantity_coefficient IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    let mut reconstructed_statement = connection.prepare(
        "WITH unit_cost_bounds AS (
           SELECT instrument_id,
             MAX(ABS(CAST(amount_coefficient AS REAL) * CAST('1e-' || amount_scale AS REAL)) /
                 ABS(CAST(quantity_coefficient AS REAL) * CAST('1e-' || quantity_scale AS REAL))) AS max_unit_cost
           FROM events
           WHERE event_type IN ('buy','sell') AND amount_coefficient IS NOT NULL
             AND quantity_coefficient IS NOT NULL AND CAST(quantity_coefficient AS REAL)<>0
           GROUP BY instrument_id
         ), candidate_dates AS (
           SELECT DISTINCT price_date FROM historical_prices
           WHERE price_date>=?1
             AND (price_date>=date('now','-180 days') OR strftime('%w', price_date)='5'
               OR price_date=(SELECT MIN(price_date) FROM historical_prices WHERE price_date>=?1))
         ), daily_coverage AS (
           SELECT price_date, COUNT(DISTINCT instrument_id) AS priced
           FROM historical_prices
           WHERE price_date IN (SELECT price_date FROM candidate_dates)
           GROUP BY price_date
         ), coverage_window AS (
           SELECT price_date, priced,
             MAX(priced) OVER (ORDER BY price_date ROWS BETWEEN 20 PRECEDING AND 20 FOLLOWING) AS nearby_max
           FROM daily_coverage
         ), eligible_dates AS (
           SELECT price_date FROM coverage_window WHERE priced * 5 >= nearby_max * 4
         )
         SELECT hp.price_date, hp.price_coefficient, hp.price_scale, hp.currency,
                e.account_id, a.broker,
                SUM((CASE
                  WHEN e.event_type='buy' THEN ABS(CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL))
                  WHEN e.event_type='sell' THEN -ABS(CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL))
                  WHEN e.event_type='transfer' THEN CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL)
                  WHEN e.event_type='corporate_action' AND lower(e.description) LIKE '%stock split close%' THEN -ABS(CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL))
                  WHEN e.event_type='corporate_action' AND lower(e.description) LIKE '%stock split open%' THEN ABS(CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL))
                  WHEN e.event_type='corporate_action' THEN CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL)
                  WHEN e.event_type='other' AND (lower(e.description) LIKE '%split%' OR lower(e.description) LIKE '%cusip/isin change%')
                    THEN CAST(e.quantity_coefficient AS REAL) * CAST('1e-' || e.quantity_scale AS REAL)
                  ELSE 0 END) *
                  CASE
                    WHEN e.instrument_id='MHY1146L2082' THEN CASE WHEN substr(e.occurred_at,1,10)<'2021-05-28' AND hp.price_date>='2021-05-28' THEN 0.1 ELSE 1 END
                    WHEN e.instrument_id='US2056504010' THEN CASE WHEN substr(e.occurred_at,1,10)<'2023-02-10' AND hp.price_date>='2023-02-10' THEN 0.01 ELSE 1 END
                    WHEN e.instrument_id='US21078F1093' THEN CASE WHEN substr(e.occurred_at,1,10)<'2023-04-12' AND hp.price_date>='2023-04-12' THEN (1.0/30.0) ELSE 1 END
                    WHEN e.instrument_id='US1724063086' THEN CASE WHEN substr(e.occurred_at,1,10)<'2023-06-09' AND hp.price_date>='2023-06-09' THEN 0.05 ELSE 1 END
                    WHEN e.instrument_id='US4268974015' THEN CASE WHEN substr(e.occurred_at,1,10)<'2023-05-11' AND hp.price_date>='2023-05-11' THEN 0.05 ELSE 1 END
                    WHEN e.instrument_id='US70387R5028' THEN CASE WHEN substr(e.occurred_at,1,10)<'2023-12-07' AND hp.price_date>='2023-12-07' THEN (1.0/15.0) ELSE 1 END
                    WHEN e.instrument_id='US05552Q3011' THEN CASE WHEN substr(e.occurred_at,1,10)<'2022-12-12' AND hp.price_date>='2022-12-12' THEN 0.1 ELSE 1 END
                    WHEN e.instrument_id='US62526P8775' THEN
                      (CASE WHEN substr(e.occurred_at,1,10)<'2025-06-02' AND hp.price_date>='2025-06-02' THEN 0.01 ELSE 1 END) *
                      (CASE WHEN substr(e.occurred_at,1,10)<'2025-08-04' AND hp.price_date>='2025-08-04' THEN 0.004 ELSE 1 END) *
                      (CASE WHEN substr(e.occurred_at,1,10)<'2025-09-22' AND hp.price_date>='2025-09-22' THEN 0.004 ELSE 1 END)
                    ELSE 1
                  END) AS quantity,
                (SELECT EXISTS(SELECT 1 FROM coverage_window WHERE priced * 5 < nearby_max * 4)) AS has_coverage_gaps
         FROM historical_prices hp
         JOIN eligible_dates eligible ON eligible.price_date=hp.price_date
         JOIN events e ON e.instrument_id=hp.instrument_id AND substr(e.occurred_at,1,10)<=hp.price_date
         JOIN accounts a ON a.id=e.account_id
         LEFT JOIN instruments i ON i.id=e.instrument_id
         LEFT JOIN unit_cost_bounds costs ON costs.instrument_id=e.instrument_id
         WHERE e.quantity_coefficient IS NOT NULL AND (
             e.event_type IN ('buy','sell','transfer','corporate_action')
             OR (e.event_type='other' AND (lower(e.description) LIKE '%split%' OR lower(e.description) LIKE '%cusip/isin change%'))
         )
           AND NOT (i.symbol IN ('PHE','PREM') AND hp.currency<>'GBP')
           AND (costs.max_unit_cost IS NULL OR
             CAST(hp.price_coefficient AS REAL) * CAST('1e-' || hp.price_scale AS REAL) <= costs.max_unit_cost * 10
             OR EXISTS (SELECT 1 FROM events action WHERE action.instrument_id=e.instrument_id AND action.event_type='corporate_action'))
           AND e.instrument_id NOT IN (
             'MHY1146L2082','US2056504010','US21078F1093','US1724063086',
             'US4268974015','US70387R5028','US05552Q3011','US62526P8775'
           )
           AND NOT EXISTS (SELECT 1 FROM broker_position_snapshots bp
             WHERE bp.account_id=e.account_id AND bp.report_date=hp.price_date
               AND bp.position_value_coefficient IS NOT NULL)
         GROUP BY hp.price_date, hp.instrument_id, e.account_id
         HAVING quantity > 0 ORDER BY hp.price_date",
    )?;
    let mut rows = reconstructed_statement.query([history_start])?;
    while let Some(row) = rows.next()? {
        let account_id: String = row.get(4)?;
        let broker: String = row.get(5)?;
        if scope
            .strip_prefix("account:")
            .is_some_and(|wanted| wanted != account_id)
            || scope
                .strip_prefix("broker:")
                .is_some_and(|wanted| wanted != broker)
        {
            continue;
        }
        let date: String = row.get(0)?;
        let price = exact(row.get(1)?, row.get(2)?).unwrap_or_default();
        let currency: String = row.get(3)?;
        if currency != reporting_currency {
            missing_conversion = true;
        }
        let quantity = Decimal::from_f64_retain(row.get::<_, f64>(6)?).unwrap_or_default();
        if row.get::<_, bool>(7)? {
            missing_conversion = true;
        }
        match crate::market::convert_amount(
            connection,
            price * quantity,
            &currency,
            &reporting_currency,
        )? {
            Some(converted) => {
                *totals.entry(date).or_default() += converted;
                reconstructed = true;
            }
            None => missing_conversion = true,
        }
    }
    if scope == "all" {
        let mut snapshots = connection.prepare(
            "SELECT substr(captured_at, 1, 10), total_coefficient, total_scale FROM portfolio_snapshots ORDER BY captured_at"
        )?;
        let mut rows = snapshots.query([])?;
        while let Some(row) = rows.next()? {
            if let Some(value) = exact(row.get(1)?, row.get(2)?) {
                totals.insert(row.get(0)?, value);
            }
        }
    }
    let current_valuation = crate::market::valuation(connection)?;
    let current_total = current_valuation
        .holdings
        .iter()
        .filter(|holding| {
            scope
                .strip_prefix("account:")
                .is_none_or(|wanted| holding.holding.account_id == wanted)
                && scope
                    .strip_prefix("broker:")
                    .is_none_or(|wanted| holding.holding.broker == wanted)
        })
        .filter_map(|holding| holding.reporting_value.as_deref())
        .filter_map(|value| value.parse::<Decimal>().ok())
        .sum::<Decimal>();
    if current_total > Decimal::ZERO {
        totals.insert(Utc::now().format("%Y-%m-%d").to_string(), current_total);
    }
    let points = totals
        .into_iter()
        .map(|(date, value)| PerformancePoint {
            date,
            value: value.normalize().to_string(),
        })
        .collect();
    let result = PerformanceHistory {
        reporting_currency,
        scope: scope.into(),
        coverage: if missing_conversion {
            "partial"
        } else if reconstructed {
            "market_reconstructed"
        } else {
            "broker_imports"
        },
        points,
    };
    let payload = serde_json::to_string(&CachedPerformanceHistory {
        reporting_currency: result.reporting_currency.clone(),
        scope: result.scope.clone(),
        coverage: result.coverage.into(),
        points: result.points.clone(),
    })
    .map_err(|_| {
        crate::error::WorthweaveError::InvalidMarketData("could not cache portfolio history".into())
    })?;
    connection.execute(
        "INSERT INTO performance_history_cache (scope, signature, payload) VALUES (?1, ?2, ?3)
         ON CONFLICT(scope) DO UPDATE SET signature=excluded.signature, payload=excluded.payload, updated_at=CURRENT_TIMESTAMP",
        params![scope, signature, payload],
    )?;
    Ok(result)
}

#[derive(Serialize, Deserialize)]
struct CachedPerformanceHistory {
    reporting_currency: String,
    scope: String,
    coverage: String,
    points: Vec<PerformancePoint>,
}

fn coverage_label(value: &str) -> &'static str {
    match value {
        "partial" => "partial",
        "market_reconstructed" => "market_reconstructed",
        _ => "broker_imports",
    }
}

pub fn reconciliation(connection: &Connection) -> Result<Vec<ReconciliationItem>> {
    let ledger: BTreeMap<(String, String), Holding> = ledger_holdings(connection)?
        .into_iter()
        .map(|holding| {
            (
                (holding.account_id.clone(), holding.instrument_id.clone()),
                holding,
            )
        })
        .collect();
    let mut statement = connection.prepare(
        "SELECT p.account_id, p.instrument_id, p.report_date, a.display_name,
                p.quantity_coefficient, p.quantity_scale
         FROM broker_position_snapshots p
         JOIN accounts a ON a.id=p.account_id
         JOIN (
           SELECT account_id, MAX(report_date) AS report_date
           FROM broker_position_snapshots GROUP BY account_id
         ) latest ON latest.account_id=p.account_id AND latest.report_date=p.report_date",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
            (
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                exact(row.get(4)?, row.get(5)?).unwrap_or_default(),
            ),
        ))
    })?;
    let mut result = Vec::new();
    for row in rows {
        let ((account_id, instrument_id), (date, account_name, broker_quantity)) = row?;
        if broker_quantity.is_zero() {
            continue;
        }
        let ledger_quantity = ledger
            .get(&(account_id.clone(), instrument_id.clone()))
            .and_then(|holding| holding.quantity.parse::<Decimal>().ok());
        let difference = ledger_quantity.map(|quantity| quantity - broker_quantity);
        let status = match difference {
            Some(value) if value.is_zero() => "matched",
            Some(_) => "mismatch",
            None => "unavailable",
        };
        result.push(ReconciliationItem {
            account_id,
            account_name,
            instrument_id,
            as_of: Some(date),
            ledger_quantity: ledger_quantity.unwrap_or_default().normalize().to_string(),
            broker_quantity: Some(broker_quantity.normalize().to_string()),
            difference: difference.map(|value| value.normalize().to_string()),
            status,
        });
    }
    result.sort_by(|left, right| {
        left.account_name
            .cmp(&right.account_name)
            .then(left.instrument_id.cmp(&right.instrument_id))
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE accounts (id TEXT PRIMARY KEY, display_name TEXT, broker TEXT);
                 CREATE TABLE app_settings (id INTEGER PRIMARY KEY, reporting_currency TEXT, onboarding_complete INTEGER, ai_onboarding_complete INTEGER, ai_runtime TEXT, ai_model TEXT, ai_endpoint TEXT);
                 CREATE TABLE instruments (id TEXT PRIMARY KEY, symbol TEXT, name TEXT, asset_class TEXT, sector TEXT, geography TEXT);
                 CREATE TABLE events (
                   id TEXT PRIMARY KEY, account_id TEXT, instrument_id TEXT, event_type TEXT,
                   amount_coefficient TEXT, amount_scale INTEGER, currency TEXT,
                   quantity_coefficient TEXT, quantity_scale INTEGER, description TEXT,
                   occurred_at TEXT
                 );
                 CREATE TABLE import_batches (imported_at TEXT);
                 CREATE TABLE historical_prices (instrument_id TEXT, price_date TEXT, price_coefficient TEXT, price_scale INTEGER, currency TEXT, fetched_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 CREATE TABLE broker_position_snapshots (account_id TEXT, report_date TEXT, instrument_id TEXT, quantity_coefficient TEXT, quantity_scale INTEGER, cost_basis_coefficient TEXT, cost_basis_scale INTEGER, cost_basis_currency TEXT, position_value_coefficient TEXT, position_value_scale INTEGER, position_value_currency TEXT);
                 CREATE TABLE market_prices (instrument_id TEXT, price_coefficient TEXT, price_scale INTEGER, currency TEXT, as_of TEXT, source TEXT);
                 CREATE TABLE fx_rates (base_currency TEXT, quote_currency TEXT, rate_coefficient TEXT, rate_scale INTEGER, as_of TEXT, source TEXT);
                 CREATE TABLE performance_history_cache (scope TEXT PRIMARY KEY, signature TEXT, payload TEXT, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 INSERT INTO app_settings VALUES (1, 'USD', 1, 0, NULL, NULL, NULL);
                 INSERT INTO accounts VALUES ('account', 'IBKR ISA', 'ibkr');",
            )
            .expect("projection schema");
        connection
    }

    fn add_event(
        connection: &Connection,
        id: &str,
        instrument_id: &str,
        event_type: &str,
        quantity: &str,
        description: &str,
        occurred_at: &str,
    ) {
        let quantity = quantity.parse::<Decimal>().expect("decimal quantity");
        connection
            .execute(
                "INSERT OR IGNORE INTO instruments (id, symbol) VALUES (?1, ?1)",
                [instrument_id],
            )
            .expect("instrument");
        connection
            .execute(
                "INSERT INTO events VALUES (?1, 'account', ?2, ?3, NULL, NULL, 'USD', ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    instrument_id,
                    event_type,
                    quantity.mantissa().to_string(),
                    quantity.scale(),
                    description,
                    occurred_at
                ],
            )
            .expect("event");
    }

    #[test]
    fn verified_reverse_split_normalizes_pre_split_quantity_once() {
        let mut position = Position {
            quantity: Decimal::from(5095),
            ..Position::default()
        };
        apply_verified_quantity_adjustments("US21078F1093", "2024-02-21T19:26:36", &mut position);
        assert_eq!(position.quantity.to_string(), "169.83333333");
        apply_verified_quantity_adjustments("US21078F1093", "2025-06-02T14:45:45", &mut position);
        assert_eq!(position.quantity.to_string(), "169.83333333");
        position.quantity -= "0.83333333".parse::<Decimal>().expect("fractional sale");
        position.quantity -= Decimal::from(169);
        assert!(position.quantity.is_zero());
    }

    #[test]
    fn adjustment_after_final_transaction_is_applied_during_projection() {
        let mut position = Position {
            quantity: Decimal::from(105),
            ..Position::default()
        };
        apply_verified_quantity_adjustments("US62526P8775", "9999-12-31", &mut position);
        assert_eq!(position.quantity.to_string(), "0.0000168");
    }

    #[test]
    fn adjustment_before_first_transaction_does_not_change_later_purchase() {
        let mut position = Position::default();
        apply_verified_quantity_adjustments("US05552Q3011", "2023-01-01", &mut position);
        position.quantity += Decimal::from(12);
        apply_verified_quantity_adjustments("US05552Q3011", "9999-12-31", &mut position);
        assert_eq!(position.quantity, Decimal::from(12));
    }

    #[test]
    fn transfers_and_ibkr_identifier_changes_reconcile_to_resulting_quantity() {
        let connection = projection_connection();
        add_event(
            &connection,
            "transfer",
            "US00847G7051",
            "transfer",
            "811",
            "AGENUS INC",
            "2024-03-18",
        );
        add_event(
            &connection,
            "old-leg",
            "US00847G7051",
            "other",
            "-971",
            "AGEN(US00847G7051) SPLIT 1 FOR 20 (AGEN.OLD, AGENUS INC, US00847G7051)",
            "2024-04-11",
        );
        add_event(
            &connection,
            "new-leg",
            "US00847G8042",
            "other",
            "48.55",
            "AGEN(US00847G7051) SPLIT 1 FOR 20 (AGEN, AGENUS INC, US00847G8042)",
            "2024-04-11",
        );
        add_event(
            &connection,
            "fractional-sale",
            "US00847G8042",
            "sell",
            "-0.55",
            "AGENUS INC",
            "2024-04-11T20:26:00",
        );

        let holdings = ledger_holdings(&connection).expect("holdings");
        assert_eq!(
            holdings.len(),
            1,
            "{:#?}",
            holdings
                .iter()
                .map(|holding| (&holding.instrument_id, &holding.quantity))
                .collect::<Vec<_>>()
        );
        assert_eq!(holdings[0].instrument_id, "US00847G8042");
        assert_eq!(holdings[0].quantity, "48");
    }

    #[test]
    fn predecessor_parser_ignores_current_identifier() {
        assert_eq!(
            predecessor_instrument_id(
                "LITM(CA83336J3073) CUSIP/ISIN CHANGE TO (CA3591341035)",
                "CA3591341035"
            )
            .as_deref(),
            Some("CA83336J3073")
        );
    }

    #[test]
    fn performance_history_excludes_dates_with_partial_price_coverage() {
        let connection = projection_connection();
        add_event(
            &connection,
            "buy-a",
            "US0000000001",
            "buy",
            "1",
            "Holding A",
            "2026-01-01",
        );
        add_event(
            &connection,
            "buy-b",
            "US0000000002",
            "buy",
            "1",
            "Holding B",
            "2026-01-01",
        );
        connection.execute_batch(
            "INSERT INTO historical_prices (instrument_id, price_date, price_coefficient, price_scale, currency) VALUES ('US0000000001', '2026-07-09', '1000', 2, 'USD');
             INSERT INTO historical_prices (instrument_id, price_date, price_coefficient, price_scale, currency) VALUES ('US0000000002', '2026-07-09', '2000', 2, 'USD');
             INSERT INTO historical_prices (instrument_id, price_date, price_coefficient, price_scale, currency) VALUES ('US0000000001', '2026-07-10', '1100', 2, 'USD');"
        ).expect("historical prices");

        let history = performance_history(&connection, "account:account").expect("history");
        assert_eq!(history.points.len(), 1);
        assert_eq!(history.points[0].date, "2026-07-09");
        assert_eq!(history.points[0].value, "30");
        assert_eq!(history.coverage, "partial");
        let cached = performance_history(&connection, "account:account").expect("cached history");
        assert_eq!(cached.points.len(), history.points.len());
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM performance_history_cache WHERE scope='account:account'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .expect("cache row"),
            1
        );
    }
}
