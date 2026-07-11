# Broker data imports

## Supported sources

- Trading 212 transaction-history CSV exports with schema variations observed from 2020–2026.
- IBKR multi-section Activity Flex Query CSV exports produced by the Worthweave query configuration.

Robinhood is supported in the account model with region-specific legal wrappers: UK individual brokerage and Stocks & Shares ISA; US individual brokerage, JTWROS joint investing, Traditional IRA, Roth IRA, and UTMA custodial accounts. Robinhood US documents downloadable account-activity CSV reports but does not publish a stable column contract, while Robinhood UK currently documents monthly PDF statements. Robinhood imports remain disabled until representative anonymised exports can be validated with fixtures. The native boundary returns an explicit unsupported-format error rather than attempting another broker's parser.

Every destination account is created with an explicit jurisdiction and legal account type. Trading 212 exports do not carry account type, so the import form requires confirmation and the native command rejects a mismatch with the destination account.

## Safety behavior

- Files larger than 50 MiB are rejected before parsing.
- Only the basename of a selected filename is retained.
- A SHA-256 content hash prevents importing the same file twice into one account.
- Broker transaction identifiers prevent the same event being duplicated by overlapping exports.
- Parsing and validation occur before the ledger transaction commits.
- Coverage dates are stored per import batch so missing historical periods can be surfaced.
- Values are stored as exact signed coefficients and decimal scales; display formatting never changes ledger precision.

## Current normalization scope

Trading 212 activity rows are normalized into buys, sells, dividends, deposits, withdrawals, interest, fees, corporate actions, and other events. IBKR trades, cash transactions, corporate actions, and transfers are normalized. IBKR open-position sections are persisted as immutable broker snapshots for position comparison. Imported events drive holdings, average cost, income, valuation, allocation, and portfolio snapshots; incomplete history or market data remains explicit rather than estimated.
