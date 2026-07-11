import { useQuery } from "@tanstack/react-query";
import type { CSSProperties } from "react";
import { useState } from "react";

import { getActivity, getCurrencies, getHoldings, getIncomeSummary, getPortfolioAllocation, getPortfolioSnapshots, getPortfolioSummary, getPortfolioValuation, getSettings } from "./api";
import { Onboarding, SettingsDialog } from "./CurrencySetup";
import { ImportDialog } from "./ImportDialog";
import { ActivityView, IncomeView, PortfolioView } from "./ReportingViews";

const navItems = [
  ["Overview", "⌁"],
  ["Portfolio", "◈"],
  ["Activity", "↗"],
  ["Income", "◇"],
  ["Insights", "✦"],
] as const;

function BrandMark() {
  return (
    <div className="brand-mark" aria-hidden="true">
      <span />
      <span />
      <span />
    </div>
  );
}

function StatusOrb({ imports }: { imports: number }) {
  const progress = imports === 0 ? 8 : Math.min(88, 18 + imports * 12);
  return (
    <div className="status-orb" style={{ "--progress": `${progress * 3.6}deg` } as CSSProperties}>
      <div>
        <strong>{progress}%</strong>
        <span>data ready</span>
      </div>
    </div>
  );
}

export function App() {
  const [activeView, setActiveView] = useState("Overview");
  const [importOpen, setImportOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: ({ signal }) => getSettings(signal),
  });
  const currencies = useQuery({
    queryKey: ["currencies"],
    queryFn: ({ signal }) => getCurrencies(signal),
    staleTime: Infinity,
  });
  const summary = useQuery({
    queryKey: ["portfolio-summary"],
    queryFn: ({ signal }) => getPortfolioSummary(signal),
  });
  const holdings = useQuery({ queryKey: ["holdings"], queryFn: ({ signal }) => getHoldings(signal) });
  const activity = useQuery({ queryKey: ["activity"], queryFn: ({ signal }) => getActivity(signal) });
  const income = useQuery({ queryKey: ["income"], queryFn: ({ signal }) => getIncomeSummary(signal) });
  const valuation = useQuery({ queryKey: ["valuation"], queryFn: ({ signal }) => getPortfolioValuation(signal) });
  const snapshots = useQuery({ queryKey: ["snapshots"], queryFn: ({ signal }) => getPortfolioSnapshots(signal) });
  const allocation = useQuery({ queryKey: ["allocation"], queryFn: ({ signal }) => getPortfolioAllocation(signal), retry: false });
  const accountCount = summary.data?.account_count ?? 0;
  const importCount = summary.data?.import_count ?? 0;
  const reportingCurrency = settings.data?.reporting_currency ?? "GBP";

  if (settings.isPending || currencies.isPending) {
    return <div className="startup-screen"><BrandMark /><span>Preparing your private portfolio…</span></div>;
  }
  if (settings.isError || currencies.isError) {
    return <div className="startup-screen error" role="alert"><strong>Worthweave couldn’t open its settings.</strong><span>{String(settings.error ?? currencies.error)}</span></div>;
  }
  if (!settings.data.onboarding_complete) {
    return <Onboarding currencies={currencies.data} />;
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a className="brand" href="#top" aria-label="Worthweave home">
          <BrandMark />
          <span>worthweave</span>
        </a>
        <nav aria-label="Primary navigation">
          {navItems.map(([label, icon]) => (
            <button className={activeView === label ? "active" : ""} type="button" onClick={() => setActiveView(label)} key={label}>
              <span aria-hidden="true">{icon}</span>
              {label}
            </button>
          ))}
          <button type="button" onClick={() => setSettingsOpen(true)}><span aria-hidden="true">⚙</span>Settings</button>
        </nav>
        <div className="privacy-card">
          <span className="privacy-pulse" />
          <div>
            <strong>Private by design</strong>
            <span>Your data stays on this Mac</span>
          </div>
        </div>
        <button className="profile-button" type="button" aria-label="Open local profile settings" onClick={() => setSettingsOpen(true)}>
          <span className="avatar">KL</span>
          <span><strong>Local portfolio</strong><small>{reportingCurrency} · macOS</small></span>
          <span aria-hidden="true">•••</span>
        </button>
      </aside>

      <main id="top">
        <header className="topbar">
          <div className="eyebrow"><span /> Local intelligence</div>
          <div className="top-actions">
            <button className="icon-button" type="button" aria-label="Search">⌕</button>
            <button className="icon-button" type="button" aria-label="Notifications">♢</button>
            <button className="primary-button" type="button" onClick={() => setImportOpen(true)}><span>＋</span> Import data</button>
          </div>
        </header>

        {activeView === "Portfolio" ? <PortfolioView holdings={holdings.data ?? []} valuation={valuation.data} allocation={allocation.data} snapshots={snapshots.data ?? []} currencies={currencies.data} reportingCurrency={reportingCurrency} /> : activeView === "Activity" ? <ActivityView events={activity.data ?? []} /> : activeView === "Income" ? <IncomeView income={income.data ?? []} /> : <>
        <section className="hero" aria-labelledby="welcome-title">
          <div>
            <p className="kicker">Saturday, 11 July</p>
            <h1 id="welcome-title">Good afternoon.<br /><em>Your wealth, in focus.</em></h1>
            <p className="hero-copy">
              One calm, trustworthy view across every investment account—calculated locally and
              explained in plain English.
            </p>
          </div>
          <StatusOrb imports={importCount} />
        </section>

        {summary.isError && (
          <div className="service-alert" role="alert">
            <span>!</span>
            <div><strong>Portfolio data is unavailable</strong><p>{summary.error.message}</p></div>
          </div>
        )}

        <section className="metric-grid" aria-label="Portfolio readiness">
          <article className="metric-card featured">
            <div className="metric-heading"><span>Total portfolio</span><span className="status-chip">Awaiting data</span></div>
            <strong className="metric-value">—</strong>
            <p>Values appear only after holdings reconcile with your broker exports.</p>
            <div className="metric-sparkline" aria-hidden="true"><span /><span /><span /><span /><span /><span /></div>
          </article>
          <article className="metric-card">
            <div className="metric-icon violet">◈</div>
            <span>Accounts</span>
            <strong className="metric-value small">{summary.isPending ? "…" : accountCount}</strong>
            <p>Trading 212 ISA · IBKR Invest · IBKR ISA</p>
          </article>
          <article className="metric-card">
            <div className="metric-icon amber">↗</div>
            <span>Imports verified</span>
            <strong className="metric-value small">{summary.isPending ? "…" : importCount}</strong>
            <p>Duplicate-safe, account-aware source batches</p>
          </article>
        </section>

        <section className="content-grid">
          <article className="journey-card" id="portfolio">
            <div className="section-heading">
              <div><span className="section-kicker">Get started</span><h2>Build your complete picture</h2></div>
              <span className="step-count">01 / 03</span>
            </div>
            <div className="journey-steps">
              <div className="journey-step current"><span>1</span><div><strong>Create your accounts</strong><p>Keep Invest and ISA histories safely separated.</p></div><button type="button" onClick={() => setImportOpen(true)}>Begin <span>→</span></button></div>
              <div className="journey-step"><span>2</span><div><strong>Import broker history</strong><p>Drop in CSV files now and add later periods anytime.</p></div><small>Next</small></div>
              <div className="journey-step"><span>3</span><div><strong>Reconcile and explore</strong><p>Worthweave checks holdings before showing performance.</p></div><small>Then</small></div>
            </div>
          </article>

          <article className="insight-card" id="insights">
            <div className="insight-glow" />
            <div className="insight-title"><span>✦</span><div><small>Worthweave intelligence</small><strong>Ask your portfolio</strong></div></div>
            <blockquote>“What changed in my portfolio, and why?”</blockquote>
            <p>Answers will cite deterministic portfolio analytics—not guess at your numbers.</p>
            <div className="prompt-row"><button type="button">Concentration risk</button><button type="button">Recent income</button></div>
            <button className="ask-button" type="button" disabled>Available after reconciliation <span>↗</span></button>
          </article>
        </section>
        </>}

        <footer><span>Worthweave · Local mode</span><span>Deterministic ledger <i /> Private AI ready</span></footer>
        <ImportDialog open={importOpen} onClose={() => setImportOpen(false)} />
        <SettingsDialog currencies={currencies.data} currentCurrency={reportingCurrency} open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      </main>
    </div>
  );
}
