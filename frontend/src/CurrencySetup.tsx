import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";

import { completeInitialSetup, createEncryptedBackup, exportPortfolioJson, restoreEncryptedBackup, updateSettings, type CurrencyOption, type InitialAccount } from "./api";
import { AiSettingsPanel } from "./AiSetup";

type CurrencyFormProps = {
  currencies: CurrencyOption[];
  initialCurrency?: string | null;
  onSaved?: () => void;
  submitLabel: string;
};

function CurrencyForm({ currencies, initialCurrency, onSaved, submitLabel }: CurrencyFormProps) {
  const queryClient = useQueryClient();
  const [currency, setCurrency] = useState(initialCurrency ?? "GBP");
  const mutation = useMutation({
    mutationFn: updateSettings,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["settings"] }),
        queryClient.invalidateQueries({ queryKey: ["portfolio-summary"] }),
        queryClient.invalidateQueries({ queryKey: ["valuation"] }),
        queryClient.invalidateQueries({ queryKey: ["allocation"] }),
      ]);
      onSaved?.();
    },
  });
  const selected = currencies.find((option) => option.code === currency);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const submittedCurrency = new FormData(event.currentTarget).get("reporting_currency");
    if (typeof submittedCurrency === "string") mutation.mutate(submittedCurrency);
  }

  return (
    <form className="currency-form" onSubmit={submit}>
      <label htmlFor="reporting-currency">Main currency</label>
      <div className="currency-select-wrap">
        <span aria-hidden="true">{selected?.symbol ?? "¤"}</span>
        <select id="reporting-currency" name="reporting_currency" value={currency} onChange={(event) => setCurrency(event.target.value)}>
          {currencies.map((option) => <option value={option.code} key={option.code}>{option.code} — {option.name}</option>)}
        </select>
      </div>
      <p>Worthweave will show your overall totals and gains in {selected?.name ?? currency}. Imported amounts keep their original currencies.</p>
      <button className="primary-button currency-submit" type="submit" disabled={mutation.isPending || currencies.length === 0}>
        {mutation.isPending ? "Saving…" : submitLabel} <span>→</span>
      </button>
      {mutation.isError && <small className="form-error" role="alert">{String(mutation.error)}</small>}
    </form>
  );
}

export function Onboarding({ currencies }: { currencies: CurrencyOption[] }) {
  const queryClient = useQueryClient();
  const [currency, setCurrency] = useState("GBP");
  const [robinhoodRegion, setRobinhoodRegion] = useState<"GB" | "US">("GB");
  const [selectedAccounts, setSelectedAccounts] = useState(() => new Set(["trading_212:stocks_and_shares_isa", "ibkr:invest", "ibkr:stocks_and_shares_isa"]));
  const accountOptions: Array<InitialAccount & { id: string; label: string; detail: string }> = [
    { id: "trading_212:stocks_and_shares_isa", broker: "trading_212", jurisdiction: "GB", account_type: "stocks_and_shares_isa", display_name: "Trading 212 ISA", label: "Stocks and Shares ISA", detail: "Tax-free investment account" },
    { id: "trading_212:invest", broker: "trading_212", jurisdiction: "GB", account_type: "invest", display_name: "Trading 212 Invest", label: "Invest account", detail: "General investment account" },
    { id: "ibkr:stocks_and_shares_isa", broker: "ibkr", jurisdiction: "GB", account_type: "stocks_and_shares_isa", display_name: "IBKR ISA", label: "Stocks and Shares ISA", detail: "Tax-free investment account" },
    { id: "ibkr:invest", broker: "ibkr", jurisdiction: "GB", account_type: "invest", display_name: "IBKR Invest", label: "Invest account", detail: "General investment account" },
    { id: "robinhood:GB:individual_brokerage", broker: "robinhood", jurisdiction: "GB", account_type: "individual_brokerage", display_name: "Robinhood UK Brokerage", label: "Individual brokerage", detail: "General investment account" },
    { id: "robinhood:GB:stocks_and_shares_isa", broker: "robinhood", jurisdiction: "GB", account_type: "stocks_and_shares_isa", display_name: "Robinhood UK ISA", label: "Stocks and Shares ISA", detail: "Tax-free investment account" },
    { id: "robinhood:US:individual_brokerage", broker: "robinhood", jurisdiction: "US", account_type: "individual_brokerage", display_name: "Robinhood Individual", label: "Individual brokerage", detail: "Taxable brokerage account" },
    { id: "robinhood:US:joint_jtwros", broker: "robinhood", jurisdiction: "US", account_type: "joint_jtwros", display_name: "Robinhood Joint", label: "Joint investing (JTWROS)", detail: "Joint ownership with survivorship" },
    { id: "robinhood:US:traditional_ira", broker: "robinhood", jurisdiction: "US", account_type: "traditional_ira", display_name: "Robinhood Traditional IRA", label: "Traditional IRA", detail: "Tax-advantaged retirement account" },
    { id: "robinhood:US:roth_ira", broker: "robinhood", jurisdiction: "US", account_type: "roth_ira", display_name: "Robinhood Roth IRA", label: "Roth IRA", detail: "After-tax retirement account" },
    { id: "robinhood:US:custodial_utma", broker: "robinhood", jurisdiction: "US", account_type: "custodial_utma", display_name: "Robinhood Custodial", label: "Custodial (UTMA)", detail: "Taxable account for a minor" },
  ];
  const brokerGroups = [
    { id: "trading_212", name: "Trading 212", accounts: accountOptions.filter((account) => account.broker === "trading_212") },
    { id: "ibkr", name: "Interactive Brokers", accounts: accountOptions.filter((account) => account.broker === "ibkr") },
  ];
  const robinhoodAccounts = accountOptions.filter((account) => account.broker === "robinhood" && account.jurisdiction === robinhoodRegion);
  const setup = useMutation({
    mutationFn: () => completeInitialSetup(currency, accountOptions.filter((account) => selectedAccounts.has(account.id))),
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["settings"] }),
  });
  const selectedCurrency = currencies.find((option) => option.code === currency);
  function toggleAccount(id: string) {
    setSelectedAccounts((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }
  return (
    <main className="onboarding-shell">
      <section className="onboarding-panel">
        <div className="onboarding-brand"><span className="onboarding-mark">W</span><strong>worthweave</strong></div>
        <span className="section-kicker">Welcome · Step 1 of 2</span>
        <h1>Bring your portfolio<br /><em>into one clear view.</em></h1>
        <p className="onboarding-intro">Choose how totals are displayed and which accounts you plan to import. Everything can be changed later.</p>
        <form className="initial-setup-form" onSubmit={(event) => { event.preventDefault(); setup.mutate(); }}>
          <label htmlFor="reporting-currency">Main currency</label>
          <div className="currency-select-wrap"><span aria-hidden="true">{selectedCurrency?.symbol ?? "¤"}</span><select id="reporting-currency" value={currency} onChange={(event) => setCurrency(event.target.value)}>{currencies.map((option) => <option value={option.code} key={option.code}>{option.code} — {option.name}</option>)}</select></div>
          <p className="field-help">Your overall value and performance use this currency. Imported amounts keep their original currencies.</p>
          <fieldset className="account-picker"><legend>Brokers and accounts <small>Optional</small></legend><p>Choose the accounts you use. Worthweave always keeps each account’s activity separate.</p><div className="broker-grid">{brokerGroups.map((group) => <fieldset className="broker-card" key={group.id}><legend>{group.name}</legend><div className="account-options">{group.accounts.map((account) => <label key={account.id} className={`account-option ${selectedAccounts.has(account.id) ? "selected" : ""}`}><input type="checkbox" aria-label={`${group.name} ${account.label}`} checked={selectedAccounts.has(account.id)} onChange={() => toggleAccount(account.id)} /><span><strong>{account.label}</strong><small>{account.detail}</small></span><i aria-hidden="true">✓</i></label>)}</div></fieldset>)}<fieldset className="broker-card robinhood-card"><legend>Robinhood</legend><label className="broker-region">Where is your account?<select aria-label="Robinhood account region" value={robinhoodRegion} onChange={(event) => setRobinhoodRegion(event.target.value as "GB" | "US")}><option value="GB">UK</option><option value="US">US</option></select></label><div className="account-options robinhood-account-options">{robinhoodAccounts.map((account) => <label key={account.id} className={`account-option ${selectedAccounts.has(account.id) ? "selected" : ""}`}><input type="checkbox" aria-label={`Robinhood ${account.jurisdiction} ${account.label}`} checked={selectedAccounts.has(account.id)} onChange={() => toggleAccount(account.id)} /><span><strong>{account.label}</strong><small>{account.detail}</small></span><i aria-hidden="true">✓</i></label>)}</div><p className="broker-import-note">Robinhood imports will be available after we have tested sample UK and US export files.</p></fieldset></div></fieldset>
          <button className="primary-button currency-submit" type="submit" disabled={setup.isPending}>{setup.isPending ? "Preparing your portfolio…" : "Continue"} <span>→</span></button>
          {setup.isError && <small className="form-error" role="alert">{String(setup.error)}</small>}
        </form>
        <div className="onboarding-trust"><span>●</span> Saved locally on this Mac</div>
      </section>
      <aside className="onboarding-art" aria-hidden="true">
        <div className="weave-orbit"><span>£</span><span>$</span><span>€</span><strong>W</strong></div>
        <p>Many currencies.<br />One clear view.</p>
      </aside>
    </main>
  );
}

type SettingsDialogProps = {
  currencies: CurrencyOption[];
  currentCurrency: string;
  open: boolean;
  onClose: () => void;
  aiRuntime?: string | null;
  aiModel?: string | null;
};

export function SettingsDialog({ currencies, currentCurrency, open, onClose, aiRuntime = null, aiModel = null }: SettingsDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog className="settings-dialog" ref={dialogRef} onClose={onClose}>
      <div className="dialog-topline">
        <div><span className="section-kicker">Preferences</span><h2>Settings</h2></div>
        <button type="button" className="dialog-close" onClick={onClose} aria-label="Close settings">×</button>
      </div>
      <section className="settings-content">
        <div><h3>Main currency</h3><p>Choose the currency used for overall totals. Your imported amounts will not be changed.</p></div>
        <CurrencyForm currencies={currencies} initialCurrency={currentCurrency} onSaved={onClose} submitLabel="Save changes" />
      </section>
      {open && <AiSettingsPanel runtime={aiRuntime} model={aiModel} />}
      <BackupPanel />
    </dialog>
  );
}

function BackupPanel() {
  const queryClient = useQueryClient();
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState("");
  const [confirmRestore, setConfirmRestore] = useState(false);
  const backupMutation = useMutation({ mutationFn: async () => {
    const path = await saveFileDialog({ defaultPath: "worthweave-backup.age", filters: [{ name: "Worthweave encrypted backup", extensions: ["age"] }] });
    if (path) await createEncryptedBackup(path, password);
    return path;
  }, onSuccess: (path) => { if (path) { setStatus("Encrypted backup created."); setPassword(""); } } });
  const restoreMutation = useMutation({ mutationFn: async () => {
    const path = await openFileDialog({ multiple: false, directory: false, filters: [{ name: "Worthweave encrypted backup", extensions: ["age"] }] });
    if (path) await restoreEncryptedBackup(path, password);
    return path;
  }, onSuccess: async (path) => { if (path) { setStatus("Backup restored."); setPassword(""); setConfirmRestore(false); await queryClient.invalidateQueries(); } } });
  const exportMutation = useMutation({ mutationFn: async () => {
    const path = await saveFileDialog({ defaultPath: "worthweave-portfolio.json", filters: [{ name: "Worthweave portfolio export", extensions: ["json"] }] });
    if (path) await exportPortfolioJson(path);
    return path;
  }, onSuccess: (path) => { if (path) setStatus("Portfolio report exported."); } });
  const busy = backupMutation.isPending || restoreMutation.isPending;
  return <section className="backup-settings"><div><h3>Export &amp; encrypted backup</h3><p>Export a readable portfolio report, or create and restore a complete encrypted backup. Backup passwords cannot be recovered.</p></div><div><label>Backup password<input type="password" minLength={12} value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="new-password" placeholder="At least 12 characters" /></label><label className="restore-confirm"><input type="checkbox" role="switch" checked={confirmRestore} onChange={(event) => setConfirmRestore(event.target.checked)} /><span className="restore-toggle" aria-hidden="true"><span /></span><span>I understand restoring replaces all current portfolio data.</span></label><div className="backup-actions"><button type="button" className="secondary-button" disabled={exportMutation.isPending} onClick={() => exportMutation.mutate()}>Export JSON</button><button type="button" className="secondary-button" disabled={password.length < 12 || busy} onClick={() => backupMutation.mutate()}>Create backup</button><button type="button" className="secondary-button danger-button" disabled={password.length < 12 || busy || !confirmRestore} onClick={() => restoreMutation.mutate()}>Restore backup</button></div>{status && <small className="backup-success">{status}</small>}{(backupMutation.isError || restoreMutation.isError || exportMutation.isError) && <small className="form-error">{String(backupMutation.error ?? restoreMutation.error ?? exportMutation.error)}</small>}</div></section>;
}
