import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import { setFxRate, setMarketPrice, type CurrencyOption, type Holding } from "./api";

type Props = { open: boolean; onClose: () => void; holdings: Holding[]; currencies: CurrencyOption[]; reportingCurrency: string };

export function MarketDataDialog({ open, onClose, holdings, currencies, reportingCurrency }: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const queryClient = useQueryClient();
  const [instrument, setInstrument] = useState(holdings[0]?.instrument_id ?? "");
  const [priceCurrency, setPriceCurrency] = useState("GBP");
  const [price, setPrice] = useState("");
  const [fxBase, setFxBase] = useState("USD");
  const [rate, setRate] = useState("");
  useEffect(() => { const dialog = dialogRef.current; if (!dialog) return; if (open && !dialog.open) dialog.showModal(); if (!open && dialog.open) dialog.close(); }, [open]);
  const effectiveInstrument = instrument || holdings[0]?.instrument_id || "";
  const refresh = () => Promise.all([
    queryClient.invalidateQueries({ queryKey: ["valuation"] }),
    queryClient.invalidateQueries({ queryKey: ["allocation"] }),
  ]);
  const priceMutation = useMutation({ mutationFn: setMarketPrice, onSuccess: async () => { setPrice(""); await refresh(); } });
  const fxMutation = useMutation({ mutationFn: setFxRate, onSuccess: async () => { setRate(""); await refresh(); } });
  function submitPrice(event: FormEvent) { event.preventDefault(); priceMutation.mutate({ instrument_id: effectiveInstrument, price, currency: priceCurrency }); }
  function submitFx(event: FormEvent) { event.preventDefault(); fxMutation.mutate({ base_currency: fxBase, quote_currency: reportingCurrency, rate }); }
  return <dialog ref={dialogRef} className="market-dialog" onClose={onClose}><div className="dialog-topline"><div><span className="section-kicker">Exact manual inputs</span><h2>Prices &amp; FX</h2></div><button type="button" className="dialog-close" onClick={onClose} aria-label="Close market data">×</button></div><div className="market-columns"><form onSubmit={submitPrice}><h3>Instrument price</h3><p>Record the latest known price with explicit currency and timestamp.</p><label>Instrument<select value={effectiveInstrument} onChange={(event) => setInstrument(event.target.value)}>{holdings.map((holding) => <option key={`${holding.account_id}-${holding.instrument_id}`} value={holding.instrument_id}>{holding.instrument_id} · {holding.account_name}</option>)}</select></label><label>Price<input required inputMode="decimal" value={price} onChange={(event) => setPrice(event.target.value)} placeholder="0.00" /></label><label>Currency<select value={priceCurrency} onChange={(event) => setPriceCurrency(event.target.value)}>{currencies.map((currency) => <option key={currency.code} value={currency.code}>{currency.code}</option>)}</select></label><button className="primary-button" disabled={!effectiveInstrument || !price || priceMutation.isPending}>{priceMutation.isPending ? "Saving…" : "Save price"}</button>{priceMutation.isError && <small className="form-error">{String(priceMutation.error)}</small>}</form><form onSubmit={submitFx}><h3>FX conversion</h3><p>Enter how much one unit of the source currency is worth in {reportingCurrency}.</p><label>From<select value={fxBase} onChange={(event) => setFxBase(event.target.value)}>{currencies.filter((currency) => currency.code !== reportingCurrency).map((currency) => <option key={currency.code} value={currency.code}>{currency.code}</option>)}</select></label><label>To<input value={reportingCurrency} disabled /></label><label>Rate<input required inputMode="decimal" value={rate} onChange={(event) => setRate(event.target.value)} placeholder="0.0000" /></label><button className="primary-button" disabled={!rate || fxMutation.isPending}>{fxMutation.isPending ? "Saving…" : "Save FX rate"}</button>{fxMutation.isError && <small className="form-error">{String(fxMutation.error)}</small>}</form></div><p className="dialog-privacy"><span>●</span> Manual entries are labelled with source and timestamp. Missing data never becomes zero.</p></dialog>;
}
