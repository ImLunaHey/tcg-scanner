import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { Modal } from "./Modal";
import { Api } from "./api";
import { CameraPrice, selectedCameraCard } from "./CameraPrice";
import { rememberCard } from "./cameraHistory";
import { detectorJPEG, pocketGrid, pocketJPEG, readBinderPhoto, type CardBox } from "./binderCapture";
import type { CameraResult } from "./Camera";
type Pocket = CardBox & { id: string; selected: boolean; state: "ready" | "identifying" | "matched" | "review" | "error"; result?: CameraResult; base64?: string; message?: string; added?: boolean };
const api = new Api("");
export function CameraBinder({ language, close, recent }: { language: string; close: () => void; recent: () => void }) {
  const [photo, setPhoto] = useState<HTMLCanvasElement>();
  const [preview, setPreview] = useState("");
  const [pockets, setPockets] = useState<Pocket[]>([]);
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("Take a clear photo with the whole page visible and as little glare as possible.");
  const [grid, setGrid] = useState("3x3");
  const [inset, setInset] = useState(0.03);
  const [manual, setManual] = useState(false);
  const version = useRef(0);
  const controller = useRef<AbortController | undefined>(undefined);
  useEffect(() => () => { version.current++; controller.current?.abort(); }, []);
  function boxes(values: CardBox[]) {
    setPockets(values.map(box => ({ ...box, id: crypto.randomUUID(), selected: true, state: "ready" })));
  }
  function gridBoxes(value = grid, padding = inset) {
    const [columns, rows] = value.split("x").map(Number);
    boxes(pocketGrid(columns, rows, padding));
    setManual(true);
    setMessage("Align the grid with the pockets. Deselect empty pockets before identifying.");
  }
  async function detect(canvas: HTMLCanvasElement) {
    const current = ++version.current;
    controller.current?.abort(); controller.current = new AbortController();
    setBusy(true); setMessage("Finding cards on the page…");
    try {
      const base64 = await detectorJPEG(canvas);
      const response = await api.response("camera/detect-page", { method: "POST", body: JSON.stringify({ base64, language }), signal: controller.current.signal }, false, 75_000);
      const result: { cards: CardBox[] } = await response.json();
      if (version.current !== current) return;
      boxes(result.cards); setManual(false);
      setMessage(result.cards.length ? `Found ${result.cards.length} card areas. Check the outlines, then identify the selected cards.` : "No card areas found. Try the pocket grid or take a clearer photo.");
    } catch (e) { if (version.current === current) setMessage(`${e instanceof Error ? e.message : e} You can use the pocket grid.`); }
    finally { if (version.current === current) setBusy(false); }
  }
  async function load(file?: File) {
    if (!file) return;
    setBusy(true); setPockets([]);
    try {
      const canvas = await readBinderPhoto(file);
      setPhoto(canvas); setPreview(canvas.toDataURL("image/jpeg", .8));
      await detect(canvas);
    } catch (e) { setMessage(String(e)); setBusy(false); }
  }
  function update(id: string, change: Partial<Pocket>) { setPockets(rows => rows.map(row => row.id === id ? { ...row, ...change } : row)); }
  async function identify(only?: string) {
    if (!photo || busy) return;
    const current = ++version.current;
    controller.current = new AbortController();
    setBusy(true);
    const selected = pockets.filter(p => p.selected && !p.added && (!only || p.id === only));
    for (const [index, pocket] of selected.entries()) {
      if (current !== version.current) break;
      setMessage(`Identifying ${index + 1} of ${selected.length} selected cards…`);
      update(pocket.id, { state: "identifying", message: undefined });
      try {
        const base64 = await pocketJPEG(photo, pocket);
        const response = await api.response("camera/identify", { method: "POST", body: JSON.stringify({ base64, language }), signal: controller.current.signal }, false, 75_000);
        const result: CameraResult = await response.json();
        if (current !== version.current) break;
        update(pocket.id, { base64, result, state: selectedCameraCard(result, language) ? "matched" : "review", message: result.candidates.length ? undefined : (pocket.width * photo.width < 450 ? "This pocket is small in the photo. Use an original-resolution photo, or scan this card separately." : "No exact match. Retry, adjust the grid, or scan this card separately.") });
      } catch (e) {
        if (current !== version.current) break;
        update(pocket.id, { state: "error", message: e instanceof Error ? e.message : String(e) });
      }
    }
    if (current === version.current) { setBusy(false); setMessage("Review the matches below. Unmatched pockets are kept for retry."); }
  }
  function cancel() {
    version.current++; controller.current?.abort(); setBusy(false);
    setPockets(rows => rows.map(r => r.state === "identifying" ? { ...r, state: "ready" } : r));
    setMessage("Stopped. Completed matches are still here. The current server lookup may take a moment to finish.");
  }
  async function add() {
    setBusy(true); setSaving(true);
    try {
      for (const pocket of pockets.filter(p => p.selected && p.state === "matched" && !p.added)) {
        const selection = pocket.result && selectedCameraCard(pocket.result, language);
        if (selection && pocket.base64) {
          await rememberCard(selection, pocket.base64, pocket.id);
          update(pocket.id, { added: true });
        }
      }
      setMessage("Matches added to Recent scans. Review quantities and prices there, then add to inventory.");
    } catch (e) { setMessage(String(e)); }
    finally { setBusy(false); setSaving(false); }
  }
  const matched = pockets.filter(p => p.selected && p.state === "matched" && !p.added).length;
  return <Modal close={saving ? () => {} : close}><section className="camera-review camera-binder" role="dialog" aria-modal="true" aria-labelledby="binder-title">
    <header><h1 id="binder-title">Binder page</h1><button className="camera-icon-button" aria-label="Close binder page" disabled={saving} onClick={close}><X /></button></header>
    <p role="status">{message}</p>
    <label className="button">Take or choose a page photo<input aria-label="Binder page photo" type="file" accept="image/*" capture="environment" disabled={busy} onChange={e => void load(e.target.files?.[0])} /></label>
    {photo && <>
      <div className="binder-controls">
        <button className="button" disabled={busy} onClick={() => void detect(photo)}>Find cards automatically</button>
        <label>Pocket grid <select aria-label="Pocket grid" disabled={busy} value={grid} onChange={e => { setGrid(e.target.value); gridBoxes(e.target.value); }}>
          {['2x2', '3x3', '3x4', '4x4'].map(size => <option key={size}>{size}</option>)}
        </select></label>
        <button className="button" disabled={busy} onClick={() => gridBoxes()}>Use pocket grid</button>
        {manual && <label>Page margin <input aria-label="Page margin" type="range" min="0" max="0.25" step="0.005" disabled={busy} value={inset} onChange={e => { const value = Number(e.target.value); setInset(value); gridBoxes(grid, value); }} /></label>}
      </div>
      <div className="binder-photo"><img src={preview} alt="Binder page to review" />
        {pockets.map((p, i) => <button key={p.id} aria-label={`Pocket ${i + 1}: ${p.state}${p.selected ? ', selected' : ', skipped'}`} disabled={busy || p.added} className="binder-box" data-state={p.state} data-selected={p.selected} style={{ left: `${p.x * 100}%`, top: `${p.y * 100}%`, width: `${p.width * 100}%`, height: `${p.height * 100}%` }} onClick={() => update(p.id, { selected: !p.selected })}><span>{i + 1} · {p.added ? "Added" : p.state === "matched" ? p.result?.candidates[0]?.name : p.selected ? p.state : "Skipped"}</span></button>)}
      </div>
      <div className="binder-controls">
        {busy ? <button className="button" disabled={saving} onClick={cancel}>{saving ? "Keeping matches…" : "Stop processing"}</button> : <button className="button primary" disabled={!pockets.some(p => p.selected && !p.added)} onClick={() => void identify()}>Identify selected cards</button>}
        <button className="button" disabled={busy || !matched} onClick={() => void add()}>Keep {matched} matches in Recent scans</button>
        <button className="button" disabled={busy} onClick={recent}>Review recent scans</button>
      </div>
      <div className="camera-recent-list">{pockets.map((p, i) => <article className="camera-recent-row" key={p.id}>
        {p.base64 && <img src={`data:image/jpeg;base64,${p.base64}`} alt={`Pocket ${i + 1}`} />}
        <div><strong>Pocket {i + 1} · {p.result?.candidates.length === 1 ? p.result.candidates[0].name : p.state}</strong>
          {p.message && <small>{p.message}</small>}
          {p.result && p.result.candidates.length > 1 && <label>Choose the printed set and number<select aria-label={`Match for pocket ${i + 1}`} disabled={busy || p.added} defaultValue="" onChange={e => { const card = p.result!.candidates.find(c => c.id === e.target.value); if (card) update(p.id, { state: "matched", result: { ...p.result!, state: "matched", candidates: [card] } }); }}><option value="">Choose a match…</option>{p.result.candidates.map(card => <option key={card.id} value={card.id}>{card.name} · {card.setName} · {card.localID}</option>)}</select></label>}
          {p.result?.candidates.length === 1 && <small>{p.result.candidates[0].setName} · {p.result.candidates[0].localID}</small>}
          {p.result && p.state === "matched" && <CameraPrice selection={selectedCameraCard(p.result, language)} />}
          {p.added ? <small>Added to Recent scans</small> : <label><input type="checkbox" checked={p.selected} disabled={busy} onChange={e => update(p.id, { selected: e.target.checked })} /> Include this pocket</label>}
        </div>
        <button className="button" disabled={busy || !p.selected || p.added} onClick={() => void identify(p.id)}>Retry</button>
      </article>)}</div>
    </>}
  </section></Modal>;
}
