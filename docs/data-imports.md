# Broker data imports

## Supported sources

- Trading 212 transaction-history CSV exports with schema variations observed from 2020–2026.
- IBKR multi-section Activity Flex Query CSV exports produced by the Worthweave query configuration.

Robinhood is supported in the account model with region-specific legal wrappers: UK individual brokerage and Stocks & Shares ISA; US individual brokerage, JTWROS joint investing, Traditional IRA, Roth IRA, and UTMA custodial accounts. Robinhood US documents downloadable account-activity CSV reports but does not publish a stable column contract, while Robinhood UK currently documents monthly PDF statements. Robinhood imports remain disabled until representative anonymised exports can be validated with fixtures. The native boundary returns an explicit unsupported-format error rather than attempting another broker's parser.

## Current positions and repair imports

### Trading 212 API synchronisation

- Invest and Stocks and Shares ISA accounts may be connected independently through Trading 212's official Public API. Each account requires its own read-only API key pair.
- Worthweave requests an official Trading 212 history report containing orders, dividends, interest and cash transactions. The downloaded CSV is passed through the same bounded parser, immutable event store and duplicate checks as a manually selected export.
- The current positions endpoint supplies an authoritative daily position snapshot, cost basis, account-currency value and broker price. It does not replace imported transaction history for realised-return calculations.
- Synchronisation runs on launch when the last successful sync is at least 24 hours old. Report generation is asynchronous; Worthweave records the report identifier locally and checks it again after the provider's rate-limit interval.
- Network, authentication and rate-limit failures leave the last successful local dataset unchanged. CSV remains available for offline use, historical repair and schema troubleshooting.
- Disconnecting deletes the credentials from macOS Keychain and local connection metadata. It does not delete previously imported portfolio records.

### IBKR Flex Web Service synchronisation

IBKR Invest and ISA accounts may be connected independently in **Settings → Broker connections**. In IBKR Client Portal, enable Flex Web Service and create an **Activity Flex Query** with CSV output. Each query must contain exactly one IBKR account. Worthweave validates that account before committing a report and remembers its `ClientAccountID` for subsequent syncs.

For useful holdings, cost basis, income and return reporting, include:

- Account Information, with `ClientAccountID`, primary currency, account type and opening date.
- Trades, with stable trade identifiers, date/time, buy/sell, quantity, proceeds or net cash, currency, symbol, ISIN, contract ID, description and asset class.
- Cash Transactions, with stable transaction identifiers, date/time, amount, currency, type, description and instrument identifiers.
- Corporate Actions and Transfers, including their identifiers, dates, quantities, amounts, currencies and instrument identifiers.
- Open Positions at summary level, with report date, quantity, mark price, position value, cost basis, currency, symbol, description, ISIN, contract ID and asset class.

Worthweave stores the Flex token and numeric query ID in macOS Keychain, not SQLite. IBKR report generation is asynchronous, so a connection or sync may initially show **Preparing**. Worthweave preserves the report reference and checks it again without duplicating imported activity. Expired tokens, invalid query IDs, IP restrictions and rate limits are surfaced as actionable connection states. Manual CSV imports remain available for offline use and historical repair.

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
