import type { ActivityEvent, Holding, IncomeSummary } from "./api";

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

export function PortfolioView({ holdings }: { holdings: Holding[] }) {
  return <section className="report-page"><header><span className="section-kicker">Deterministic ledger</span><h1>Your holdings</h1><p>Quantities and average cost are reconstructed from imported broker events.</p></header>{holdings.length === 0 ? <EmptyState title="No open holdings yet" copy="Import transaction history to reconstruct your positions." /> : <div className="report-table-wrap"><table><thead><tr><th>Instrument</th><th>Account</th><th>Quantity</th><th>Average cost</th><th>Cost basis</th></tr></thead><tbody>{holdings.map((holding) => <tr key={`${holding.account_id}-${holding.instrument_id}-${holding.currency}`}><td><strong>{holding.instrument_id}</strong><small>{brokerName(holding.broker)}</small></td><td>{holding.account_name}</td><td className="number">{holding.quantity}</td><td className="number">{holding.cost_basis_complete ? money(holding.average_cost, holding.currency) : <span className="basis-warning">Incomplete history</span>}</td><td className="number">{holding.cost_basis_complete ? money(holding.cost_basis, holding.currency) : "—"}</td></tr>)}</tbody></table></div>}</section>;
}

export function ActivityView({ events }: { events: ActivityEvent[] }) {
  return <section className="report-page"><header><span className="section-kicker">Canonical history</span><h1>Activity</h1><p>Broker events normalized into one chronological ledger.</p></header>{events.length === 0 ? <EmptyState title="No activity yet" copy="Verified imports will appear here." /> : <div className="report-table-wrap"><table><thead><tr><th>Date</th><th>Event</th><th>Account</th><th>Instrument</th><th>Quantity</th><th>Amount</th></tr></thead><tbody>{events.map((event) => <tr key={event.id}><td>{event.occurred_at.slice(0, 10)}</td><td><span className={`event-pill ${event.event_type}`}>{event.event_type.replaceAll("_", " ")}</span><small>{event.description}</small></td><td>{event.account_name}</td><td>{event.instrument_id ?? "—"}</td><td className="number">{event.quantity ?? "—"}</td><td className="number">{money(event.amount, event.currency)}</td></tr>)}</tbody></table></div>}</section>;
}

export function IncomeView({ income }: { income: IncomeSummary[] }) {
  return <section className="report-page"><header><span className="section-kicker">Cash distributions</span><h1>Income</h1><p>Dividend and interest totals remain in source currency until verified FX rates are available.</p></header>{income.length === 0 ? <EmptyState title="No income recorded" copy="Dividend and interest events from your imports will appear here." /> : <div className="income-grid">{income.map((item) => <article key={item.currency}><span>{item.currency}</span><strong>{money(item.total, item.currency)}</strong><dl><div><dt>Dividends</dt><dd>{money(item.dividends, item.currency)}</dd></div><div><dt>Interest</dt><dd>{money(item.interest, item.currency)}</dd></div></dl></article>)}</div>}</section>;
}
