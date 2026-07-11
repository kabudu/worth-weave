import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";

import { createEncryptedBackup, exportPortfolioJson, restoreEncryptedBackup, updateSettings, type CurrencyOption } from "./api";
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
      <label htmlFor="reporting-currency">Reporting currency</label>
      <div className="currency-select-wrap">
        <span aria-hidden="true">{selected?.symbol ?? "¤"}</span>
        <select id="reporting-currency" name="reporting_currency" value={currency} onChange={(event) => setCurrency(event.target.value)}>
          {currencies.map((option) => <option value={option.code} key={option.code}>{option.code} — {option.name}</option>)}
        </select>
      </div>
      <p>Portfolio totals, gains and reports will be converted into {selected?.name ?? currency}. Original broker currencies remain unchanged.</p>
      <button className="primary-button currency-submit" type="submit" disabled={mutation.isPending || currencies.length === 0}>
        {mutation.isPending ? "Saving…" : submitLabel} <span>→</span>
      </button>
      {mutation.isError && <small className="form-error" role="alert">{String(mutation.error)}</small>}
    </form>
  );
}

export function Onboarding({ currencies }: { currencies: CurrencyOption[] }) {
  return (
    <main className="onboarding-shell">
      <section className="onboarding-panel">
        <div className="onboarding-brand"><span className="onboarding-mark">W</span><strong>worthweave</strong></div>
        <span className="section-kicker">Welcome · Step 1 of 2</span>
        <h1>Make every number<br /><em>feel like home.</em></h1>
        <p className="onboarding-intro">Choose the currency Worthweave should use when bringing your investments together. You can change it later in Settings.</p>
        <CurrencyForm currencies={currencies} submitLabel="Continue" />
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
        <div><h3>Reporting currency</h3><p>Changes how consolidated portfolio values are presented. Source transactions are never rewritten.</p></div>
        <CurrencyForm currencies={currencies} initialCurrency={currentCurrency} onSaved={onClose} submitLabel="Save changes" />
      </section>
      <AiSettingsPanel runtime={aiRuntime} model={aiModel} />
      <BackupPanel />
    </dialog>
  );
}

function BackupPanel() {
  const queryClient = useQueryClient();
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState("");
  const backupMutation = useMutation({ mutationFn: async () => {
    const path = await saveFileDialog({ defaultPath: "worthweave-backup.age", filters: [{ name: "Worthweave encrypted backup", extensions: ["age"] }] });
    if (path) await createEncryptedBackup(path, password);
    return path;
  }, onSuccess: (path) => { if (path) { setStatus("Encrypted backup created."); setPassword(""); } } });
  const restoreMutation = useMutation({ mutationFn: async () => {
    const path = await openFileDialog({ multiple: false, directory: false, filters: [{ name: "Worthweave encrypted backup", extensions: ["age"] }] });
    if (path) await restoreEncryptedBackup(path, password);
    return path;
  }, onSuccess: async (path) => { if (path) { setStatus("Backup restored."); setPassword(""); await queryClient.invalidateQueries(); } } });
  const exportMutation = useMutation({ mutationFn: async () => {
    const path = await saveFileDialog({ defaultPath: "worthweave-portfolio.json", filters: [{ name: "Worthweave portfolio export", extensions: ["json"] }] });
    if (path) await exportPortfolioJson(path);
    return path;
  }, onSuccess: (path) => { if (path) setStatus("Portfolio report exported."); } });
  const busy = backupMutation.isPending || restoreMutation.isPending;
  return <section className="backup-settings"><div><h3>Export &amp; encrypted backup</h3><p>Export a readable portfolio report, or create and restore a complete encrypted backup. Backup passwords cannot be recovered.</p></div><div><label>Backup password<input type="password" minLength={12} value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="new-password" placeholder="At least 12 characters" /></label><div className="backup-actions"><button type="button" className="secondary-button" disabled={exportMutation.isPending} onClick={() => exportMutation.mutate()}>Export JSON</button><button type="button" className="secondary-button" disabled={password.length < 12 || busy} onClick={() => backupMutation.mutate()}>Create backup</button><button type="button" className="secondary-button" disabled={password.length < 12 || busy} onClick={() => restoreMutation.mutate()}>Restore backup</button></div>{status && <small className="backup-success">{status}</small>}{(backupMutation.isError || restoreMutation.isError || exportMutation.isError) && <small className="form-error">{String(backupMutation.error ?? restoreMutation.error ?? exportMutation.error)}</small>}</div></section>;
}
