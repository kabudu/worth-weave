import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
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
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: ({ signal }) => getAccounts(signal), enabled: open });
  const [selectedId, setSelectedId] = useState("");
  const [broker, setBroker] = useState<Broker>("trading_212");
  const [jurisdiction, setJurisdiction] = useState<Account["jurisdiction"]>("GB");
  const [accountType, setAccountType] = useState<AccountType>("stocks_and_shares_isa");
  const [displayName, setDisplayName] = useState("Trading 212 ISA");
  const [filePaths, setFilePaths] = useState<string[]>([]);
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
    mutationFn: async ({ account, sources }: { account: Account; sources: string[] }) => {
      const results: ImportResult[] = [];
      for (const source of sources) results.push(await importBrokerFile(account, source));
      return {
        ...results.at(-1)!,
        coverage_start: results.map((item) => item.coverage_start).sort()[0]!,
        coverage_end: results.map((item) => item.coverage_end).sort().at(-1)!,
        events_added: results.reduce((total, item) => total + item.events_added, 0),
        warnings: results.flatMap((item) => item.warnings),
      };
    },
    onSuccess: async (nextResult) => {
      setResult(nextResult);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["portfolio-summary"] }),
        queryClient.invalidateQueries({ queryKey: ["holdings"] }),
        queryClient.invalidateQueries({ queryKey: ["activity"] }),
        queryClient.invalidateQueries({ queryKey: ["income"] }),
        queryClient.invalidateQueries({ queryKey: ["valuation"] }),
        queryClient.invalidateQueries({ queryKey: ["allocation"] }),
        queryClient.invalidateQueries({ queryKey: ["reconciliation"] }),
        queryClient.invalidateQueries({ queryKey: ["total-return"] }),
      ]);
    },
  });

  function handleCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    createMutation.mutate({ broker, jurisdiction, account_type: accountType, display_name: displayName.trim() });
  }

  function handleImport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const account = accounts.data?.find((candidate) => candidate.id === effectiveSelectedId);
    if (account && account.broker !== "robinhood" && filePaths.length > 0) importMutation.mutate({ account, sources: filePaths });
  }

  async function chooseFile() {
    const selected = await openFileDialog({
      multiple: true,
      directory: false,
      filters: [{ name: "Broker CSV export", extensions: ["csv"] }],
    });
    if (selected) setFilePaths(Array.isArray(selected) ? selected : [selected]);
  }

  function close() {
    setResult(null);
    importMutation.reset();
    createMutation.reset();
    onClose();
  }
  const selectedAccount = accounts.data?.find((candidate) => candidate.id === effectiveSelectedId);
  const importSupported = selectedAccount?.broker !== "robinhood";
  const accountTypeOptions: Array<{ value: AccountType; label: string }> = broker === "robinhood"
    ? jurisdiction === "GB"
      ? [{ value: "individual_brokerage", label: "Individual brokerage" }, { value: "stocks_and_shares_isa", label: "Stocks & Shares ISA" }]
      : [{ value: "individual_brokerage", label: "Individual brokerage" }, { value: "joint_jtwros", label: "Joint investing (JTWROS)" }, { value: "traditional_ira", label: "Traditional IRA" }, { value: "roth_ira", label: "Roth IRA" }, { value: "custodial_utma", label: "Custodial (UTMA)" }]
    : [{ value: "stocks_and_shares_isa", label: "Stocks & Shares ISA" }, { value: "invest", label: "Invest" }];
  function defaultAccountName(nextBroker: Broker, nextJurisdiction: Account["jurisdiction"], nextType: AccountType) {
    if (nextBroker === "trading_212") return `Trading 212 ${nextType === "stocks_and_shares_isa" ? "ISA" : "Invest"}`;
    if (nextBroker === "ibkr") return `IBKR ${nextType === "stocks_and_shares_isa" ? "ISA" : "Invest"}`;
    const names: Partial<Record<AccountType, string>> = {
      individual_brokerage: nextJurisdiction === "US" ? "Robinhood Individual" : "Robinhood UK Brokerage",
      stocks_and_shares_isa: "Robinhood UK ISA",
      joint_jtwros: "Robinhood Joint",
      traditional_ira: "Robinhood Traditional IRA",
      roth_ira: "Robinhood Roth IRA",
      custodial_utma: "Robinhood Custodial",
    };
    return names[nextType] ?? "Robinhood account";
  }
  function changeBroker(nextBroker: Broker) {
    setBroker(nextBroker);
    setJurisdiction("GB");
    const nextType = nextBroker === "robinhood" ? "individual_brokerage" : "stocks_and_shares_isa";
    setAccountType(nextType);
    setDisplayName(defaultAccountName(nextBroker, "GB", nextType));
  }
  function changeJurisdiction(nextJurisdiction: Account["jurisdiction"]) {
    setJurisdiction(nextJurisdiction);
    setAccountType("individual_brokerage");
    setDisplayName(defaultAccountName("robinhood", nextJurisdiction, "individual_brokerage"));
  }
  function changeAccountType(nextType: AccountType) {
    setAccountType(nextType);
    setDisplayName(defaultAccountName(broker, jurisdiction, nextType));
  }

  return (
    <dialog ref={dialogRef} className="import-dialog" onClose={close}>
      <div className="dialog-topline">
        <div><span className="section-kicker">Your files stay on this Mac</span><h2>Import account history</h2></div>
        <button type="button" className="dialog-close" onClick={close} aria-label="Close import dialog">×</button>
      </div>

      {result ? (
        <section className="import-success" aria-live="polite">
          <span>✓</span>
          <h3>{result.events_added === 0 ? "Import checked" : "Import complete"}</h3>
          <p>{result.events_added === 0 ? "No duplicate transactions were added. Existing investment links and broker market data were refreshed." : `${result.events_added.toLocaleString()} transactions and cash events added.`}</p>
          <dl><div><dt>First date</dt><dd>{result.coverage_start}</dd></div><div><dt>Last date</dt><dd>{result.coverage_end}</dd></div></dl>
          {result.warnings.map((warning) => <small key={warning}>{warning}</small>)}
          <button className="primary-button" type="button" onClick={close}>Done</button>
        </section>
      ) : (
        <div className="dialog-columns">
          <form onSubmit={handleCreate}>
            <div className="form-number">1</div>
            <h3>Create an account</h3>
            <p>Create each account separately so their investments and tax treatment never get mixed together.</p>
            <label>Broker<select value={broker} onChange={(event) => changeBroker(event.target.value as Broker)}><option value="trading_212">Trading 212</option><option value="ibkr">Interactive Brokers</option><option value="robinhood">Robinhood</option></select></label>
            {broker === "robinhood" && <label>Region<select value={jurisdiction} onChange={(event) => changeJurisdiction(event.target.value as Account["jurisdiction"])}><option value="GB">United Kingdom</option><option value="US">United States</option></select></label>}
            <label>Account type<select value={accountType} onChange={(event) => changeAccountType(event.target.value as AccountType)}>{accountTypeOptions.map((option) => <option value={option.value} key={option.value}>{option.label}</option>)}</select></label>
            <label>Account name<input value={displayName} maxLength={160} required onChange={(event) => setDisplayName(event.target.value)} /></label>
            <button type="submit" className="secondary-button" disabled={createMutation.isPending || !displayName.trim()}>{createMutation.isPending ? "Creating…" : "Create account"}</button>
            {createMutation.isError && <small className="form-error" role="alert">{createMutation.error.message}</small>}
          </form>

          <form onSubmit={handleImport}>
            <div className="form-number">2</div>
            <h3>Choose an exported CSV file</h3>
            <p>Worthweave checks the file on this Mac and will not import the same file twice.</p>
            <label>Import into<select value={effectiveSelectedId} required onChange={(event) => setSelectedId(event.target.value)}><option value="" disabled>Select an account</option>{accounts.data?.map((account) => <option value={account.id} key={account.id}>{account.display_name}</option>)}</select></label>
            <button className="file-drop" type="button" disabled={!importSupported} onClick={chooseFile}><span>{importSupported ? filePaths.length > 1 ? `${filePaths.length} CSV files selected` : filePaths.length === 1 ? filePaths[0]!.split(/[\\/]/).at(-1) : "Choose CSV files" : "Robinhood import is not ready yet"}</span><small>{importSupported ? "Select one or several exports · up to 50 MB each" : "We need to test sample Robinhood export files first"}</small></button>
            <button type="submit" className="primary-button" disabled={!effectiveSelectedId || filePaths.length === 0 || !importSupported || importMutation.isPending}>{importMutation.isPending ? `Checking ${filePaths.length} file${filePaths.length === 1 ? "" : "s"}…` : `Import ${filePaths.length > 1 ? `${filePaths.length} files` : "file"}`}</button>
            {importMutation.isError && <small className="form-error" role="alert">{importMutation.error.message}</small>}
          </form>
        </div>
      )}
      <p className="dialog-privacy"><span>●</span> You never need to share your broker password.</p>
    </dialog>
  );
}
