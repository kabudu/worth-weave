import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { getAiRecommendation, setupRecommendedAi, skipAiSetup } from "./api";

export function AiOnboarding() {
  const queryClient = useQueryClient();
  const recommendation = useQuery({ queryKey: ["ai-recommendation"], queryFn: ({ signal }) => getAiRecommendation(signal), staleTime: Infinity });
  const finish = async () => queryClient.invalidateQueries({ queryKey: ["settings"] });
  const setup = useMutation({ mutationFn: setupRecommendedAi, onSuccess: finish });
  const skip = useMutation({ mutationFn: skipAiSetup, onSuccess: finish });
  const choice = recommendation.data;
  const busy = setup.isPending || skip.isPending;

  return <main className="onboarding-shell ai-onboarding">
    <section className="onboarding-panel">
      <div className="onboarding-brand"><span className="onboarding-mark">W</span><strong>worthweave</strong></div>
      <span className="section-kicker">Welcome · Step 2 of 2</span>
      <h1>Clear answers,<br /><em>kept on your Mac.</em></h1>
      <p className="onboarding-intro">Worthweave can explain your portfolio and answer questions using private AI on this Mac. Your investments and questions never leave this device.</p>
      {recommendation.isPending && <div className="ai-choice">Finding the best option for this Mac…</div>}
      {recommendation.isError && <div className="form-error" role="alert">We couldn’t check this Mac: {String(recommendation.error)}</div>}
      {choice && <div className="ai-choice">
        <div className="ai-choice-heading"><span>✦</span><div><small>Recommended private AI</small><strong>{choice.runtime_name}</strong></div>{choice.installed && <em>Installed</em>}</div>
        <dl><div><dt>AI model</dt><dd>{choice.model}</dd></div><div><dt>Why we chose it</dt><dd>{choice.rationale}</dd></div></dl>
        <p>Setup uses the official installer and downloads files that may take several gigabytes of storage. Nothing is installed without your permission.</p>
      </div>}
      <div className="ai-actions">
        <button className="primary-button" type="button" disabled={!choice || !choice.supported || busy} onClick={() => setup.mutate()}>{setup.isPending ? "Setting up private AI…" : "Set up private AI"}<span>→</span></button>
        <button className="text-button" type="button" disabled={busy} onClick={() => skip.mutate()}>Continue without AI</button>
      </div>
      {(setup.isError || skip.isError) && <small className="form-error" role="alert">{String(setup.error ?? skip.error)}</small>}
      <div className="onboarding-trust"><span>●</span> Optional · private · change it later</div>
    </section>
    <aside className="onboarding-art ai-art" aria-hidden="true"><div className="ai-orbit"><span>✦</span><strong>W</strong></div><p>Your numbers.<br />Clearly explained.</p></aside>
  </main>;
}

export function AiSettingsPanel({ runtime, model }: { runtime: string | null; model: string | null }) {
  const queryClient = useQueryClient();
  const recommendation = useQuery({ queryKey: ["ai-recommendation"], queryFn: ({ signal }) => getAiRecommendation(signal), staleTime: Infinity });
  const setup = useMutation({ mutationFn: setupRecommendedAi, onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["settings"] }) });
  return <section className="ai-settings">
    <div><h3>Private AI</h3><p>{runtime && model ? `${runtime} · ${model}` : "AI answers are turned off."}</p></div>
    <div><p>{recommendation.data ? `Best option for this Mac: ${recommendation.data.runtime_name} with ${recommendation.data.model}.` : "Checking this Mac…"}</p><button className="secondary-button" type="button" disabled={!recommendation.data || setup.isPending} onClick={() => setup.mutate()}>{setup.isPending ? "Downloading and setting up…" : runtime ? "Set up again" : "Set up private AI"}</button>{setup.isPending && <small className="ai-setup-status" role="status">Keep Worthweave open. The model is several gigabytes and may take a while to download.</small>}{setup.isSuccess && <small className="ai-setup-success" role="status">Private AI is ready to use.</small>}{setup.isError && <small className="form-error" role="alert">Setup couldn’t finish: {String(setup.error)}</small>}</div>
  </section>;
}
