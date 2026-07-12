# Broker data imports

## Supported sources

- Trading 212 transaction-history CSV exports with schema variations observed from 2020–2026.
- IBKR multi-section Activity Flex Query CSV exports produced by the Worthweave query configuration.

Robinhood is supported in the account model with region-specific legal wrappers: UK individual brokerage and Stocks & Shares ISA; US individual brokerage, JTWROS joint investing, Traditional IRA, Roth IRA, and UTMA custodial accounts. Robinhood US documents downloadable account-activity CSV reports but does not publish a stable column contract, while Robinhood UK currently documents monthly PDF statements. Robinhood imports remain disabled until representative anonymised exports can be validated with fixtures. The native boundary returns an explicit unsupported-format error rather than attempting another broker's parser.

## Current positions and repair imports

- When an IBKR export contains an open-positions section, its latest dated position snapshot is authoritative for current quantities. Transactions remain the source for cost basis and return attribution only when the imported history fully explains that quantity.
- IBKR instrument matching prefers ISIN, then contract ID, then a normalized symbol. Symbol-only trades are linked to the stronger identity from a position row in the same export when available.
- IBKR mark prices and their currencies are imported from the latest position rows as broker-provided market data. They are never treated as live quotes.
- Importing the same file again is idempotent. Worthweave does not duplicate events; it repairs missing instrument links and refreshes broker position and price data.
- The file picker accepts several CSV exports at once. Each file commits atomically; if a later file is invalid, earlier successful files remain and retrying the full selection is safe because repeated files use the repair path.
- Trading 212 transaction exports do not contain a current position snapshot. Worthweave derives their holdings from the complete imported transaction history and does not present the absence of a broker snapshot as a reconciliation failure.
- Corporate actions and security transfers are never inferred from ambiguous descriptions. If their exact effect is unavailable, current quantity comes from the broker snapshot and cost basis or return attribution remains explicitly incomplete.

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
