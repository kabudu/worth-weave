import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useRef, useState } from "react";

export function AboutDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [version, setVersion] = useState("0.1.0");
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
    if (open) void getVersion().then(setVersion).catch(() => undefined);
  }, [open]);
  return <dialog className="about-dialog" ref={dialogRef} onClose={onClose}>
    <button type="button" className="dialog-close about-close" onClick={onClose} aria-label="Close About Worthweave">×</button>
    <div className="about-hero"><div className="about-mark" aria-hidden="true"><span /><span /><span /></div><span className="section-kicker">Worthweave for macOS</span><h2>Your investments,<br /><em>woven into one clear view.</em></h2><p>Bring your accounts together, understand your returns, and keep your financial data on your Mac.</p></div>
    <div className="about-details">
      <div className="about-facts"><span><strong>Version</strong>{version}</span><span><strong>Privacy</strong>Local-first</span><span><strong>Licence</strong>Apache 2.0</span></div>
      <section className="about-promise"><span aria-hidden="true">⌂</span><div><strong>Your portfolio stays yours</strong><p>Portfolio files, calculations, and private AI conversations remain on this Mac.</p></div></section>
      <div className="about-brokers"><span>Trading 212</span><span>Interactive Brokers</span><span>Robinhood</span></div>
      <p className="about-note">Independent open-source software. Worthweave does not provide financial advice and is not affiliated with the supported brokers.</p>
      <button type="button" className="primary-button about-done" onClick={onClose}>Done</button>
    </div>
  </dialog>;
}
