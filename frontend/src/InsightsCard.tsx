import { useMutation } from "@tanstack/react-query";
import { Fragment, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import { explainPortfolio } from "./api";

function InlineMarkdown({ children }: { children: string }) {
  const parts = children.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).filter(Boolean);
  return <>{parts.map((part, index) => {
    if (part.startsWith("**") && part.endsWith("**")) return <strong key={index}>{part.slice(2, -2)}</strong>;
    if (part.startsWith("`") && part.endsWith("`")) return <code key={index}>{part.slice(1, -1)}</code>;
    return <Fragment key={index}>{part}</Fragment>;
  })}</>;
}

function StyledAnswer({ answer }: { answer: string }) {
  const blocks: ReactNode[] = [];
  const lines = answer.replace(/\r/g, "").split("\n");
  let paragraph: string[] = [];
  let bullets: string[] = [];
  const flushParagraph = () => {
    if (paragraph.length) blocks.push(<p key={`p-${blocks.length}`}><InlineMarkdown>{paragraph.join(" ")}</InlineMarkdown></p>);
    paragraph = [];
  };
  const flushBullets = () => {
    if (bullets.length) blocks.push(<ul key={`ul-${blocks.length}`}>{bullets.map((bullet, index) => <li key={index}><InlineMarkdown>{bullet}</InlineMarkdown></li>)}</ul>);
    bullets = [];
  };
  for (const rawLine of lines) {
    const line = rawLine.trim();
    const heading = line.match(/^#{1,3}\s+(.+)$/);
    const bullet = line.match(/^(?:[-*•]|\d+[.)])\s+(.+)$/);
    const emphasizedHeading = line.match(/^\*\*([^*]+)\*\*:?$/);
    if (!line) { flushParagraph(); flushBullets(); continue; }
    if (heading || emphasizedHeading) {
      flushParagraph(); flushBullets();
      const text = (heading?.[1] ?? emphasizedHeading?.[1] ?? "").replace(/\*\*/g, "");
      blocks.push(blocks.length === 0 ? <h2 key={`h-${blocks.length}`}>{text}</h2> : <h3 key={`h-${blocks.length}`}>{text}</h3>);
    } else if (bullet) {
      flushParagraph(); bullets.push(bullet[1] ?? "");
    } else {
      flushBullets(); paragraph.push(line);
    }
  }
  flushParagraph(); flushBullets();
  return <div className="ai-result-content">{blocks}</div>;
}

function AiResultDialog({ answer, model, open, onClose, onAskAnother }: { answer: string; model: string; open: boolean; onClose: () => void; onAskAnother: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);
  return <dialog ref={ref} className="ai-result-dialog" onClose={onClose}>
    <header><div><span className="section-kicker">Private AI · on this Mac</span><h2>Your portfolio answer</h2></div><button type="button" className="dialog-close" onClick={onClose} aria-label="Close portfolio answer">×</button></header>
    <StyledAnswer answer={answer} />
    <footer><small>{model} · based only on the figures available in Worthweave · not financial advice</small><div><button type="button" className="secondary-button" onClick={onAskAnother}>Ask another question</button><button type="button" className="primary-button" onClick={onClose}>Done</button></div></footer>
  </dialog>;
}

export function InsightsCard({ configured, onOpenSettings }: { configured: boolean; onOpenSettings: () => void }) {
  const [question, setQuestion] = useState("What changed in my portfolio, and why?");
  const [answerOpen, setAnswerOpen] = useState(false);
  const explanation = useMutation({ mutationFn: explainPortfolio, onSuccess: () => setAnswerOpen(true) });
  const explanationError = explanation.isError
    ? String(explanation.error).replace(/^local AI request failed:\s*/i, "")
    : "";
  function submit(event: FormEvent) { event.preventDefault(); if (configured && question.trim()) explanation.mutate(question); }
  return <article className={`insight-card${configured ? "" : " insight-card-disabled"}`} id="insights" data-availability={configured ? "ready" : "not-configured"}>
    <div className="insight-glow" />
    <div className="insight-title"><span>{configured ? "✦" : "⌁"}</span><div><small>Private AI</small><strong>Ask about your portfolio</strong></div>{!configured && <em className="insight-status">Not set up</em>}</div>
    {!configured && <div className="insight-disabled-note" role="status"><strong>Private AI is currently off</strong><span>Set it up to ask questions about your portfolio without sending your financial data away from this Mac.</span></div>}
    {explanation.data ? <div className="insight-answer-ready"><span>✓</span><div><strong>Your private answer is ready</strong><small>Presented as a clear, structured portfolio brief.</small></div><button type="button" onClick={() => setAnswerOpen(true)}>View answer</button></div> : <form onSubmit={submit}><div className="insight-controls" aria-disabled={!configured || explanation.isPending}><label htmlFor="portfolio-question">Your question</label><textarea id="portfolio-question" maxLength={500} value={question} onChange={(event) => setQuestion(event.target.value)} disabled={!configured || explanation.isPending} /><div className="prompt-row"><button type="button" disabled={!configured || explanation.isPending} onClick={() => setQuestion("Where is my portfolio concentrated?")}>Portfolio balance</button><button type="button" disabled={!configured || explanation.isPending} onClick={() => setQuestion("Summarise my recent investment income.")}>Recent income</button></div>{configured && <button className="ask-button" type="submit" disabled={explanation.isPending}>{explanation.isPending ? "Starting private AI…" : "Ask Worthweave"}<span>↗</span></button>}</div>{explanation.isPending && <div className="ai-working" role="status"><span aria-hidden="true"><i/><i/><i/></span><div><strong>Starting the private model on this Mac</strong><small>The first answer after opening Worthweave can take one or two minutes. Later answers will be faster.</small></div></div>}{!configured && <button className="ai-setup-button" type="button" aria-label="Set up private AI in Settings" onClick={onOpenSettings}><span><strong>Set up private AI</strong><small>Open Settings to get started</small></span><b aria-hidden="true">→</b></button>}</form>}
    {explanation.isError && <div className="ai-error" role="alert"><strong>Private AI couldn’t answer yet</strong><span>{explanationError}</span><button type="button" disabled={!question.trim()} onClick={() => explanation.mutate(question)}>Try again</button></div>}
    {explanation.data && <AiResultDialog answer={explanation.data.answer} model={explanation.data.model} open={answerOpen} onClose={() => setAnswerOpen(false)} onAskAnother={() => { setAnswerOpen(false); explanation.reset(); }} />}
  </article>;
}
