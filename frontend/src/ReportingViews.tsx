import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { capturePortfolioSnapshot, type Account, type ActivityEvent, type AllocationReport, type CurrencyOption, type Holding, type IncomeSummary, type PortfolioSnapshot, type ReconciliationItem, type TotalReturnAttribution, type ValuationSummary } from "./api";
import { MarketDataDialog } from "./MarketDataDialog";

function money(value: string | null, currency: string | null) {
  if (!value || !currency) return "—";
  return new Intl.NumberFormat(undefined, { style: "currency", currency, maximumFractionDigits: 4 }).format(Number(value));
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
  const queryClient = useQueryClient();
  const snapshotMutation = useMutation({ mutationFn: capturePortfolioSnapshot, onSuccess: () => queryClient.invalidateQueries({ queryKey: ["snapshots"] }) });
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
    {attribution && <AttributionPanel report={attribution} />}
    {holdings.length === 0 ? <EmptyState title="No investments to show yet" copy="Import your account history to see what you own." /> : <><section className="portfolio-browser" aria-label="Browse investments"><div className="portfolio-tabs" role="tablist" aria-label="Provider"><button role="tab" aria-selected={brokerFilter === "all"} onClick={() => chooseBroker("all")}>All providers <span>{holdings.length}</span></button>{(["trading_212", "ibkr", "robinhood"] as const).filter((broker) => holdings.some((holding) => holding.broker === broker)).map((broker) => <button key={broker} role="tab" aria-selected={brokerFilter === broker} onClick={() => chooseBroker(broker)}>{brokerName(broker)} <span>{holdings.filter((holding) => holding.broker === broker).length}</span></button>)}</div><div className="portfolio-subtabs"><button className={accountFilter === "all" ? "active" : ""} onClick={() => setAccountFilter("all")}>All accounts</button>{visibleAccounts.filter((account) => holdings.some((holding) => holding.account_id === account.id)).map((account) => <button className={accountFilter === account.id ? "active" : ""} key={account.id} onClick={() => setAccountFilter(account.id)}>{account.display_name}<small>{account.account_type.replaceAll("_", " ")}</small></button>)}</div><label className="portfolio-search"><span>Search holdings</span><input type="search" value={holdingSearch} onChange={(event) => setHoldingSearch(event.target.value)} placeholder="Symbol, company or ISIN" /></label><p>{filteredHoldings.length} of {holdings.length} holdings</p></section>{filteredHoldings.length === 0 ? <EmptyState title="No matching investments" copy="Try another provider, account, or search term." /> : <div className="report-table-wrap"><table><thead><tr><th>Investment</th><th>Account</th><th>Quantity</th><th>Average cost</th><th>Amount invested</th><th>Current value</th><th>Value in {reportingCurrency}</th><th>Gain / loss</th></tr></thead><tbody>{filteredHoldings.map((holding) => {
      const item = valued.get(`${holding.account_id}-${holding.instrument_id}-${holding.currency}`);
      return <tr key={`${holding.account_id}-${holding.instrument_id}-${holding.currency}`}><td><strong>{holding.symbol ?? holding.instrument_id}</strong><small>{holding.name ?? `${brokerName(holding.broker)} · ${holding.instrument_id}`}{item?.price?.stale ? " · stale price" : ""}</small></td><td>{holding.account_name}</td><td className="number">{holding.quantity}</td><td className="number">{holding.cost_basis_complete ? money(holding.average_cost, holding.currency) : <span className="basis-warning">Incomplete history</span>}</td><td className="number">{holding.cost_basis_complete ? money(holding.cost_basis, holding.currency) : "—"}</td><td className="number">{money(item?.market_value ?? null, item?.price?.currency ?? null)}</td><td className="number">{money(item?.reporting_value ?? null, reportingCurrency)}</td><td className="number">{money(item?.gain_loss ?? null, reportingCurrency)}</td></tr>;
    })}</tbody></table></div>}<section className="performance-card"><div><span className="section-kicker">Portfolio history</span><h2>Saved portfolio values</h2><p>{valuation?.valuation_complete ? "Save today’s total to track how your portfolio changes over time." : "Complete all missing prices before saving a portfolio value."}</p></div><div className="snapshot-chart">{snapshots.length === 0 ? <small>No saved values yet</small> : snapshots.slice(-12).map((snapshot) => <span key={snapshot.id} title={`${snapshot.captured_at}: ${snapshot.total_value} ${snapshot.reporting_currency}`} style={{ height: `${Math.max(12, Number(snapshot.total_value) / Math.max(...snapshots.map((item) => Number(item.total_value))) * 100)}%` }} />)}</div><button type="button" className="secondary-button" disabled={!valuation?.valuation_complete || snapshotMutation.isPending} onClick={() => snapshotMutation.mutate()}>{snapshotMutation.isPending ? "Saving…" : "Save today’s value"}</button>{snapshotMutation.isError && <small className="form-error">{String(snapshotMutation.error)}</small>}</section>{allocation && <section className="allocation-grid"><AllocationCard title="By broker" slices={allocation.by_platform} currency={allocation.reporting_currency} /><AllocationCard title="By account" slices={allocation.by_account} currency={allocation.reporting_currency} /><AllocationCard title="By investment type" slices={allocation.by_asset_class} currency={allocation.reporting_currency} /><AllocationCard title="By sector" slices={allocation.by_sector} currency={allocation.reporting_currency} /><AllocationCard title="By country or region" slices={allocation.by_geography} currency={allocation.reporting_currency} /><AllocationCard title="By currency" slices={allocation.by_currency} currency={allocation.reporting_currency} /></section>}</>}
    <ReconciliationPanel items={reconciliation} />
    {historyChange !== null && <p className="history-change">Change since {firstSnapshot?.captured_at.slice(0, 10)}: <strong>{money(String(historyChange), reportingCurrency)}</strong></p>}
    <MarketDataDialog open={marketOpen} onClose={() => setMarketOpen(false)} holdings={holdings} currencies={currencies} reportingCurrency={reportingCurrency} />
  </section>;
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
  const incomplete = items.filter((item) => item.status === "unavailable").length;
  return <section className="reconciliation-card" aria-labelledby="reconciliation-title"><div><span className="section-kicker">Cost-history coverage</span><h2 id="reconciliation-title">Current quantities come from your broker</h2><p>Worthweave is using all {items.length} latest broker positions. Imported trades fully explain {matched}.{incomplete > 0 ? ` ${incomplete} need earlier history before cost and return figures can be complete.` : ""}</p></div><div className="reconciliation-list">{items.filter((item) => item.status !== "matched").map((item) => <div key={`${item.account_id}-${item.instrument_id}`}><span className={`reconciliation-status ${item.status}`}>{item.status === "unavailable" ? "Cost history incomplete" : "History quantity differs"}</span><strong>{item.instrument_id}</strong><small>{item.account_name} · transaction history {item.ledger_quantity} · current broker quantity {item.broker_quantity ?? "not included"}{item.as_of ? ` · ${item.as_of}` : ""} · current holding uses broker quantity</small></div>)}</div></section>;
}

function AllocationCard({ title, slices, currency }: { title: string; slices: AllocationReport["by_account"]; currency: string }) {
  return <article><span className="section-kicker">How your portfolio is spread</span><h2>{title}</h2>{slices.map((slice) => <div className="allocation-row" key={slice.label}><div><strong>{slice.label}</strong><small>{money(slice.value, currency)}</small></div><div><span style={{ width: `${Math.min(100, Number(slice.percentage))}%` }} /></div><b>{slice.percentage}%</b></div>)}</article>;
}

export function ActivityView({ events }: { events: ActivityEvent[] }) {
  return <section className="report-page"><header><span className="section-kicker">Account history</span><h1>Activity</h1><p>Buys, sells, dividends, fees and other activity from all your accounts, newest first.</p></header>{events.length === 0 ? <EmptyState title="No activity yet" copy="Your imported account history will appear here." /> : <div className="report-table-wrap"><table><thead><tr><th>Date</th><th>Activity</th><th>Account</th><th>Investment</th><th>Quantity</th><th>Amount</th></tr></thead><tbody>{events.map((event) => <tr key={event.id}><td>{event.occurred_at.slice(0, 10)}</td><td><span className={`event-pill ${event.event_type}`}>{event.event_type.replaceAll("_", " ")}</span><small>{event.description}</small></td><td>{event.account_name}</td><td>{event.instrument_id ?? "—"}</td><td className="number">{event.quantity ?? "—"}</td><td className="number">{money(event.amount, event.currency)}</td></tr>)}</tbody></table></div>}</section>;
}

export function IncomeView({ income }: { income: IncomeSummary[] }) {
  return <section className="report-page"><header><span className="section-kicker">Investment income</span><h1>Income</h1><p>Dividend and interest totals remain in their original currencies until exchange rates are available.</p></header>{income.length === 0 ? <EmptyState title="No income recorded" copy="Dividend and interest events from your imports will appear here." /> : <div className="income-grid">{income.map((item) => <article key={item.currency}><span>{item.currency}</span><strong>{money(item.total, item.currency)}</strong><dl><div><dt>Dividends</dt><dd>{money(item.dividends, item.currency)}</dd></div><div><dt>Interest</dt><dd>{money(item.interest, item.currency)}</dd></div></dl></article>)}</div>}</section>;
}
