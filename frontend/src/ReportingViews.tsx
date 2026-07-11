import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { capturePortfolioSnapshot, type ActivityEvent, type AllocationReport, type CurrencyOption, type Holding, type IncomeSummary, type PortfolioSnapshot, type ValuationSummary } from "./api";
import { MarketDataDialog } from "./MarketDataDialog";

function money(value: string | null, currency: string | null) {
  if (!value || !currency) return "—";
  return new Intl.NumberFormat(undefined, { style: "currency", currency, maximumFractionDigits: 4 }).format(Number(value));
}

function brokerName(broker: "trading_212" | "ibkr") {
  return broker === "trading_212" ? "Trading 212" : "IBKR";
}

function EmptyState({ title, copy }: { title: string; copy: string }) {
  return <div className="report-empty"><span>◇</span><h3>{title}</h3><p>{copy}</p></div>;
}

type PortfolioProps = {
  holdings: Holding[];
  valuation?: ValuationSummary;
  allocation?: AllocationReport;
  snapshots: PortfolioSnapshot[];
  currencies: CurrencyOption[];
  reportingCurrency: string;
};

export function PortfolioView({ holdings, valuation, allocation, snapshots, currencies, reportingCurrency }: PortfolioProps) {
  const [marketOpen, setMarketOpen] = useState(false);
  const queryClient = useQueryClient();
  const snapshotMutation = useMutation({ mutationFn: capturePortfolioSnapshot, onSuccess: () => queryClient.invalidateQueries({ queryKey: ["snapshots"] }) });
  const valued = new Map(valuation?.holdings.map((item) => [
    `${item.holding.account_id}-${item.holding.instrument_id}-${item.holding.currency}`,
    item,
  ]));
  return <section className="report-page">
    <header className="report-title-row"><div><span className="section-kicker">Deterministic ledger</span><h1>Your holdings</h1><p>Quantities and average cost are reconstructed from imported broker events.</p></div><div className="valuation-total"><span>Portfolio value · {reportingCurrency}</span><strong>{money(valuation?.total_value ?? null, reportingCurrency)}</strong>{valuation && !valuation.total_value && <small>{valuation.missing_price_count} prices · {valuation.missing_fx_count} FX rates missing</small>}<button type="button" onClick={() => setMarketOpen(true)}>Update prices &amp; FX</button></div></header>
    {holdings.length === 0 ? <EmptyState title="No open holdings yet" copy="Import transaction history to reconstruct your positions." /> : <><div className="report-table-wrap"><table><thead><tr><th>Instrument</th><th>Account</th><th>Quantity</th><th>Average cost</th><th>Cost basis</th><th>Market value</th><th>{reportingCurrency} value</th></tr></thead><tbody>{holdings.map((holding) => {
      const item = valued.get(`${holding.account_id}-${holding.instrument_id}-${holding.currency}`);
      return <tr key={`${holding.account_id}-${holding.instrument_id}-${holding.currency}`}><td><strong>{holding.instrument_id}</strong><small>{brokerName(holding.broker)}</small></td><td>{holding.account_name}</td><td className="number">{holding.quantity}</td><td className="number">{holding.cost_basis_complete ? money(holding.average_cost, holding.currency) : <span className="basis-warning">Incomplete history</span>}</td><td className="number">{holding.cost_basis_complete ? money(holding.cost_basis, holding.currency) : "—"}</td><td className="number">{money(item?.market_value ?? null, item?.price?.currency ?? null)}</td><td className="number">{money(item?.reporting_value ?? null, reportingCurrency)}</td></tr>;
    })}</tbody></table></div><section className="performance-card"><div><span className="section-kicker">Portfolio history</span><h2>Valuation snapshots</h2><p>Capture a point-in-time total after prices and FX are complete.</p></div><div className="snapshot-chart">{snapshots.length === 0 ? <small>No snapshots yet</small> : snapshots.slice(-12).map((snapshot) => <span key={snapshot.id} title={`${snapshot.captured_at}: ${snapshot.total_value} ${snapshot.reporting_currency}`} style={{ height: `${Math.max(12, Number(snapshot.total_value) / Math.max(...snapshots.map((item) => Number(item.total_value))) * 100)}%` }} />)}</div><button type="button" className="secondary-button" disabled={!valuation?.total_value || snapshotMutation.isPending} onClick={() => snapshotMutation.mutate()}>{snapshotMutation.isPending ? "Capturing…" : "Capture snapshot"}</button>{snapshotMutation.isError && <small className="form-error">{String(snapshotMutation.error)}</small>}</section>{allocation && <section className="allocation-grid"><AllocationCard title="By account" slices={allocation.by_account} currency={allocation.reporting_currency} /><AllocationCard title="By source currency" slices={allocation.by_currency} currency={allocation.reporting_currency} /></section>}</>}
    <MarketDataDialog open={marketOpen} onClose={() => setMarketOpen(false)} holdings={holdings} currencies={currencies} reportingCurrency={reportingCurrency} />
  </section>;
}

function AllocationCard({ title, slices, currency }: { title: string; slices: AllocationReport["by_account"]; currency: string }) {
  return <article><span className="section-kicker">Allocation</span><h2>{title}</h2>{slices.map((slice) => <div className="allocation-row" key={slice.label}><div><strong>{slice.label}</strong><small>{money(slice.value, currency)}</small></div><div><span style={{ width: `${Math.min(100, Number(slice.percentage))}%` }} /></div><b>{slice.percentage}%</b></div>)}</article>;
}

export function ActivityView({ events }: { events: ActivityEvent[] }) {
  return <section className="report-page"><header><span className="section-kicker">Canonical history</span><h1>Activity</h1><p>Broker events normalized into one chronological ledger.</p></header>{events.length === 0 ? <EmptyState title="No activity yet" copy="Verified imports will appear here." /> : <div className="report-table-wrap"><table><thead><tr><th>Date</th><th>Event</th><th>Account</th><th>Instrument</th><th>Quantity</th><th>Amount</th></tr></thead><tbody>{events.map((event) => <tr key={event.id}><td>{event.occurred_at.slice(0, 10)}</td><td><span className={`event-pill ${event.event_type}`}>{event.event_type.replaceAll("_", " ")}</span><small>{event.description}</small></td><td>{event.account_name}</td><td>{event.instrument_id ?? "—"}</td><td className="number">{event.quantity ?? "—"}</td><td className="number">{money(event.amount, event.currency)}</td></tr>)}</tbody></table></div>}</section>;
}

export function IncomeView({ income }: { income: IncomeSummary[] }) {
  return <section className="report-page"><header><span className="section-kicker">Cash distributions</span><h1>Income</h1><p>Dividend and interest totals remain in source currency until verified FX rates are available.</p></header>{income.length === 0 ? <EmptyState title="No income recorded" copy="Dividend and interest events from your imports will appear here." /> : <div className="income-grid">{income.map((item) => <article key={item.currency}><span>{item.currency}</span><strong>{money(item.total, item.currency)}</strong><dl><div><dt>Dividends</dt><dd>{money(item.dividends, item.currency)}</dd></div><div><dt>Interest</dt><dd>{money(item.interest, item.currency)}</dd></div></dl></article>)}</div>}</section>;
}
