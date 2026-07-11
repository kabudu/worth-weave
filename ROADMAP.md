# Worthweave product roadmap

This roadmap captures promising reporting and analysis features requested by users of Trading 212, Interactive Brokers, and Robinhood. It is a product backlog, not a delivery commitment. Features should be tackled only after their required data is available and the underlying deterministic calculations can be independently tested.

Worthweave's [v1 completion contract](docs/roadmap.md) tracks capabilities already delivered. True total-return attribution (F1) is complete and therefore is not repeated below.

## Product principles

- Financial calculations belong in deterministic Rust code. The local LLM may explain verified results but must not invent figures.
- Missing history, prices, classifications, or exchange rates produce an explicit partial or unavailable result—never an estimate disguised as fact.
- Imported broker records remain immutable. Corrections and enrichment are stored separately with provenance.
- Reporting must remain account-, jurisdiction-, currency-, and tax-wrapper-aware.
- Advice-like features begin as read-only analysis. Worthweave must not place trades without a separate security and regulatory review.
- Every metric should expose its period, inputs, methodology, and completeness status.

## Recommended delivery order

| Sequence | Feature | Priority | Indicative effort | Principal dependency |
| --- | --- | --- | --- | --- |
| 1 | F2 — Dividend intelligence | High | Medium | Reliable income events and security metadata |
| 2 | F3 — Transparent benchmark comparison | High | Medium | Historical valuations, cash flows, benchmark data |
| 3 | F7 — Currency attribution | High | Medium | Transaction-date FX history |
| 4 | F11 — Portfolio change digest | High | Medium | F1, F2, and F7 deterministic outputs |
| 5 | F5 — Allocation drift and contribution planner | High | Medium | Target allocations and complete valuations |
| 6 | F8 — Concentration and correlation risk | Medium–high | Medium | Historical prices and classifications |
| 7 | F6 — UK tax workspace | High | High | Fully reconciled transaction and corporate-action history |
| 8 | F4 — Portfolio X-ray | High | High | Dependable ETF constituent data |
| 9 | F9 — Drawdown and risk dashboard | Medium–high | Medium | Dense historical price and valuation coverage |
| 10 | F10 — Tax-lot explorer | Medium–high | High | Jurisdiction-specific lot engines |
| 11 | F12 — Scenario laboratory | Medium | High | Stable portfolio, risk, and attribution engines |

The order deliberately builds shared calculation foundations before advanced presentation or AI explanation.

---

## F2 — Dividend intelligence

### Goal

Give income-focused investors a clear historical and forward-looking view of portfolio income across every broker and account.

### Proposed capabilities

- Monthly, quarterly, annual, and tax-year dividend totals.
- Income grouped by instrument, account, broker, sector, geography, and currency.
- Gross dividend, withholding tax, fees, and net amount.
- Ex-dividend, record, declaration, and payment dates.
- Upcoming dividend calendar with confirmed versus estimated status.
- Forward annual income based on declared distributions and current holdings.
- Current yield, yield on cost, dividend growth, and income growth.
- Reinvested versus retained-as-cash income.
- Coverage and confidence indicators for every forecast.

### Data and calculation requirements

- Normalize gross, tax, net, and reinvestment events without double counting.
- Obtain corporate dividend schedules from a licensed or otherwise dependable source.
- Distinguish ordinary, special, return-of-capital, and substitute payments.
- Adjust entitlement for trade dates, settlement rules, and position changes.
- Keep forecasts deterministic and label assumptions explicitly.

### Safeguards

- Never present an inferred payment as declared income.
- Do not treat historical dividend growth as a reliable prediction.
- Surface missing withholding-tax data and unsupported instruments.

### Community signal

Dividend history, forecasts, calendars, and portfolio-level income views were among the most repeated requests in the [Trading 212 community](https://community.trading212.com/t/2026-most-wanted-features-poll/89542) and [Robinhood community](https://www.reddit.com/r/RobinhoodApp/comments/1q2oa30/robinhood_rh_needs_a_way_to_track_income_dividends/).

---

## F3 — Transparent benchmark comparison

### Goal

Let users compare portfolio performance with relevant indices without obscuring cash-flow timing or return methodology.

### Proposed capabilities

- Selectable benchmarks such as the FTSE All-Share, S&P 500, and MSCI World.
- Custom benchmark selection where supported by market data.
- Time-weighted return (TWR) for like-for-like investment performance.
- Money-weighted return/XIRR for the investor's experienced return.
- Benchmark simulation using the same deposit and withdrawal schedule.
- Total-return benchmarks that include distributions.
- YTD, tax year, calendar year, trailing periods, and custom ranges.
- Portfolio-versus-benchmark return, volatility, and drawdown comparison.
- Plain-language methodology and formula disclosure.

### Data and calculation requirements

- Daily portfolio valuations or sufficiently granular cash-flow segmentation.
- Historical benchmark prices, distributions, and currency conversion.
- Correct handling of transfers, fees, deposits, withdrawals, and partial periods.
- Reproducible tests against known TWR and XIRR examples.

### Safeguards

- Do not compare MWR with an unadjusted benchmark return.
- Prevent a benchmark period from silently differing from the portfolio period.
- Clearly distinguish price-return from total-return indices.

### Community signal

Users report confusion about benchmark methodology and inconsistent periods in [IBKR PortfolioAnalyst](https://www.reddit.com/r/interactivebrokers/comments/188j9iv), while Robinhood users request an IRR/TWR toggle and Trading 212 users request standard YTD comparisons.

---

## F4 — Portfolio X-ray

### Goal

Reveal the user's true economic exposure by looking through ETFs and combining their constituents with directly held securities.

### Proposed capabilities

- Aggregate direct and indirect company exposure.
- ETF overlap analysis.
- Look-through allocation by sector, country, region, currency, and asset class.
- Top underlying exposures and concentration thresholds.
- Exposure provenance showing which funds contribute to each underlying holding.
- Data date and constituent-coverage indicators.

### Data and calculation requirements

- Dependable, licensed ETF constituent and weight data.
- Identifier resolution across ISIN, FIGI, exchange ticker, and broker contract IDs.
- Fund-of-fund recursion with cycle and depth protection.
- Correct treatment of cash, derivatives, synthetic replication, and unavailable constituents.

### Safeguards

- Never imply full transparency when a fund reports only partial or stale holdings.
- Show both reported and look-through allocation.
- Avoid double counting nested funds.

### Community signal

IBKR users explicitly seek a cross-broker version of the platform's concentration or “X-ray” report to expose ETF overlap across their complete portfolio: [community discussion](https://www.reddit.com/r/interactivebrokers/comments/1aiqrft).

---

## F5 — Allocation drift and contribution planner

### Goal

Help users move towards target allocations, preferably by directing new contributions rather than triggering unnecessary sales.

### Proposed capabilities

- Portfolio-wide and account-specific target allocations.
- Actual versus target values and percentage-point drift.
- User-configurable drift thresholds.
- “What should I buy next?” contribution-only planner.
- Proposed post-contribution allocation preview.
- Sell-inclusive rebalance simulation with estimated realised gains and costs.
- Tax-wrapper-aware placement across ISA and taxable accounts.
- Cash reserve targets and minimum trade constraints.

### Data and calculation requirements

- Complete current valuations and classification metadata.
- Explicit optimization objective and deterministic solver.
- Fractional-share, minimum-order, and currency constraints.
- Estimated fee and tax consequences from the relevant engines.

### Safeguards

- Begin as read-only analysis; do not place orders.
- Label tax estimates and unavailable cost basis.
- Explain why each proposed contribution changes allocation.

### Community signal

Robinhood users request target allocations, automatic rebalancing, and M1-style baskets. Contribution-only rebalancing is particularly attractive because it can reduce taxable disposals.

---

## F6 — UK tax workspace

### Goal

Produce an auditable UK-oriented workspace that assists users and their accountants with investment tax preparation.

### Proposed capabilities

- UK tax-year reporting rather than calendar-year-only reporting.
- Section 104 pooled allowable cost.
- Same-day and 30-day bed-and-breakfast matching.
- Realised capital gains and losses by disposal.
- Remaining annual exempt amount based on user-configured tax-year rules.
- Dividend, interest, foreign income, and withholding-tax summaries.
- Transaction-date GBP conversion with source provenance.
- ISA exclusion and account-wrapper checks.
- Excess Reportable Income reminders for relevant offshore accumulating funds.
- Accountant-friendly CSV/PDF exports and calculation audit trail.

### Data and calculation requirements

- Complete cross-broker UK taxable history, including transfers and corporate actions.
- Versioned tax rules by tax year.
- Transaction-date FX rates from an accepted source.
- Correct handling of splits, mergers, spin-offs, reorganizations, equalisation, and return of capital.
- Explicit user confirmation of tax residence and relevant elections.

### Safeguards

- Present the feature as tax calculation assistance, not tax advice.
- Never calculate a disposal from incomplete pooled history.
- Version and display every rule and rate used.
- Keep ISA and taxable activity structurally separated.

### Community signal

Trading 212 and IBKR users repeatedly build spreadsheets or third-party tools to obtain HMRC-ready calculations, including transaction-date FX, Section 104 pooling, dividends, and interest: [Trading 212 discussion](https://community.trading212.com/t/annual-statement-for-tax/90163/10), [IBKR discussion](https://www.reddit.com/r/interactivebrokers/comments/1pz6e9z/filing-uk-tax-returns/).

---

## F7 — Currency attribution

### Goal

Explain how much of a return came from the investment itself and how much came from exchange-rate movement.

### Proposed capabilities

- Native-currency return.
- Reporting-currency return.
- Constant-currency return.
- Explicit FX contribution.
- FX fees and conversion costs.
- Currency exposure across securities, funds, and cash.
- Historical exchange-rate chart aligned with transactions and valuations.
- Hedged versus unhedged exposure where instrument data supports it.

### Data and calculation requirements

- Transaction-date and valuation-date FX rates with provenance.
- Historical prices in the instrument's quote currency.
- Correct separation of security return, income, fees, and currency movement.
- Treatment of cross-currency cash flows and broker conversions.

### Safeguards

- Do not use today's FX rate to claim historical FX attribution.
- Surface triangulated rates and their source currencies.
- Distinguish listing currency from underlying economic currency exposure.

### Community signal

Trading 212 users explicitly request constant-currency returns and an FX contribution split: [community request](https://community.trading212.com/t/returns-in-constant-currency-vs-benchmark/63322).

---

## F8 — Concentration and correlation risk

### Goal

Identify hidden concentrations and holdings that behave similarly, without reducing a complex portfolio to a simplistic score.

### Proposed capabilities

- Largest position, top-three, top-five, and top-ten concentration.
- Sector, geography, asset-class, broker, account, and currency concentration.
- Correlation matrix and correlated holding clusters.
- Indirect concentration using Portfolio X-ray data.
- Marginal risk contribution by holding.
- Configurable portfolio-risk alerts rather than price alerts.
- Historical concentration trend.

### Data and calculation requirements

- Sufficiently long, aligned price histories.
- Return-frequency and minimum-observation rules.
- Classification and ETF look-through data.
- Stable statistical calculations with explicit windows.

### Safeguards

- Describe detected exposure rather than declaring a portfolio “safe” or “unsafe.”
- Show the observation window and missing-data exclusions.
- Explain that correlation changes and is not a prediction.

### Community signal

IBKR users ask for portfolio correlation tables, while Robinhood users value sector-diversity reporting and noticed when it disappeared: [IBKR discussion](https://www.reddit.com/r/interactivebrokers/comments/1elyeb9), [Robinhood discussion](https://www.reddit.com/r/RobinhoodApp/comments/1rpadsb/new_ui/).

---

## F9 — Drawdown and risk dashboard

### Goal

Show how the portfolio has behaved during adverse periods and how efficiently it has historically generated returns for the risk taken.

### Proposed capabilities

- Maximum and current drawdown.
- Peak, trough, recovery date, and time underwater.
- Rolling and annualised volatility.
- Sharpe and Sortino ratios.
- Best and worst day, month, quarter, and year.
- Historical stress-period replay.
- Downside capture and benchmark-relative risk.
- Risk contribution by holding or allocation group.

### Data and calculation requirements

- Dense, reliable valuation history adjusted for external cash flows.
- Risk-free-rate data for relevant currencies and periods.
- Deterministic methodology for irregular observations.
- Benchmark engine from F3.

### Safeguards

- Do not calculate annualised statistics from inadequate history.
- Display frequency, period, benchmark, and risk-free-rate assumptions.
- Explain that historical risk is not a forecast.

### Community signal

IBKR users building their own dashboards emphasize maximum drawdown, cash-flow-adjusted Sharpe ratio, concentration, and equity curves: [community dashboard](https://www.reddit.com/r/interactivebrokers/comments/1trxbop/sharing_built_a_custom_local_dashboard_to_better/).

---

## F10 — Tax-lot explorer

### Goal

Make lot-level cost basis and potential disposal consequences visible without forcing users into a broker's sell workflow.

### Proposed capabilities

- Purchase date, quantity remaining, cost basis, and current gain/loss per lot.
- Holding-period classification appropriate to the selected jurisdiction.
- Disposal-method simulations such as specific identification or FIFO where legally relevant.
- Estimated gain/loss before a proposed disposal.
- Lot lineage through transfers and corporate actions.
- Short-term versus long-term summaries for US accounts.
- Links from aggregate holdings directly to their underlying lots.

### Data and calculation requirements

- Immutable lot lineage and complete acquisition history.
- Jurisdiction- and account-specific disposal rules.
- Corporate-action and transfer basis allocation.
- Broker lot elections and confirmations where available.

### Safeguards

- Do not assume FIFO globally.
- Keep performance lots distinct from statutory tax calculations.
- Withhold estimates when basis or jurisdiction is unknown.

### Community signal

Robinhood users repeatedly request tax lots outside the order-entry flow and better long-term/short-term visibility: [community discussion](https://www.reddit.com/r/RobinhoodApp/comments/1s5f5v5/tax_lots_are_available_on_the_website/).

---

## F11 — Portfolio change digest

### Goal

Explain, in concise language, what changed in the portfolio and which verified components caused the change.

### Proposed capabilities

- Daily, weekly, monthly, quarterly, and custom-period summaries.
- Largest contributors and detractors.
- Price, FX, dividend, interest, fee, tax, and external-cash-flow attribution.
- New, increased, reduced, and closed holdings.
- Allocation and concentration changes.
- Income received and upcoming confirmed payments.
- Data-quality, stale-price, reconciliation, and coverage warnings.
- Optional private local-LLM narrative grounded in the deterministic report.

### Data and calculation requirements

- Period-aware outputs from F1, F2, and F7.
- Historical snapshots and instrument classifications.
- A stable, versioned JSON explanation contract.
- Deterministic ranking of material changes before LLM summarisation.

### Safeguards

- The LLM receives calculated facts, not raw authority to calculate.
- Generated text must preserve material caveats and unavailable data.
- No predictions or personalised trading recommendations.
- Provide a calculation-only view when local AI is disabled.

### Example

> Your portfolio rose £1,240 this month. £780 came from market movement, £310 from currency movement, £94 from dividends, and £56 from net contributions. Technology exposure increased from 21% to 24%.

---

## F12 — Scenario laboratory

### Goal

Allow users to explore deterministic “what if?” scenarios without changing ledger data or presenting uncertain projections as promises.

### Proposed capabilities

- Contribution amount and frequency changes.
- Target allocation and rebalancing proposals.
- Currency and market shocks.
- Historical drawdown replays.
- Fee-drag and tax-drag comparisons.
- Dividend-growth and income scenarios.
- Long-term contribution projections with editable return ranges.
- Side-by-side scenarios with saved assumptions.

### Data and calculation requirements

- Separate immutable real portfolio state from ephemeral scenario state.
- Versioned assumptions and deterministic projection engine.
- Inflation, fee, tax, and contribution timing support.
- Historical scenario data with survivorship and availability caveats.

### Safeguards

- Use ranges and distributions where appropriate, not false point precision.
- Label every assumption and make it editable.
- Never describe a projection as expected or guaranteed performance.
- Keep scenarios local unless the user deliberately exports them.

### Community signal

Robinhood users respond positively to long-term simulations but question hidden assumptions, while IBKR users want manually specified model portfolios and backtesting separate from real holdings: [Robinhood discussion](https://www.reddit.com/r/RobinhoodApp/comments/1s2ndet/robinhood_strategies/), [IBKR discussion](https://www.reddit.com/r/interactivebrokers/comments/opqhos).

---

## Cross-cutting platform work

The roadmap features depend on several shared capabilities:

- Historical market-price and FX-rate storage with source provenance.
- Instrument identity resolution and corporate-action processing.
- Period-aware reporting APIs rather than all-time-only projections.
- Calculation methodology registry and versioned result schemas.
- Data-coverage diagnostics per account, instrument, currency, and period.
- Reproducible golden financial test fixtures.
- Exportable reports with calculation notes and audit trails.
- Performance controls for portfolios approaching the 500,000-row import ceiling.

## Definition of ready

A feature is ready for implementation when:

1. Its required source data is available with acceptable licensing and provenance.
2. The deterministic formula and incomplete-data behaviour are documented.
3. Jurisdiction and account-wrapper implications have been identified.
4. Golden test cases can be written before UI work begins.
5. User-facing terminology and non-advice boundaries are agreed.

## Definition of done

A roadmap feature is done only when:

1. Native calculations pass deterministic unit and integration tests.
2. Partial and unavailable states are tested, not merely the happy path.
3. The frontend validates the native response schema.
4. Keyboard, screen-reader, contrast, and responsive checks pass.
5. Security, privacy, performance, dependency, and packaged-app checks pass.
6. Architecture, user documentation, and this roadmap are updated.

