import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";

import { getMassiveProviderStatus, refreshFxRates, refreshMassivePrices, removeMassiveApiKey, saveMassiveApiKey, setFxRate, setMarketPrice, updateInstrumentMetadata, type CurrencyOption, type Holding } from "./api";

type Props = { open: boolean; onClose: () => void; holdings: Holding[]; currencies: CurrencyOption[]; reportingCurrency: string };

export function MarketDataDialog({ open, onClose, holdings, currencies, reportingCurrency }: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const queryClient = useQueryClient();
  const [instrument, setInstrument] = useState(holdings[0]?.instrument_id ?? "");
  const [priceCurrency, setPriceCurrency] = useState("GBP");
  const [price, setPrice] = useState("");
  const [fxBase, setFxBase] = useState("USD");
  const [rate, setRate] = useState("");
  const [massiveKey, setMassiveKey] = useState("");
  const [assetClass, setAssetClass] = useState<string | null>(null);
  const [sector, setSector] = useState<string | null>(null);
  const [geography, setGeography] = useState<string | null>(null);
  useEffect(() => { const dialog = dialogRef.current; if (!dialog) return; if (open && !dialog.open) dialog.showModal(); if (!open && dialog.open) dialog.close(); }, [open]);
  const effectiveInstrument = instrument || holdings[0]?.instrument_id || "";
  const selected = holdings.find((holding) => holding.instrument_id === effectiveInstrument);
  const effectiveAssetClass = assetClass ?? selected?.asset_class ?? "";
  const effectiveSector = sector ?? selected?.sector ?? "";
  const effectiveGeography = geography ?? selected?.geography ?? "";
  const refreshQueries = () => Promise.all([queryClient.invalidateQueries({ queryKey: ["valuation"] }), queryClient.invalidateQueries({ queryKey: ["allocation"] }), queryClient.invalidateQueries({ queryKey: ["total-return"] })]);
  const priceMutation = useMutation({ mutationFn: setMarketPrice, onSuccess: async () => { setPrice(""); await refreshQueries(); } });
  const fxMutation = useMutation({ mutationFn: setFxRate, onSuccess: async () => { setRate(""); await refreshQueries(); } });
  const referenceMutation = useMutation({ mutationFn: refreshFxRates, onSuccess: refreshQueries });
  const massiveStatus = useQuery({ queryKey: ["massive-status"], queryFn: getMassiveProviderStatus, enabled: open });
  const saveMassiveMutation = useMutation({ mutationFn: saveMassiveApiKey, onSuccess: async () => { setMassiveKey(""); await queryClient.invalidateQueries({ queryKey: ["massive-status"] }); } });
  const removeMassiveMutation = useMutation({ mutationFn: removeMassiveApiKey, onSuccess: () => queryClient.invalidateQueries({ queryKey: ["massive-status"] }) });
  const massiveRefreshMutation = useMutation({ mutationFn: refreshMassivePrices, onSuccess: refreshQueries });
  const metadataMutation = useMutation({ mutationFn: updateInstrumentMetadata, onSuccess: async () => { await Promise.all([queryClient.invalidateQueries({ queryKey: ["holdings"] }), refreshQueries()]); } });
  function submitPrice(event: FormEvent) { event.preventDefault(); priceMutation.mutate({ instrument_id: effectiveInstrument, price, currency: priceCurrency }); }
  function submitFx(event: FormEvent) { event.preventDefault(); fxMutation.mutate({ base_currency: fxBase, quote_currency: reportingCurrency, rate }); }
  function submitMetadata(event: FormEvent) { event.preventDefault(); metadataMutation.mutate({ instrument_id: effectiveInstrument, asset_class: effectiveAssetClass, sector: effectiveSector, geography: effectiveGeography }); }
  function selectInstrument(value: string) { setInstrument(value); setAssetClass(null); setSector(null); setGeography(null); }
  const instrumentSelect = <label>Investment<select value={effectiveInstrument} onChange={(event) => selectInstrument(event.target.value)}>{holdings.map((holding) => <option key={`${holding.account_id}-${holding.instrument_id}`} value={holding.instrument_id}>{holding.symbol ?? holding.instrument_id} · {holding.account_name}</option>)}</select></label>;
  return <dialog ref={dialogRef} className="market-dialog" onClose={onClose}>
    <div className="dialog-topline"><div><span className="section-kicker">Update your figures</span><h2>Prices, exchange rates &amp; categories</h2></div><button type="button" className="dialog-close" onClick={onClose} aria-label="Close market data">×</button></div>
    <section className="market-provider"><h3>Automatic US stock prices</h3><p>Optional Massive integration. Your API key is stored in macOS Keychain. Worthweave sends unresolved ticker symbols to Massive only when you click refresh; Stocks Starter quotes are 15-minute delayed.</p>
      {massiveStatus.data?.configured ? <div className="market-provider-actions"><button type="button" className="primary-button" disabled={massiveRefreshMutation.isPending} onClick={() => massiveRefreshMutation.mutate()}>{massiveRefreshMutation.isPending ? "Refreshing…" : "Refresh from Massive"}</button><button type="button" className="secondary-button" disabled={removeMassiveMutation.isPending} onClick={() => removeMassiveMutation.mutate()}>Remove API key</button></div> : <form onSubmit={(event) => { event.preventDefault(); saveMassiveMutation.mutate(massiveKey); }}><label>Massive API key<input type="password" autoComplete="off" required value={massiveKey} onChange={(event) => setMassiveKey(event.target.value)} /></label><button className="primary-button" disabled={saveMassiveMutation.isPending}>Save securely</button></form>}
      {massiveRefreshMutation.data && <small className="backup-success">Saved {massiveRefreshMutation.data.prices_saved} of {massiveRefreshMutation.data.requested} requested quotes. {massiveRefreshMutation.data.delisted.length > 0 && `Delisted US securities: ${massiveRefreshMutation.data.delisted.join(", ")}. `}{massiveRefreshMutation.data.foreign_inactive_matches.length > 0 && `Outside US coverage: ${massiveRefreshMutation.data.foreign_inactive_matches.join(", ")}. `}{massiveRefreshMutation.data.not_found.length > 0 && `Not found or outside US coverage: ${massiveRefreshMutation.data.not_found.join(", ")}. `}{massiveRefreshMutation.data.failed.length > 0 && `${massiveRefreshMutation.data.failed.length} could not be refreshed; try again shortly.`}</small>}
      {(massiveStatus.isError || saveMassiveMutation.isError || removeMassiveMutation.isError || massiveRefreshMutation.isError) && <small className="form-error">{String(massiveStatus.error ?? saveMassiveMutation.error ?? removeMassiveMutation.error ?? massiveRefreshMutation.error)}</small>}
    </section>
    <div className="market-columns">
      <form onSubmit={submitPrice}><h3>Investment price</h3><p>Broker position files supply prices when available. Add only the current prices still missing from your portfolio.</p>{instrumentSelect}<label>Price<input required inputMode="decimal" value={price} onChange={(event) => setPrice(event.target.value)} placeholder="0.00" /></label><label>Currency<select value={priceCurrency} onChange={(event) => setPriceCurrency(event.target.value)}>{currencies.map((currency) => <option key={currency.code} value={currency.code}>{currency.code}</option>)}</select></label><button className="primary-button" disabled={!effectiveInstrument || !price || priceMutation.isPending}>{priceMutation.isPending ? "Saving…" : "Save price"}</button>{priceMutation.isError && <small className="form-error">{String(priceMutation.error)}</small>}</form>
      <form onSubmit={submitFx}><h3>Exchange rates</h3><p>Worthweave uses the latest ECB reference rates automatically. Manual rates remain available as an override.</p><button className="secondary-button" type="button" disabled={referenceMutation.isPending} onClick={() => referenceMutation.mutate()}>{referenceMutation.isPending ? "Refreshing…" : "Refresh ECB rates"}</button>{referenceMutation.data && <small className="backup-success">Updated from {referenceMutation.data.source} · {referenceMutation.data.as_of.slice(0, 10)}</small>}{referenceMutation.isError && <small className="form-error">{String(referenceMutation.error)}</small>}<label>Manual rate from<select value={fxBase} onChange={(event) => setFxBase(event.target.value)}>{currencies.filter((currency) => currency.code !== reportingCurrency).map((currency) => <option key={currency.code} value={currency.code}>{currency.code}</option>)}</select></label><label>To<input value={reportingCurrency} disabled /></label><label>Value of 1 {fxBase}<input required inputMode="decimal" value={rate} onChange={(event) => setRate(event.target.value)} placeholder="0.0000" /></label><button className="primary-button" disabled={!rate || fxMutation.isPending}>{fxMutation.isPending ? "Saving…" : "Save manual rate"}</button>{fxMutation.isError && <small className="form-error">{String(fxMutation.error)}</small>}</form>
      <form onSubmit={submitMetadata}><h3>Investment categories</h3><p>Add details that help group your investments.</p>{instrumentSelect}<label>Asset class<input maxLength={80} value={effectiveAssetClass} onChange={(event) => setAssetClass(event.target.value)} placeholder="Equity" /></label><label>Sector<input maxLength={80} value={effectiveSector} onChange={(event) => setSector(event.target.value)} placeholder="Technology" /></label><label>Country or region<input maxLength={80} value={effectiveGeography} onChange={(event) => setGeography(event.target.value)} placeholder="United Kingdom" /></label><button className="primary-button" disabled={!effectiveInstrument || metadataMutation.isPending}>{metadataMutation.isPending ? "Saving…" : "Save categories"}</button>{metadataMutation.isError && <small className="form-error">{String(metadataMutation.error)}</small>}</form>
    </div>
    <p className="dialog-privacy"><span>●</span> These details stay on this Mac. Missing categories are shown as “Not categorised”.</p>
  </dialog>;
}
