import { useEffect, useState } from "react";
import { X, Trash2 } from "lucide-react";
import { Link } from "wouter";
import { useAuth } from "./auth";
import { useQueryClient } from "@tanstack/react-query";
import { Modal } from "./Modal";
import { CameraPrice } from "./CameraPrice";
import { recentCards, removeRecentCard, storeRecentCard, type RecentCard } from "./cameraHistory";

export function CameraRecent({ close }: { close: () => void }) {
  const [rows, setRows] = useState<RecentCard[]>([]);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [excluded, setExcluded] = useState<Set<string>>(new Set());
  const { api, session } = useAuth();
  const queries = useQueryClient();
  const pending = rows.filter(r => !r.savedBatchID && !excluded.has(r.id));
  async function save() {
    setSaving(true); setError("");
    try {
      for (const row of pending) {
        const locked = { ...row, importStarted: true };
        await storeRecentCard(locked);
        await refresh();
        const response = await api.response("camera/import", { method: "POST", body: JSON.stringify({ id: row.id, selection: row.selection, base64: row.base64, quantity: row.quantity }) }, true, 120_000);
        const batch = await response.json();
        await storeRecentCard({ ...locked, savedBatchID: batch.id });
        await refresh();
      }
      await queries.invalidateQueries({ queryKey: ["inventory"] });
      await queries.invalidateQueries({ queryKey: ["batches"] });
    } catch (e) { setError(`${e instanceof Error ? e.message : String(e)} Retry uses the same capture IDs and will not add duplicates.`); }
    finally { setSaving(false); }
  }
  const refresh = () => recentCards().then(setRows).catch(e => setError(String(e)));
  useEffect(() => { void refresh(); }, []);
  async function update(row: RecentCard) {
    try { await storeRecentCard(row); await refresh(); } catch (e) { setError(String(e)); }
  }
  return <Modal close={saving ? () => {} : close}><section className="camera-review" role="dialog" aria-modal="true" aria-labelledby="recent-title">
    <header><h1 id="recent-title">Recent scans</h1><button className="camera-icon-button" aria-label="Close recent scans" disabled={saving} onClick={close}><X /></button></header>
    <p>Saved on this device. Repeated frames count once; adjust quantities for additional copies.</p>
    {error && <p role="alert">{error}</p>}
    {!rows.length && <p>No recent cards yet. Match a card to start your list.</p>}
    <div className="camera-recent-list">{rows.map(row => <article key={row.id} className="camera-recent-row">
      {!row.savedBatchID && <input type="checkbox" aria-label={`Select ${row.selection.card.name}`} checked={!excluded.has(row.id)} disabled={saving} onChange={e => setExcluded(previous => { const next = new Set(previous); if (e.target.checked) next.delete(row.id); else next.add(row.id); return next; })} />}
      <img src={`data:image/jpeg;base64,${row.base64}`} alt={row.selection.card.name} />
      <div><strong>{row.selection.card.name}</strong><small>{row.selection.card.setName} · {row.selection.card.localID}</small>
        <CameraPrice selection={row.selection} detail />
        <label>Quantity <input aria-label={`Quantity for ${row.selection.card.name}`} type="number" min="1" max="99" value={row.quantity} disabled={!!row.savedBatchID || row.importStarted || saving} onChange={e => { const quantity = Number(e.target.value); if (Number.isInteger(quantity) && quantity > 0 && quantity <= 99) void update({ ...row, quantity }); }} /></label>
        <label>Finish <select aria-label={`Finish for ${row.selection.card.name}`} value={row.selection.finish} disabled={!!row.savedBatchID || row.importStarted || saving} onChange={e => void update({ ...row, selection: { ...row.selection, finish: e.target.value } })}>
          <option value="unverified">Not checked</option>
          {(row.selection.card.availableFinishes || []).filter(f => f !== "unverified").map(f => <option key={f} value={f}>{f === "reverseHolo" ? "Reverse holo" : f === "normal" ? "Non-holo" : f}</option>)}
        </select></label>
        {row.savedBatchID && <Link href={`/batches/${row.savedBatchID}`}>Saved to inventory · view batch</Link>}
        {row.importStarted && !row.savedBatchID && <small>Save pending · retry to confirm</small>}
        <small>{new Date(row.createdAt).toLocaleString()}</small>
      </div>
      <button className="camera-icon-button" aria-label={`Remove ${row.selection.card.name} from recent scans`} disabled={saving || (row.importStarted && !row.savedBatchID)} onClick={() => void removeRecentCard(row.id).then(refresh).catch(e => setError(String(e)))}><Trash2 size={18} /></button>
    </article>)}</div>
    <p>Check the set, number, finish and quantity before adding. Condition remains unassessed.</p>
    {session ? <button className="button primary" disabled={saving || !pending.length} onClick={() => void save()}>{saving ? "Saving…" : `Add ${pending.reduce((n, row) => n + row.quantity, 0)} cards to inventory`}</button> : <Link className="button primary" href="/">Connect to scanner to add to inventory</Link>}
    <button className="button" disabled={saving} onClick={close}>Back to camera</button>
  </section></Modal>;
}
