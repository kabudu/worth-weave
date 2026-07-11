import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import {
  createAccount,
  getAccounts,
  importBrokerFile,
  type Account,
  type AccountType,
  type Broker,
  type ImportResult,
} from "./api";

type ImportDialogProps = {
  open: boolean;
  onClose: () => void;
};

export function ImportDialog({ open, onClose }: ImportDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const queryClient = useQueryClient();
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: ({ signal }) => getAccounts(signal) });
  const [selectedId, setSelectedId] = useState("");
  const [broker, setBroker] = useState<Broker>("trading_212");
  const [accountType, setAccountType] = useState<AccountType>("stocks_and_shares_isa");
  const [displayName, setDisplayName] = useState("Trading 212 ISA");
  const [file, setFile] = useState<File | null>(null);
  const [result, setResult] = useState<ImportResult | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  const effectiveSelectedId = selectedId || accounts.data?.[0]?.id || "";

  const createMutation = useMutation({
    mutationFn: createAccount,
    onSuccess: async (account) => {
      await queryClient.invalidateQueries({ queryKey: ["accounts"] });
      await queryClient.invalidateQueries({ queryKey: ["portfolio-summary"] });
      setSelectedId(account.id);
    },
  });
  const importMutation = useMutation({
    mutationFn: ({ account, source }: { account: Account; source: File }) =>
      importBrokerFile(account, source),
    onSuccess: async (nextResult) => {
      setResult(nextResult);
      await queryClient.invalidateQueries({ queryKey: ["portfolio-summary"] });
    },
  });

  function handleCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    createMutation.mutate({ broker, account_type: accountType, display_name: displayName.trim() });
  }

  function handleImport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const account = accounts.data?.find((candidate) => candidate.id === effectiveSelectedId);
    if (account && file) importMutation.mutate({ account, source: file });
  }

  function close() {
    setResult(null);
    importMutation.reset();
    createMutation.reset();
    onClose();
  }

  return (
    <dialog ref={dialogRef} className="import-dialog" onClose={close}>
      <div className="dialog-topline">
        <div><span className="section-kicker">Secure local import</span><h2>Add portfolio data</h2></div>
        <button type="button" className="dialog-close" onClick={close} aria-label="Close import dialog">×</button>
      </div>

      {result ? (
        <section className="import-success" aria-live="polite">
          <span>✓</span>
          <h3>Import verified</h3>
          <p>{result.events_added.toLocaleString()} canonical events added.</p>
          <dl><div><dt>Coverage starts</dt><dd>{result.coverage_start}</dd></div><div><dt>Coverage ends</dt><dd>{result.coverage_end}</dd></div></dl>
          {result.warnings.map((warning) => <small key={warning}>{warning}</small>)}
          <button className="primary-button" type="button" onClick={close}>Return to overview</button>
        </section>
      ) : (
        <div className="dialog-columns">
          <form onSubmit={handleCreate}>
            <div className="form-number">1</div>
            <h3>Create an account</h3>
            <p>Account boundaries keep ISA and taxable activity separate.</p>
            <label>Platform<select value={broker} onChange={(event) => setBroker(event.target.value as Broker)}><option value="trading_212">Trading 212</option><option value="ibkr">Interactive Brokers</option></select></label>
            <label>Account type<select value={accountType} onChange={(event) => setAccountType(event.target.value as AccountType)}><option value="stocks_and_shares_isa">Stocks &amp; Shares ISA</option><option value="invest">Invest</option></select></label>
            <label>Account name<input value={displayName} maxLength={160} required onChange={(event) => setDisplayName(event.target.value)} /></label>
            <button type="submit" className="secondary-button" disabled={createMutation.isPending || !displayName.trim()}>{createMutation.isPending ? "Creating…" : "Create account"}</button>
            {createMutation.isError && <small className="form-error" role="alert">{createMutation.error.message}</small>}
          </form>

          <form onSubmit={handleImport}>
            <div className="form-number">2</div>
            <h3>Choose a CSV export</h3>
            <p>The file is processed locally and protected from duplicate imports.</p>
            <label>Destination account<select value={effectiveSelectedId} required onChange={(event) => setSelectedId(event.target.value)}><option value="" disabled>Select an account</option>{accounts.data?.map((account) => <option value={account.id} key={account.id}>{account.display_name}</option>)}</select></label>
            <label className="file-drop"><input type="file" accept=".csv,text/csv" required onChange={(event) => setFile(event.target.files?.[0] ?? null)} /><span>{file ? file.name : "Choose CSV file"}</span><small>Maximum 50 MB · CSV only</small></label>
            <button type="submit" className="primary-button" disabled={!effectiveSelectedId || !file || importMutation.isPending}>{importMutation.isPending ? "Verifying…" : "Verify and import"}</button>
            {importMutation.isError && <small className="form-error" role="alert">{importMutation.error.message}</small>}
          </form>
        </div>
      )}
      <p className="dialog-privacy"><span>●</span> Broker credentials are never required for file imports.</p>
    </dialog>
  );
}
