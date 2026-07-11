import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";

import { setFxRate, setMarketPrice, updateInstrumentMetadata, type CurrencyOption, type Holding } from "./api";

type Props = { open: boolean; onClose: () => void; holdings: Holding[]; currencies: CurrencyOption[]; reportingCurrency: string };

export function MarketDataDialog({ open, onClose, holdings, currencies, reportingCurrency }: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const queryClient = useQueryClient();
  const [instrument, setInstrument] = useState(holdings[0]?.instrument_id ?? "");
  const [priceCurrency, setPriceCurrency] = useState("GBP");
  const [price, setPrice] = useState("");
  const [fxBase, setFxBase] = useState("USD");
  const [rate, setRate] = useState("");
  const [assetClass, setAssetClass] = useState<string | null>(null);
  const [sector, setSector] = useState<string | null>(null);
  const [geography, setGeography] = useState<string | null>(null);
  useEffect(() => { const dialog = dialogRef.current; if (!dialog) return; if (open && !dialog.open) dialog.showModal(); if (!open && dialog.open) dialog.close(); }, [open]);
  const effectiveInstrument = instrument || holdings[0]?.instrument_id || "";
  const selected = holdings.find((holding) => holding.instrument_id === effectiveInstrument);
  const effectiveAssetClass = assetClass ?? selected?.asset_class ?? "";
  const effectiveSector = sector ?? selected?.sector ?? "";
  const effectiveGeography = geography ?? selected?.geography ?? "";
  const refresh = () => Promise.all([queryClient.invalidateQueries({ queryKey: ["valuation"] }), queryClient.invalidateQueries({ queryKey: ["allocation"] })]);
  const priceMutation = useMutation({ mutationFn: setMarketPrice, onSuccess: async () => { setPrice(""); await refresh(); } });
  const fxMutation = useMutation({ mutationFn: setFxRate, onSuccess: async () => { setRate(""); await refresh(); } });
  const metadataMutation = useMutation({ mutationFn: updateInstrumentMetadata, onSuccess: async () => { await Promise.all([queryClient.invalidateQueries({ queryKey: ["holdings"] }), refresh()]); } });
  function submitPrice(event: FormEvent) { event.preventDefault(); priceMutation.mutate({ instrument_id: effectiveInstrument, price, currency: priceCurrency }); }
  function submitFx(event: FormEvent) { event.preventDefault(); fxMutation.mutate({ base_currency: fxBase, quote_currency: reportingCurrency, rate }); }
  function submitMetadata(event: FormEvent) { event.preventDefault(); metadataMutation.mutate({ instrument_id: effectiveInstrument, asset_class: effectiveAssetClass, sector: effectiveSector, geography: effectiveGeography }); }
  function selectInstrument(value: string) { setInstrument(value); setAssetClass(null); setSector(null); setGeography(null); }
  const instrumentSelect = <label>Investment<select value={effectiveInstrument} onChange={(event) => selectInstrument(event.target.value)}>{holdings.map((holding) => <option key={`${holding.account_id}-${holding.instrument_id}`} value={holding.instrument_id}>{holding.symbol ?? holding.instrument_id} · {holding.account_name}</option>)}</select></label>;
  return <dialog ref={dialogRef} className="market-dialog" onClose={onClose}>
    <div className="dialog-topline"><div><span className="section-kicker">Update your figures</span><h2>Prices, exchange rates &amp; categories</h2></div><button type="button" className="dialog-close" onClick={onClose} aria-label="Close market data">×</button></div>
    <div className="market-columns">
      <form onSubmit={submitPrice}><h3>Investment price</h3><p>Record the latest known price and its currency.</p>{instrumentSelect}<label>Price<input required inputMode="decimal" value={price} onChange={(event) => setPrice(event.target.value)} placeholder="0.00" /></label><label>Currency<select value={priceCurrency} onChange={(event) => setPriceCurrency(event.target.value)}>{currencies.map((currency) => <option key={currency.code} value={currency.code}>{currency.code}</option>)}</select></label><button className="primary-button" disabled={!effectiveInstrument || !price || priceMutation.isPending}>{priceMutation.isPending ? "Saving…" : "Save price"}</button>{priceMutation.isError && <small className="form-error">{String(priceMutation.error)}</small>}</form>
      <form onSubmit={submitFx}><h3>Exchange rate</h3><p>Enter the value of 1 {fxBase} in {reportingCurrency}.</p><label>From<select value={fxBase} onChange={(event) => setFxBase(event.target.value)}>{currencies.filter((currency) => currency.code !== reportingCurrency).map((currency) => <option key={currency.code} value={currency.code}>{currency.code}</option>)}</select></label><label>To<input value={reportingCurrency} disabled /></label><label>Rate<input required inputMode="decimal" value={rate} onChange={(event) => setRate(event.target.value)} placeholder="0.0000" /></label><button className="primary-button" disabled={!rate || fxMutation.isPending}>{fxMutation.isPending ? "Saving…" : "Save exchange rate"}</button>{fxMutation.isError && <small className="form-error">{String(fxMutation.error)}</small>}</form>
      <form onSubmit={submitMetadata}><h3>Investment categories</h3><p>Add details that help group your investments.</p>{instrumentSelect}<label>Asset class<input maxLength={80} value={effectiveAssetClass} onChange={(event) => setAssetClass(event.target.value)} placeholder="Equity" /></label><label>Sector<input maxLength={80} value={effectiveSector} onChange={(event) => setSector(event.target.value)} placeholder="Technology" /></label><label>Country or region<input maxLength={80} value={effectiveGeography} onChange={(event) => setGeography(event.target.value)} placeholder="United Kingdom" /></label><button className="primary-button" disabled={!effectiveInstrument || metadataMutation.isPending}>{metadataMutation.isPending ? "Saving…" : "Save categories"}</button>{metadataMutation.isError && <small className="form-error">{String(metadataMutation.error)}</small>}</form>
    </div>
    <p className="dialog-privacy"><span>●</span> These details stay on this Mac. Missing categories are shown as “Not categorised”.</p>
  </dialog>;
}
