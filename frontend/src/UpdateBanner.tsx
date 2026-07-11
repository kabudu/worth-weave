import { isTauri } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useRef, useState } from "react";

type UpdateState = "available" | "downloading" | "installing" | "failed";

export function UpdateBanner() {
  const nativeApp = isTauri();
  const updateRef = useRef<Update | null>(null);
  const [update, setUpdate] = useState<Update | null>(null);
  const [state, setState] = useState<UpdateState>("available");
  const [downloaded, setDownloaded] = useState(0);
  const [total, setTotal] = useState<number | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!nativeApp) return;
    let cancelled = false;
    void check({ timeout: 10_000 })
      .then((availableUpdate) => {
        if (cancelled) {
          void availableUpdate?.close();
          return;
        }
        updateRef.current = availableUpdate;
        setUpdate(availableUpdate);
      })
      .catch(() => {
        // A failed background check should not interrupt normal app use.
      });
    return () => {
      cancelled = true;
      void updateRef.current?.close();
    };
  }, [nativeApp]);

  if (!nativeApp || !update) return null;
  const availableUpdate = update;

  function trackDownload(event: DownloadEvent) {
    if (event.event === "Started") {
      setTotal(event.data.contentLength ?? null);
      setDownloaded(0);
    } else if (event.event === "Progress") {
      setDownloaded((current) => current + event.data.chunkLength);
    } else if (event.event === "Finished") {
      setState("installing");
    }
  }

  async function install() {
    setState("downloading");
    setError("");
    try {
      await availableUpdate.downloadAndInstall(trackDownload, { timeout: 300_000 });
      setState("installing");
      await relaunch();
    } catch (cause) {
      setState("failed");
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  const percentage = total && total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
  return <section className="update-banner" aria-live="polite" aria-labelledby="update-title">
    <div className="update-icon" aria-hidden="true">↑</div>
    <div className="update-copy">
      <strong id="update-title">Worthweave {update.version} is ready</strong>
      <span>{state === "downloading" ? `Downloading${percentage === null ? "…" : ` · ${percentage}%`}` : state === "installing" ? "Installing, then Worthweave will restart…" : state === "failed" ? "The update couldn’t be installed." : "Install the update now, then Worthweave will restart."}</span>
      {state === "failed" && error && <small>{error}</small>}
    </div>
    <button type="button" onClick={() => void install()} disabled={state === "downloading" || state === "installing"}>
      {state === "failed" ? "Try again" : state === "available" ? "Update and restart" : "Updating…"}
    </button>
  </section>;
}
