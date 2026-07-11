use std::collections::BTreeMap;

use rusqlite::Connection;
use rust_decimal::Decimal;

use crate::error::Result;
use crate::models::{ActivityEvent, Holding, IncomeSummary, ReconciliationItem};

fn exact(coefficient: Option<String>, scale: Option<u32>) -> Option<Decimal> {
    let coefficient = coefficient?.parse::<i128>().ok()?;
    Some(Decimal::from_i128_with_scale(coefficient, scale?))
}

pub fn activity(connection: &Connection, limit: u32) -> Result<Vec<ActivityEvent>> {
    let limit = limit.clamp(1, 500);
    let mut statement = connection.prepare(
        "SELECT e.id, e.account_id, a.display_name, a.broker, e.event_type, e.occurred_at,
                e.description, e.amount_coefficient, e.amount_scale, e.currency,
                e.quantity_coefficient, e.quantity_scale, e.instrument_id
         FROM events e JOIN accounts a ON a.id = e.account_id
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
}

pub fn holdings(connection: &Connection) -> Result<Vec<Holding>> {
    let mut statement = connection.prepare(
        "SELECT e.account_id, a.display_name, a.broker, e.instrument_id, e.event_type,
                e.amount_coefficient, e.amount_scale, e.currency,
                e.quantity_coefficient, e.quantity_scale, i.symbol, i.name
         FROM events e JOIN accounts a ON a.id = e.account_id
         LEFT JOIN instruments i ON i.id=e.instrument_id
         WHERE e.instrument_id IS NOT NULL AND e.event_type IN ('buy', 'sell')
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
        if quantity.is_zero() {
            continue;
        }
        let position = positions
            .entry((
                account_id,
                instrument_id,
                currency.clone().unwrap_or_default(),
            ))
            .or_insert_with(|| Position {
                account_name,
                broker,
                currency,
                basis_complete: true,
                symbol,
                name,
                ..Position::default()
            });
        if event_type == "buy" {
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

pub fn reconciliation(connection: &Connection) -> Result<Vec<ReconciliationItem>> {
    let holdings = holdings(connection)?;
    let mut broker = BTreeMap::new();
    let mut statement = connection.prepare(
        "SELECT p.account_id, p.instrument_id, p.report_date,
                p.quantity_coefficient, p.quantity_scale
         FROM broker_position_snapshots p
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
                exact(row.get(3)?, row.get(4)?).unwrap_or_default(),
            ),
        ))
    })?;
    for row in rows {
        let (key, value) = row?;
        broker.insert(key, value);
    }

    let mut result = Vec::new();
    for holding in holdings {
        let ledger = holding.quantity.parse::<Decimal>().unwrap_or_default();
        let snapshot = broker.remove(&(holding.account_id.clone(), holding.instrument_id.clone()));
        let (as_of, broker_quantity, difference, status) = match snapshot {
            Some((date, quantity)) => {
                let delta = ledger - quantity;
                (
                    Some(date),
                    Some(quantity.normalize().to_string()),
                    Some(delta.normalize().to_string()),
                    if delta.is_zero() {
                        "matched"
                    } else {
                        "mismatch"
                    },
                )
            }
            None => (None, None, None, "unavailable"),
        };
        result.push(ReconciliationItem {
            account_id: holding.account_id,
            account_name: holding.account_name,
            instrument_id: holding.instrument_id,
            as_of,
            ledger_quantity: ledger.normalize().to_string(),
            broker_quantity,
            difference,
            status,
        });
    }
    for ((account_id, instrument_id), (date, quantity)) in broker {
        let account_name = connection.query_row(
            "SELECT display_name FROM accounts WHERE id=?1",
            [&account_id],
            |row| row.get(0),
        )?;
        result.push(ReconciliationItem {
            account_id,
            account_name,
            instrument_id,
            as_of: Some(date),
            ledger_quantity: "0".into(),
            broker_quantity: Some(quantity.normalize().to_string()),
            difference: Some((-quantity).normalize().to_string()),
            status: "mismatch",
        });
    }
    result.sort_by(|left, right| {
        left.account_name
            .cmp(&right.account_name)
            .then(left.instrument_id.cmp(&right.instrument_id))
    });
    Ok(result)
}
