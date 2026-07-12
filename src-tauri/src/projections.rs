use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;
use rust_decimal::Decimal;

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

fn ledger_holdings(connection: &Connection) -> Result<Vec<Holding>> {
    let mut statement = connection.prepare(
        "SELECT e.account_id, a.display_name, a.broker, e.instrument_id, e.event_type,
                e.amount_coefficient, e.amount_scale, e.currency,
                e.quantity_coefficient, e.quantity_scale, i.symbol, i.name,
                i.asset_class, i.sector, i.geography, e.description, e.occurred_at
         FROM events e JOIN accounts a ON a.id = e.account_id
         LEFT JOIN instruments i ON i.id=e.instrument_id
         WHERE e.instrument_id IS NOT NULL AND e.event_type IN ('buy', 'sell', 'corporate_action')
         ORDER BY e.occurred_at, e.id",
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
        let quantity = exact(row.get(8)?, row.get(9)?).unwrap_or_default().abs();
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
        if event_type == "corporate_action" {
            let description = description.to_lowercase();
            if description.contains("stock split open") {
                position.quantity += quantity;
            } else if description.contains("stock split close") {
                if quantity > position.quantity {
                    position.basis_complete = false;
                }
                position.quantity -= quantity;
            } else {
                position.basis_complete = false;
            }
        } else if event_type == "buy" {
            position.quantity += quantity;
            position.cost_basis += amount;
        } else {
            if quantity > position.quantity {
                position.quantity -= quantity;
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
        let mut snapshots = connection.prepare(
            "SELECT substr(captured_at, 1, 10), total_coefficient, total_scale, reporting_currency FROM portfolio_snapshots ORDER BY captured_at"
        )?;
        let mut rows = snapshots.query([])?;
        while let Some(row) = rows.next()? {
            consume(row)?;
        }
    }
    let points = totals
        .into_iter()
        .map(|(date, value)| PerformancePoint {
            date,
            value: value.normalize().to_string(),
        })
        .collect();
    Ok(PerformanceHistory {
        reporting_currency,
        scope: scope.into(),
        coverage: if missing_conversion {
            "partial"
        } else {
            "broker_imports"
        },
        points,
    })
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
}
