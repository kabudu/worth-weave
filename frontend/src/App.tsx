import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CSSProperties } from "react";
import { useEffect, useRef, useState } from "react";

import { capturePortfolioSnapshot, getAccounts, getActivity, getBrokerConnectionStatuses, getCurrencies, getHoldings, getIncomeSummary, getPortfolioAllocation, getPortfolioReconciliation, getPortfolioSnapshots, getPortfolioSummary, getPortfolioValuation, getSettings, getTotalReturnAttribution, refreshFxRates, refreshPortfolioHistory, syncBroker } from "./api";
import { Onboarding, SettingsDialog } from "./CurrencySetup";
import { AiOnboarding } from "./AiSetup";
import { ImportDialog } from "./ImportDialog";
import { InsightsCard } from "./InsightsCard";
import { ActivityView, IncomeView, PortfolioView } from "./ReportingViews";
import { UpdateBanner } from "./UpdateBanner";
import { AboutDialog } from "./AboutDialog";

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

function PortfolioLoading() {
  return (
    <section className="portfolio-loading" role="status" aria-live="polite" aria-label="Loading portfolio">
      <div className="portfolio-loading-mark"><BrandMark /><span className="portfolio-loading-ring" /></div>
      <span className="section-kicker">Preparing your portfolio</span>
      <h1>Bringing your figures together…</h1>
      <p>Worthweave is calculating holdings, prices and history securely on this Mac.</p>
      <div className="portfolio-loading-progress" aria-hidden="true"><span /></div>
      <small>Large account histories can take a little longer the first time. You can keep Worthweave open.</small>
    </section>
  );
}

function StatusOrb({ accounts, imports, valuedCount, totalCount, missingPrices }: { accounts: number; imports: number; valuedCount: number; totalCount: number; missingPrices: number }) {
  const progress = totalCount > 0 ? Math.round(valuedCount / totalCount * 100) : imports > 0 ? 18 : accounts > 0 ? 10 : 4;
  const headline = totalCount > 0 ? `${valuedCount}/${totalCount}` : imports > 0 ? "Imported" : accounts > 0 ? "Ready" : "Start";
  const label = totalCount > 0 ? "holdings valued" : imports > 0 ? "add market data" : accounts > 0 ? "import a file" : "add an account";
  return (
    <div className="status-orb" style={{ "--progress": `${progress * 3.6}deg` } as CSSProperties}>
      <div>
        <strong>{headline}</strong>
        <span>{label}</span>
        {totalCount > 0 && missingPrices > 0 && <small>{missingPrices} prices needed</small>}
      </div>
    </div>
  );
}

export function App() {
  const queryClient = useQueryClient();
  const [activeView, setActiveView] = useState("Overview");
  const [importOpen, setImportOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen("open-about-worthweave", () => setAboutOpen(true));
    return () => { void unlisten.then((dispose) => dispose()); };
  }, []);
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: ({ signal }) => getSettings(signal),
  });
  const currencies = useQuery({
    queryKey: ["currencies"],
    queryFn: ({ signal }) => getCurrencies(signal),
    staleTime: Infinity,
  });
  const ready = Boolean(settings.data?.onboarding_complete && settings.data?.ai_onboarding_complete);
  const summary = useQuery({
    queryKey: ["portfolio-summary"],
    queryFn: ({ signal }) => getPortfolioSummary(signal), enabled: ready,
  });
  const holdings = useQuery({ queryKey: ["holdings"], queryFn: ({ signal }) => getHoldings(signal), enabled: ready, staleTime: 5 * 60_000 });
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: ({ signal }) => getAccounts(signal), enabled: ready, staleTime: 5 * 60_000 });
  const brokerConnections = useQuery({ queryKey: ["broker-connections"], queryFn: getBrokerConnectionStatuses, enabled: ready, refetchInterval: (query) => query.state.data?.some((status) => status.sync_state === "preparing") ? 65_000 : false });
  const brokerAttempts = useRef(new Map<string, number>());
  const brokerSync = useMutation({ mutationFn: syncBroker, onSuccess: async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["broker-connections"] }),
      queryClient.invalidateQueries({ queryKey: ["portfolio-summary"] }),
      queryClient.invalidateQueries({ queryKey: ["holdings"] }),
      queryClient.invalidateQueries({ queryKey: ["valuation"] }),
      queryClient.invalidateQueries({ queryKey: ["allocation"] }),
      queryClient.invalidateQueries({ queryKey: ["total-return"] }),
      queryClient.invalidateQueries({ queryKey: ["reconciliation"] }),
    ]);
  } });
  useEffect(() => {
    if (!brokerConnections.data || brokerSync.isPending) return;
    const now = Date.now();
    const due = brokerConnections.data.find((status) => {
      if (!status.configured) return false;
      if (status.sync_state === "attention") return false;
      const attempted = brokerAttempts.current.get(status.account_id) ?? 0;
      if (now - attempted < 65_000) return false;
      if (status.sync_state === "preparing") return true;
      if (!status.last_success_at) return true;
      return now - new Date(status.last_success_at).getTime() >= 24 * 60 * 60_000;
    });
    if (due) {
      brokerAttempts.current.set(due.account_id, now);
      brokerSync.mutate(due.account_id);
    }
  }, [brokerConnections.data, brokerSync]);
  const activity = useQuery({ queryKey: ["activity"], queryFn: ({ signal }) => getActivity(signal), enabled: ready && activeView === "Activity" });
  const income = useQuery({ queryKey: ["income"], queryFn: ({ signal }) => getIncomeSummary(signal), enabled: ready && activeView === "Income" });
  const valuation = useQuery({ queryKey: ["valuation"], queryFn: ({ signal }) => getPortfolioValuation(signal), enabled: ready && (activeView === "Overview" || activeView === "Portfolio") });
  const autoSnapshot = useMutation({ mutationFn: capturePortfolioSnapshot, onSuccess: () => queryClient.invalidateQueries({ queryKey: ["performance"] }) });
  useEffect(() => {
    if (valuation.data?.valuation_complete && autoSnapshot.isIdle) autoSnapshot.mutate();
  }, [valuation.data?.valuation_complete, autoSnapshot]);
  const fxRefresh = useMutation({
    mutationFn: refreshFxRates,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["valuation"] }),
        queryClient.invalidateQueries({ queryKey: ["allocation"] }),
        queryClient.invalidateQueries({ queryKey: ["total-return"] }),
      ]);
    },
  });
  useEffect(() => {
    if (ready && fxRefresh.isIdle) fxRefresh.mutate();
  }, [ready, fxRefresh]);
  const attribution = useQuery({ queryKey: ["total-return"], queryFn: ({ signal }) => getTotalReturnAttribution(signal), enabled: ready && activeView === "Portfolio" });
  const snapshots = useQuery({ queryKey: ["snapshots"], queryFn: ({ signal }) => getPortfolioSnapshots(signal), enabled: ready && activeView === "Portfolio" });
  const historyRefresh = useMutation({
    mutationFn: refreshPortfolioHistory,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["performance"] }),
        queryClient.invalidateQueries({ queryKey: ["valuation"] }),
        queryClient.invalidateQueries({ queryKey: ["allocation"] }),
      ]);
    },
  });
  useEffect(() => {
    if (ready && historyRefresh.isIdle) historyRefresh.mutate();
  }, [ready, historyRefresh]);
  const allocation = useQuery({ queryKey: ["allocation"], queryFn: ({ signal }) => getPortfolioAllocation(signal), retry: false, enabled: ready && activeView === "Portfolio" });
  const reconciliation = useQuery({ queryKey: ["reconciliation"], queryFn: ({ signal }) => getPortfolioReconciliation(signal), enabled: ready && activeView === "Portfolio" });
  const portfolioIsPreparing = activeView === "Portfolio" && [
    accounts,
    holdings,
    valuation,
    attribution,
    allocation,
    reconciliation,
    snapshots,
  ].some((query) => query.isPending);
  const accountCount = summary.data?.account_count ?? 0;
  const importCount = summary.data?.import_count ?? 0;
  const reportingCurrency = settings.data?.reporting_currency ?? "GBP";
  const now = new Date();
  const greeting = now.getHours() < 12 ? "Good morning" : now.getHours() < 18 ? "Good afternoon" : "Good evening";
  const dateLabel = new Intl.DateTimeFormat(undefined, { weekday: "long", day: "numeric", month: "long" }).format(now);
  const journeyStep = accountCount === 0 ? 1 : importCount === 0 ? 2 : 3;

  if (settings.isPending || currencies.isPending) {
    return <div className="startup-screen"><BrandMark /><span>Preparing your private portfolio…</span></div>;
  }
  if (settings.isError || currencies.isError) {
    return <div className="startup-screen error" role="alert"><strong>Worthweave couldn’t open its settings.</strong><span>{String(settings.error ?? currencies.error)}</span></div>;
  }
  if (!settings.data.onboarding_complete) {
    return <Onboarding currencies={currencies.data} />;
  }
  if (!settings.data.ai_onboarding_complete) return <AiOnboarding />;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <button className="brand" type="button" onClick={() => setActiveView("Overview")} aria-label="Worthweave home">
          <BrandMark />
          <span>worthweave</span>
        </button>
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
          <span className="avatar">W</span>
          <span><strong>Local portfolio</strong><small>{reportingCurrency} · macOS</small></span>
          <span aria-hidden="true">•••</span>
        </button>
      </aside>

      <main id="top">
        <header className="topbar">
          <div className="eyebrow"><span /> Your portfolio stays on this Mac</div>
          <div className="top-actions">
            <button className="primary-button" type="button" onClick={() => setImportOpen(true)}><span>＋</span> Import data</button>
          </div>
        </header>

        <UpdateBanner />

        {activeView === "Portfolio" ? portfolioIsPreparing ? <PortfolioLoading /> : <PortfolioView accounts={accounts.data ?? []} holdings={holdings.data ?? []} reconciliation={reconciliation.data ?? []} valuation={valuation.data} attribution={attribution.data} allocation={allocation.data} snapshots={snapshots.data ?? []} currencies={currencies.data} reportingCurrency={reportingCurrency} /> : activeView === "Activity" ? <ActivityView events={activity.data ?? []} /> : activeView === "Income" ? <IncomeView income={income.data ?? []} /> : activeView === "Insights" ? <section className="report-page insights-page"><header><span className="section-kicker">Private AI</span><h1>Ask about your portfolio</h1><p>Get clear answers based on the figures already shown in Worthweave.</p></header><InsightsCard configured={Boolean(settings.data.ai_runtime && settings.data.ai_model && settings.data.ai_endpoint)} onOpenSettings={() => setSettingsOpen(true)} /></section> : <>
        <section className="hero" aria-labelledby="welcome-title">
          <div>
            <p className="kicker">{dateLabel}</p>
            <h1 id="welcome-title">{greeting}.<br /><em>Your wealth, in focus.</em></h1>
            <p className="hero-copy">
              One calm, trustworthy view across every investment account—calculated on this Mac and
              explained in plain English.
            </p>
          </div>
          <StatusOrb accounts={accountCount} imports={importCount} valuedCount={valuation.data?.valued_holding_count ?? 0} totalCount={valuation.data?.holdings.length ?? 0} missingPrices={valuation.data?.missing_price_count ?? 0} />
        </section>

        {summary.isError && (
          <div className="service-alert" role="alert">
            <span>!</span>
            <div><strong>Portfolio data is unavailable</strong><p>{summary.error.message}</p></div>
          </div>
        )}

        <section className="metric-grid" aria-label="Portfolio readiness">
          <article className="metric-card featured">
            <div className="metric-heading"><span>{valuation.data?.valuation_complete ? "Total portfolio" : "Valued so far"}</span><span className="status-chip">{valuation.data?.valuation_complete ? "Valued" : valuation.data?.total_value ? "Partial" : importCount > 0 ? "Needs market data" : "Awaiting data"}</span></div>
            <strong className="metric-value">{valuation.data?.total_value ? new Intl.NumberFormat(undefined, { style: "currency", currency: reportingCurrency }).format(Number(valuation.data.total_value)) : "—"}</strong>
            <p>{valuation.data?.valuation_complete ? `${valuation.data.total_gain_loss ? `${new Intl.NumberFormat(undefined, { style: "currency", currency: reportingCurrency }).format(Number(valuation.data.total_gain_loss))} total gain/loss · ` : ""}${valuation.data.stale_price_count + valuation.data.stale_fx_count} prices or exchange rates need updating` : valuation.data?.total_value ? `${valuation.data.valued_holding_count} holdings valued · ${valuation.data.missing_price_count} still need prices` : fxRefresh.isPending ? "Refreshing reference exchange rates…" : "Add current prices to calculate your portfolio value."}</p>
          </article>
          <article className="metric-card">
            <div className="metric-icon violet">◈</div>
            <span>Accounts</span>
            <strong className="metric-value small">{summary.isPending ? "…" : accountCount}</strong>
            <p>{accountCount === 0 ? "Add each Invest and ISA account separately" : `${accountCount} account${accountCount === 1 ? "" : "s"} ready for imports`}</p>
          </article>
          <article className="metric-card">
            <div className="metric-icon amber">↗</div>
            <span>Files imported</span>
            <strong className="metric-value small">{summary.isPending ? "…" : importCount}</strong>
            <p>Broker files checked for duplicates and assigned to the right account</p>
          </article>
        </section>

        <section className="content-grid">
          <article className="journey-card" id="portfolio">
            <div className="section-heading">
              <div><span className="section-kicker">Get started</span><h2>Build your complete picture</h2></div>
              <span className="step-count">0{journeyStep} / 03</span>
            </div>
            <div className="journey-steps">
              <div className={`journey-step ${journeyStep === 1 ? "current" : "complete"}`}><span>{journeyStep > 1 ? "✓" : "1"}</span><div><strong>Create your accounts</strong><p>Keep Invest and ISA histories safely separated.</p></div>{journeyStep === 1 ? <button type="button" onClick={() => setImportOpen(true)}>Begin <span>→</span></button> : <small>Done</small>}</div>
              <div className={`journey-step ${journeyStep === 2 ? "current" : journeyStep > 2 ? "complete" : ""}`}><span>{journeyStep > 2 ? "✓" : "2"}</span><div><strong>Import broker history</strong><p>Drop in CSV files now and add later periods anytime.</p></div>{journeyStep === 2 ? <button type="button" onClick={() => setImportOpen(true)}>Import <span>→</span></button> : <small>{journeyStep > 2 ? "Done" : "Next"}</small>}</div>
              <div className={`journey-step ${journeyStep === 3 ? "current" : ""}`}><span>3</span><div><strong>Check and explore</strong><p>Compare your holdings and see how your investments are doing.</p></div>{journeyStep === 3 ? <button type="button" onClick={() => setActiveView("Portfolio")}>Explore <span>→</span></button> : <small>Then</small>}</div>
            </div>
          </article>

          <InsightsCard configured={Boolean(settings.data.ai_runtime && settings.data.ai_model && settings.data.ai_endpoint)} onOpenSettings={() => setSettingsOpen(true)} />
        </section>
        </>}

        <footer><span>Worthweave · Your data stays here</span><span>Figures calculated by Worthweave <i /> {settings.data.ai_runtime ? "Private AI ready" : "Private AI optional"}</span></footer>
        <ImportDialog open={importOpen} onClose={() => setImportOpen(false)} />
        <SettingsDialog accounts={accounts.data ?? []} currencies={currencies.data} currentCurrency={reportingCurrency} aiRuntime={settings.data.ai_runtime} aiModel={settings.data.ai_model} open={settingsOpen} onClose={() => setSettingsOpen(false)} />
        <AboutDialog open={aboutOpen} onClose={() => setAboutOpen(false)} />
      </main>
    </div>
  );
}
