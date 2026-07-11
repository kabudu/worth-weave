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
      <h1>Private insight,<br /><em>tuned to this Mac.</em></h1>
      <p className="onboarding-intro">Worthweave can explain deterministic portfolio analytics with a model that runs locally. Your holdings and questions stay on this device.</p>
      {recommendation.isPending && <div className="ai-choice">Inspecting this device…</div>}
      {recommendation.isError && <div className="form-error" role="alert">Could not inspect this device: {String(recommendation.error)}</div>}
      {choice && <div className="ai-choice">
        <div className="ai-choice-heading"><span>✦</span><div><small>Recommended runtime</small><strong>{choice.runtime_name}</strong></div>{choice.installed && <em>Installed</em>}</div>
        <dl><div><dt>Model</dt><dd>{choice.model}</dd></div><div><dt>Why this fit</dt><dd>{choice.rationale}</dd></div></dl>
        <p>Setup uses the runtime’s official package tooling and downloads model files that may use several gigabytes. Nothing is installed until you choose setup.</p>
      </div>}
      <div className="ai-actions">
        <button className="primary-button" type="button" disabled={!choice || !choice.supported || busy} onClick={() => setup.mutate()}>{setup.isPending ? "Setting up local AI…" : "Set up recommended AI"}<span>→</span></button>
        <button className="text-button" type="button" disabled={busy} onClick={() => skip.mutate()}>Continue without AI</button>
      </div>
      {(setup.isError || skip.isError) && <small className="form-error" role="alert">{String(setup.error ?? skip.error)}</small>}
      <div className="onboarding-trust"><span>●</span> Optional · local · changeable later</div>
    </section>
    <aside className="onboarding-art ai-art" aria-hidden="true"><div className="ai-orbit"><span>✦</span><strong>W</strong></div><p>Your numbers.<br />Clearly explained.</p></aside>
  </main>;
}

export function AiSettingsPanel({ runtime, model }: { runtime: string | null; model: string | null }) {
  const queryClient = useQueryClient();
  const recommendation = useQuery({ queryKey: ["ai-recommendation"], queryFn: ({ signal }) => getAiRecommendation(signal), staleTime: Infinity });
  const setup = useMutation({ mutationFn: setupRecommendedAi, onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["settings"] }) });
  return <section className="ai-settings">
    <div><h3>Local AI</h3><p>{runtime && model ? `${runtime} · ${model}` : "Local explanations are currently disabled."}</p></div>
    <div><p>{recommendation.data ? `Recommended for this device: ${recommendation.data.runtime_name} with ${recommendation.data.model}.` : "Checking this device…"}</p><button className="secondary-button" type="button" disabled={!recommendation.data || setup.isPending} onClick={() => setup.mutate()}>{setup.isPending ? "Setting up…" : runtime ? "Reinstall recommendation" : "Set up local AI"}</button>{setup.isError && <small className="form-error">{String(setup.error)}</small>}</div>
  </section>;
}
