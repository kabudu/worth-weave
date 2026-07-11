import { useMutation } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";

import { explainPortfolio } from "./api";

export function InsightsCard({ configured }: { configured: boolean }) {
  const [question, setQuestion] = useState("What changed in my portfolio, and why?");
  const explanation = useMutation({ mutationFn: explainPortfolio });
  function submit(event: FormEvent) { event.preventDefault(); if (configured && question.trim()) explanation.mutate(question); }
  return <article className="insight-card" id="insights">
    <div className="insight-glow" />
    <div className="insight-title"><span>✦</span><div><small>Private AI</small><strong>Ask about your portfolio</strong></div></div>
    {explanation.data ? <div className="insight-answer"><p>{explanation.data.answer}</p><small>{explanation.data.model} · answered on this Mac · not financial advice</small></div> : <form onSubmit={submit}><label htmlFor="portfolio-question">Your question</label><textarea id="portfolio-question" maxLength={500} value={question} onChange={(event) => setQuestion(event.target.value)} disabled={!configured || explanation.isPending} /><div className="prompt-row"><button type="button" onClick={() => setQuestion("Where is my portfolio concentrated?")}>Portfolio balance</button><button type="button" onClick={() => setQuestion("Summarise my recent investment income.")}>Recent income</button></div><button className="ask-button" type="submit" disabled={!configured || explanation.isPending}>{!configured ? "Set up private AI in Settings" : explanation.isPending ? "Working on your answer…" : "Ask Worthweave"}<span>↗</span></button></form>}
    {explanation.isError && <small className="form-error" role="alert">{String(explanation.error)}</small>}
  </article>;
}
