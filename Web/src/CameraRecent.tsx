import { useEffect, useState } from "react";
import { X, Trash2 } from "lucide-react";
import { Modal } from "./Modal";
import { CameraPrice } from "./CameraPrice";
import { recentCards, removeRecentCard, storeRecentCard, type RecentCard } from "./cameraHistory";

export function CameraRecent({ close }: { close: () => void }) {
  const [rows, setRows] = useState<RecentCard[]>([]);
  const [error, setError] = useState("");
  const refresh = () => recentCards().then(setRows).catch(e => setError(String(e)));
  useEffect(() => { void refresh(); }, []);
  async function update(row: RecentCard) {
    try { await storeRecentCard(row); await refresh(); } catch (e) { setError(String(e)); }
  }
  return <Modal close={close}><section className="camera-review" role="dialog" aria-modal="true" aria-labelledby="recent-title">
    <header><h1 id="recent-title">Recent scans</h1><button className="camera-icon-button" aria-label="Close recent scans" onClick={close}><X /></button></header>
    <p>Saved on this device. Repeated frames count once; adjust quantities for additional copies.</p>
    {error && <p role="alert">{error}</p>}
    {!rows.length && <p>No recent cards yet. Match a card to start your list.</p>}
    <div className="camera-recent-list">{rows.map(row => <article key={row.id} className="camera-recent-row">
      <img src={`data:image/jpeg;base64,${row.base64}`} alt={row.selection.card.name} />
      <div><strong>{row.selection.card.name}</strong><small>{row.selection.card.setName} · {row.selection.card.localID}</small>
        <CameraPrice selection={row.selection} detail />
        <label>Quantity <input aria-label={`Quantity for ${row.selection.card.name}`} type="number" min="1" max="99" value={row.quantity} disabled={!!row.savedBatchID} onChange={e => { const quantity = Number(e.target.value); if (Number.isInteger(quantity) && quantity > 0 && quantity <= 99) void update({ ...row, quantity }); }} /></label>
        <small>{new Date(row.createdAt).toLocaleString()}</small>
      </div>
      <button className="camera-icon-button" aria-label={`Remove ${row.selection.card.name} from recent scans`} onClick={() => void removeRecentCard(row.id).then(refresh).catch(e => setError(String(e)))}><Trash2 size={18} /></button>
    </article>)}</div>
    <button className="button" onClick={close}>Back to camera</button>
  </section></Modal>;
}
