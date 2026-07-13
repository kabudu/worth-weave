import { useMutation } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";

import { explainPortfolio } from "./api";

export function InsightsCard({ configured, onOpenSettings }: { configured: boolean; onOpenSettings: () => void }) {
  const [question, setQuestion] = useState("What changed in my portfolio, and why?");
  const explanation = useMutation({ mutationFn: explainPortfolio });
  const explanationError = explanation.isError
    ? String(explanation.error).replace(/^local AI request failed:\s*/i, "")
    : "";
  function submit(event: FormEvent) { event.preventDefault(); if (configured && question.trim()) explanation.mutate(question); }
  return <article className={`insight-card${configured ? "" : " insight-card-disabled"}`} id="insights" data-availability={configured ? "ready" : "not-configured"}>
    <div className="insight-glow" />
    <div className="insight-title"><span>{configured ? "✦" : "⌁"}</span><div><small>Private AI</small><strong>Ask about your portfolio</strong></div>{!configured && <em className="insight-status">Not set up</em>}</div>
    {!configured && <div className="insight-disabled-note" role="status"><strong>Private AI is currently off</strong><span>Set it up to ask questions about your portfolio without sending your financial data away from this Mac.</span></div>}
    {explanation.data ? <div className="insight-answer"><p>{explanation.data.answer}</p><small>{explanation.data.model} · answered on this Mac · not financial advice</small></div> : <form onSubmit={submit}><div className="insight-controls" aria-disabled={!configured || explanation.isPending}><label htmlFor="portfolio-question">Your question</label><textarea id="portfolio-question" maxLength={500} value={question} onChange={(event) => setQuestion(event.target.value)} disabled={!configured || explanation.isPending} /><div className="prompt-row"><button type="button" disabled={!configured || explanation.isPending} onClick={() => setQuestion("Where is my portfolio concentrated?")}>Portfolio balance</button><button type="button" disabled={!configured || explanation.isPending} onClick={() => setQuestion("Summarise my recent investment income.")}>Recent income</button></div>{configured && <button className="ask-button" type="submit" disabled={explanation.isPending}>{explanation.isPending ? "Starting private AI…" : "Ask Worthweave"}<span>↗</span></button>}</div>{explanation.isPending && <div className="ai-working" role="status"><span aria-hidden="true"><i/><i/><i/></span><div><strong>Starting the private model on this Mac</strong><small>The first answer after opening Worthweave can take one or two minutes. Later answers will be faster.</small></div></div>}{!configured && <button className="ai-setup-button" type="button" aria-label="Set up private AI in Settings" onClick={onOpenSettings}><span><strong>Set up private AI</strong><small>Open Settings to get started</small></span><b aria-hidden="true">→</b></button>}</form>}
    {explanation.isError && <div className="ai-error" role="alert"><strong>Private AI couldn’t answer yet</strong><span>{explanationError}</span><button type="button" disabled={!question.trim()} onClick={() => explanation.mutate(question)}>Try again</button></div>}
  </article>;
}
