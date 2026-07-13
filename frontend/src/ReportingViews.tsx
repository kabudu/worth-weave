import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { capturePortfolioSnapshot, getPortfolioPerformance, type Account, type ActivityEvent, type AllocationReport, type CurrencyOption, type Holding, type IncomeSummary, type PerformanceHistory, type PortfolioSnapshot, type ReconciliationItem, type TotalReturnAttribution, type ValuationSummary } from "./api";
import { MarketDataDialog } from "./MarketDataDialog";

function money(value: string | null, currency: string | null, fractionDigits = 2) {
  if (!value || !currency) return "—";
  return new Intl.NumberFormat(undefined, { style: "currency", currency, minimumFractionDigits: fractionDigits, maximumFractionDigits: fractionDigits }).format(Number(value));
}

function brokerName(broker: "trading_212" | "ibkr" | "robinhood") {
  return broker === "trading_212" ? "Trading 212" : broker === "ibkr" ? "IBKR" : "Robinhood";
}

function EmptyState({ title, copy }: { title: string; copy: string }) {
  return <div className="report-empty"><span>◇</span><h2>{title}</h2><p>{copy}</p></div>;
}

type PortfolioProps = {
  accounts: Account[];
  holdings: Holding[];
  valuation?: ValuationSummary;
  attribution?: TotalReturnAttribution;
  allocation?: AllocationReport;
  snapshots: PortfolioSnapshot[];
  currencies: CurrencyOption[];
  reportingCurrency: string;
  reconciliation: ReconciliationItem[];
};

export function PortfolioView({ accounts, holdings, valuation, attribution, allocation, snapshots, currencies, reportingCurrency, reconciliation }: PortfolioProps) {
  const [marketOpen, setMarketOpen] = useState(false);
  const [brokerFilter, setBrokerFilter] = useState<string>("all");
  const [accountFilter, setAccountFilter] = useState<string>("all");
  const [holdingSearch, setHoldingSearch] = useState("");
  const [historyRange, setHistoryRange] = useState("All");
  const queryClient = useQueryClient();
  const snapshotMutation = useMutation({ mutationFn: capturePortfolioSnapshot, onSuccess: () => queryClient.invalidateQueries({ queryKey: ["snapshots"] }) });
  const historyScope = accountFilter !== "all" ? `account:${accountFilter}` : brokerFilter !== "all" ? `broker:${brokerFilter}` : "all";
  const performance = useQuery({ queryKey: ["performance", historyScope], queryFn: ({ signal }) => getPortfolioPerformance(historyScope, signal) });
  const valued = new Map(valuation?.holdings.map((item) => [
    `${item.holding.account_id}-${item.holding.instrument_id}-${item.holding.currency}`,
    item,
  ]));
  const firstSnapshot = snapshots.at(0);
  const latestSnapshot = snapshots.at(-1);
  const historyChange = firstSnapshot && latestSnapshot
    ? Number(latestSnapshot.total_value) - Number(firstSnapshot.total_value)
    : null;
  const visibleAccounts = accounts.filter((account) => brokerFilter === "all" || account.broker === brokerFilter);
  const filteredHoldings = holdings.filter((holding) => {
    const haystack = `${holding.symbol ?? ""} ${holding.name ?? ""} ${holding.instrument_id} ${holding.account_name}`.toLowerCase();
    return (brokerFilter === "all" || holding.broker === brokerFilter)
      && (accountFilter === "all" || holding.account_id === accountFilter)
      && haystack.includes(holdingSearch.trim().toLowerCase());
  });
  function chooseBroker(broker: string) { setBrokerFilter(broker); setAccountFilter("all"); }
  return <section className="report-page">
    <header className="report-title-row"><div><span className="section-kicker">Based on your imported history</span><h1>Your investments</h1><p>Current broker positions set quantities when they are available. Imported transactions provide cost and return history.</p></div><div className="valuation-total"><span>{valuation?.valuation_complete ? "Portfolio value" : "Valued so far"} · {reportingCurrency}</span><strong>{money(valuation?.total_value ?? null, reportingCurrency)}</strong>{valuation && !valuation.valuation_complete && <small>{valuation.valued_holding_count} holdings valued · {valuation.missing_price_count} prices · {valuation.missing_fx_count} exchange-rate pairs still needed</small>}<button type="button" onClick={() => setMarketOpen(true)}>Update market data</button></div></header>
    {holdings.length > 0 && <><nav className="history-scopes" aria-label="Portfolio chart scope"><button type="button" className={historyScope === "all" ? "active" : ""} onClick={() => chooseBroker("all")}>Full portfolio</button>{(["trading_212", "ibkr", "robinhood"] as const).filter((broker) => holdings.some((holding) => holding.broker === broker)).map((broker) => <button type="button" key={broker} className={historyScope === `broker:${broker}` ? "active" : ""} onClick={() => chooseBroker(broker)}>{brokerName(broker)}</button>)}{accounts.filter((account) => holdings.some((holding) => holding.account_id === account.id)).map((account) => <button type="button" key={account.id} className={historyScope === `account:${account.id}` ? "active" : ""} onClick={() => { setBrokerFilter(account.broker); setAccountFilter(account.id); }}>{account.display_name}</button>)}</nav><PerformanceChart history={performance.data} range={historyRange} onRange={setHistoryRange} scopeLabel={accountFilter !== "all" ? accounts.find((account) => account.id === accountFilter)?.display_name ?? "Account" : brokerFilter !== "all" ? brokerName(brokerFilter as Account["broker"]) : "Full portfolio"} loading={performance.isPending} /><button type="button" className="history-save" disabled={!valuation?.valuation_complete || snapshotMutation.isPending} onClick={() => snapshotMutation.mutate()}>{snapshotMutation.isPending ? "Saving…" : "Save today’s complete value"}</button>{snapshotMutation.isError && <small className="form-error">{String(snapshotMutation.error)}</small>}</>}
    {attribution && <AttributionPanel report={attribution} />}
    {holdings.length === 0 ? <EmptyState title="No investments to show yet" copy="Import your account history to see what you own." /> : <><section className="portfolio-browser" aria-label="Browse investments"><div className="portfolio-tabs" role="tablist" aria-label="Provider"><button role="tab" aria-selected={brokerFilter === "all"} onClick={() => chooseBroker("all")}>All providers <span>{holdings.length}</span></button>{(["trading_212", "ibkr", "robinhood"] as const).filter((broker) => holdings.some((holding) => holding.broker === broker)).map((broker) => <button key={broker} role="tab" aria-selected={brokerFilter === broker} onClick={() => chooseBroker(broker)}>{brokerName(broker)} <span>{holdings.filter((holding) => holding.broker === broker).length}</span></button>)}</div><div className="portfolio-subtabs"><button className={accountFilter === "all" ? "active" : ""} onClick={() => setAccountFilter("all")}>All accounts</button>{visibleAccounts.filter((account) => holdings.some((holding) => holding.account_id === account.id)).map((account) => <button className={accountFilter === account.id ? "active" : ""} key={account.id} onClick={() => setAccountFilter(account.id)}>{account.display_name}<small>{account.account_type.replaceAll("_", " ")}</small></button>)}</div><label className="portfolio-search"><span>Search holdings</span><input type="search" value={holdingSearch} onChange={(event) => setHoldingSearch(event.target.value)} placeholder="Symbol, company or ISIN" /></label><p>{filteredHoldings.length} of {holdings.length} holdings</p></section>{filteredHoldings.length === 0 ? <EmptyState title="No matching investments" copy="Try another provider, account, or search term." /> : <div className={`report-table-wrap portfolio-content ${accountFilter !== "all" ? "single-account" : ""}`}><table><thead><tr><th>Investment</th>{accountFilter === "all" && <th>Account</th>}<th>Quantity</th><th>Average cost</th><th>Amount invested</th><th>Current value</th><th>Value in {reportingCurrency}</th><th>Gain / loss</th></tr></thead><tbody>{filteredHoldings.map((holding) => {
      const item = valued.get(`${holding.account_id}-${holding.instrument_id}-${holding.currency}`);
      return <tr key={`${holding.account_id}-${holding.instrument_id}-${holding.currency}`}><td><strong>{holding.symbol ?? holding.instrument_id}</strong><small>{holding.name ?? `${brokerName(holding.broker)} · ${holding.instrument_id}`}{item?.price?.stale ? " · stale price" : ""}</small></td>{accountFilter === "all" && <td>{holding.account_name}</td>}<td className="number">{holding.quantity}</td><td className="number">{holding.cost_basis_complete ? money(holding.average_cost, holding.currency, 4) : <span className="basis-warning" title="The broker-reported quantity is known, but an acquisition cost could not be reconstructed from the imported records.">Cost basis unavailable</span>}</td><td className="number">{holding.cost_basis_complete ? money(holding.cost_basis, holding.currency) : "—"}</td><td className="number">{money(item?.market_value ?? null, item?.price?.currency ?? null)}</td><td className="number">{money(item?.reporting_value ?? null, reportingCurrency)}</td><td className="number">{money(item?.gain_loss ?? null, reportingCurrency)}</td></tr>;
    })}</tbody></table></div>}{allocation && <section className="allocation-grid"><AllocationCard title="By broker" slices={allocation.by_platform} currency={allocation.reporting_currency} /><AllocationCard title="By account" slices={allocation.by_account} currency={allocation.reporting_currency} /><AllocationCard title="By investment type" slices={allocation.by_asset_class} currency={allocation.reporting_currency} /><AllocationCard title="By sector" slices={allocation.by_sector} currency={allocation.reporting_currency} /><AllocationCard title="By country or region" slices={allocation.by_geography} currency={allocation.reporting_currency} /><AllocationCard title="By currency" slices={allocation.by_currency} currency={allocation.reporting_currency} /></section>}</>}
    <ReconciliationPanel items={reconciliation} />
    {historyChange !== null && <p className="history-change">Change since {firstSnapshot?.captured_at.slice(0, 10)}: <strong>{money(String(historyChange), reportingCurrency)}</strong></p>}
    <MarketDataDialog open={marketOpen} onClose={() => setMarketOpen(false)} holdings={holdings} currencies={currencies} reportingCurrency={reportingCurrency} />
  </section>;
}

const historyRanges = ["1D", "5D", "1M", "6M", "YTD", "1Y", "5Y", "All"];

function PerformanceChart({ history, range, onRange, scopeLabel, loading }: { history?: PerformanceHistory; range: string; onRange: (range: string) => void; scopeLabel: string; loading: boolean }) {
  const end = history?.points.at(-1) ? new Date(`${history.points.at(-1)?.date}T12:00:00`) : new Date();
  const cutoff = new Date(end);
  if (range === "1D") cutoff.setDate(cutoff.getDate() - 1);
  else if (range === "5D") cutoff.setDate(cutoff.getDate() - 5);
  else if (range === "1M") cutoff.setMonth(cutoff.getMonth() - 1);
  else if (range === "6M") cutoff.setMonth(cutoff.getMonth() - 6);
  else if (range === "YTD") cutoff.setMonth(0, 1);
  else if (range === "1Y") cutoff.setFullYear(cutoff.getFullYear() - 1);
  else if (range === "5Y") cutoff.setFullYear(cutoff.getFullYear() - 5);
  const points = (history?.points ?? []).filter((point) => range === "All" || new Date(`${point.date}T12:00:00`) >= cutoff);
  const values = points.map((point) => Number(point.value));
  const min = Math.min(...values);
  const max = Math.max(...values);
  const spread = Math.max(max - min, max * .02, 1);
  const coordinates = points.map((point, index) => ({
    ...point,
    x: points.length === 1 ? 50 : index / (points.length - 1) * 100,
    y: 92 - ((Number(point.value) - min) / spread * 76),
  }));
  const line = coordinates.map((point, index) => `${index ? "L" : "M"}${point.x.toFixed(2)},${point.y.toFixed(2)}`).join(" ");
  const area = coordinates.length ? `${line} L100,100 L0,100 Z` : "";
  const first = values.at(0);
  const latest = values.at(-1);
  const change = first !== undefined && latest !== undefined ? latest - first : null;
  const changePercent = history?.coverage !== "partial" && change !== null && first && first > 0 ? change / first * 100 : null;
  const latestPoint = coordinates.at(-1);
  return <section className="history-panel" aria-labelledby="portfolio-history-title"><div className="history-heading"><div><span className="section-kicker">Portfolio growth</span><h2 id="portfolio-history-title">{scopeLabel}</h2><p>{history?.coverage === "partial" ? "Partial history; some prices or currency conversions are unavailable." : history?.coverage === "market_reconstructed" ? "Reconstructed from imported activity and cached historical closing prices." : "Based on broker-reported historical position values."}</p></div><div><strong>{money(latest?.toString() ?? null, history?.reporting_currency ?? null)}</strong>{change !== null && <span className={change >= 0 ? "positive" : "negative"}>{change >= 0 ? "+" : ""}{money(change.toString(), history?.reporting_currency ?? null)}{changePercent !== null ? ` · ${changePercent >= 0 ? "+" : ""}${changePercent.toFixed(2)}%` : ""}</span>}</div></div><div className="history-ranges" role="group" aria-label="Chart period">{historyRanges.map((item) => <button type="button" className={range === item ? "active" : ""} aria-pressed={range === item} key={item} onClick={() => onRange(item)}>{item}</button>)}</div><div className="area-chart">{loading ? <p>Loading portfolio history…</p> : coordinates.length < 2 ? <p>Re-import broker files to backfill historical valuations. Worthweave will keep adding complete values automatically.</p> : <><svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label={`${scopeLabel} value history`}><defs><linearGradient id="portfolio-area" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#3cad78" stopOpacity=".42"/><stop offset="1" stopColor="#3cad78" stopOpacity=".03"/></linearGradient></defs><path d={area} fill="url(#portfolio-area)"/><path d={line} fill="none" stroke="#21845f" strokeWidth="1.7" vectorEffect="non-scaling-stroke"/></svg>{latestPoint && <span className="latest-point-marker" style={{left: `${latestPoint.x}%`, top: `${latestPoint.y}%`}} title={`${latestPoint.date}: ${money(latestPoint.value, history?.reporting_currency ?? null)}`}/>}</>}</div>{coordinates.length >= 2 && <div className="history-axis"><span>{coordinates.at(0)?.date}</span><span>{coordinates.at(-1)?.date}</span></div>}</section>;
}

function AttributionPanel({ report }: { report: TotalReturnAttribution }) {
  const components = [
    ["Realised gains", report.realized_gain_loss, false, "Needs complete trade and transfer history"],
    ["Unrealised gains", report.unrealized_gain_loss, false, "Needs complete cost basis and prices"],
    ["Dividends", report.dividends, false, "Needs transaction-date exchange rates"],
    ["Interest", report.interest, false, "Needs transaction-date exchange rates"],
    ["Fees", report.fees, true, "Needs transaction-date exchange rates"],
    ["Taxes", report.taxes, true, "Needs transaction-date exchange rates"],
    ["Currency movement", report.fx_impact, false, "Historical FX attribution not ready"],
  ] as const;
  const headline = report.total_return ?? report.attributed_subtotal;
  return <section className="attribution-card" aria-labelledby="attribution-title">
    <div className="attribution-heading"><div><span className="section-kicker">Investment return</span><h2 id="attribution-title">What changed your return</h2><p>{report.coverage_start && report.coverage_end ? `${report.coverage_start} to ${report.coverage_end}` : "Import history to get started"} · {report.reporting_currency}</p></div><div className={`attribution-total ${report.status}`}><span>{report.total_return ? "Total return" : "Calculated so far"}</span><strong>{money(headline, report.reporting_currency)}</strong><small>{report.status === "complete" ? "Complete for the imported dates" : report.status === "partial" ? "Some figures are still missing" : "Not enough information yet"}</small></div></div>
    <div className="attribution-components">{components.map(([label, value, deduction, reason]) => <article key={label}><span>{label}</span><strong>{value === null ? "Not calculated" : `${deduction && Number(value) !== 0 ? "−" : ""}${money(deduction ? String(Math.abs(Number(value))) : value, report.reporting_currency)}`}</strong>{value === null && <small>{reason}</small>}</article>)}</div>
    {report.notes.length > 0 && <div className="attribution-notes"><strong>What you need to know</strong><ul>{report.notes.map((note) => <li key={note}>{note}</li>)}</ul></div>}
  </section>;
}

function ReconciliationPanel({ items }: { items: ReconciliationItem[] }) {
  if (items.length === 0) return null;
  const matched = items.filter((item) => item.status === "matched").length;
  const brokerBasis = items.filter((item) => item.status === "broker_basis").length;
  const issues = items.filter((item) => item.status === "unavailable" || item.status === "mismatch");
  return <section className="reconciliation-card" aria-labelledby="reconciliation-title"><div><span className="section-kicker">Cost-history coverage</span><h2 id="reconciliation-title">Current quantities and costs come from your broker</h2><p>Worthweave is using all {items.length} latest broker positions. Imported activity fully explains {matched}; {brokerBasis} use the broker-reported current cost basis.{issues.length > 0 ? ` ${issues.length} still lack enough information for a current cost basis.` : " All current positions have cost-basis coverage."}</p></div>{issues.length > 0 && <div className="reconciliation-list">{issues.map((item) => <div key={`${item.account_id}-${item.instrument_id}`}><span className={`reconciliation-status ${item.status}`}>{item.status === "unavailable" ? "Cost history incomplete" : "History quantity differs"}</span><strong>{item.instrument_id}</strong><small>{item.account_name} · transaction history {item.ledger_quantity} · current broker quantity {item.broker_quantity ?? "not included"}{item.as_of ? ` · ${item.as_of}` : ""} · current holding uses broker quantity</small></div>)}</div>}</section>;
}

function AllocationCard({ title, slices, currency }: { title: string; slices: AllocationReport["by_account"]; currency: string }) {
  return <article><span className="section-kicker">How your portfolio is spread</span><h2>{title}</h2>{slices.map((slice) => <div className="allocation-row" key={slice.label}><div><strong>{slice.label}</strong><small>{money(slice.value, currency)}</small></div><div><span style={{ width: `${Math.min(100, Number(slice.percentage))}%` }} /></div><b>{slice.percentage}%</b></div>)}</article>;
}

export function ActivityView({ events }: { events: ActivityEvent[] }) {
  return <section className="report-page"><header><span className="section-kicker">Account history</span><h1>Activity</h1><p>Buys, sells, dividends, fees and other activity from all your accounts, newest first.</p></header>{events.length === 0 ? <EmptyState title="No activity yet" copy="Your imported account history will appear here." /> : <div className="report-table-wrap"><table><thead><tr><th>Date</th><th>Activity</th><th>Account</th><th>Investment</th><th>Quantity</th><th>Amount</th></tr></thead><tbody>{events.map((event) => <tr key={event.id}><td>{event.occurred_at.slice(0, 10)}</td><td><span className={`event-pill ${event.event_type}`}>{event.event_type.replaceAll("_", " ")}</span><small>{event.description}</small></td><td>{event.account_name}</td><td><strong>{event.symbol ?? event.instrument_name ?? event.instrument_id ?? "—"}</strong>{event.symbol && <small>{event.instrument_name ?? event.instrument_id}</small>}</td><td className="number">{event.quantity ?? "—"}</td><td className="number">{money(event.amount, event.currency)}</td></tr>)}</tbody></table></div>}</section>;
}

export function IncomeView({ income }: { income: IncomeSummary[] }) {
  return <section className="report-page"><header><span className="section-kicker">Investment income</span><h1>Income</h1><p>Shown in each payment’s original currency for an accurate audit trail. Current ECB rates value today’s portfolio; converting historical income requires the rate from each payment date.</p></header>{income.length === 0 ? <EmptyState title="No income recorded" copy="Dividend and interest events from your imports will appear here." /> : <><div className="income-fx-note"><strong>Why currencies are separate</strong><span>Worthweave does not apply today’s exchange rate to past income. Transaction-date conversion will be shown when historical rates are available.</span></div><div className="income-grid">{income.map((item) => <article key={item.currency}><span>{item.currency} · original currency</span><strong>{money(item.total, item.currency)}</strong><dl><div><dt>Dividends</dt><dd>{money(item.dividends, item.currency)}</dd></div><div><dt>Interest</dt><dd>{money(item.interest, item.currency)}</dd></div></dl></article>)}</div></>}</section>;
}
