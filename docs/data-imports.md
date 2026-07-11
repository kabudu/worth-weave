# Broker data imports

## Supported sources

- Trading 212 transaction-history CSV exports with schema variations observed from 2020–2026.
- IBKR multi-section Activity Flex Query CSV exports produced by the Worthweave query configuration.

Every destination account is created explicitly as Invest or Stocks & Shares ISA. Trading 212 exports do not carry account type, so the import form requires confirmation and the native command rejects a mismatch with the destination account.

## Safety behavior

- Files larger than 50 MiB are rejected before parsing.
- Only the basename of a selected filename is retained.
- A SHA-256 content hash prevents importing the same file twice into one account.
- Broker transaction identifiers prevent the same event being duplicated by overlapping exports.
- Parsing and validation occur before the ledger transaction commits.
- Coverage dates are stored per import batch so missing historical periods can be surfaced.
- Values are stored as exact signed coefficients and decimal scales; display formatting never changes ledger precision.

## Current normalization scope

Trading 212 activity rows are normalized into buys, sells, dividends, deposits, withdrawals, interest, fees, corporate actions, and other events. IBKR trades, cash transactions, corporate actions, and transfers are normalized. Other IBKR sections contribute coverage and reconciliation context but are not yet persisted as first-class snapshot records.

Current holdings, cost-basis projections, daily portfolio snapshots, market prices, security classification, and return calculations are not yet exposed by the application. They are subsequent deterministic-ledger milestones; the interface deliberately shows an awaiting-data state rather than estimated values.
