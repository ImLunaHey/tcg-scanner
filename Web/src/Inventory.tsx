import { useEffect, useState, type CSSProperties } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { Link } from "wouter";
import { ImageIcon } from "lucide-react";
import { useAuth } from "./auth";
import { batchLabel, captureLabel, gameLabel, type CardGameDetection } from "./models";
import { Empty, ErrorNote, Loading } from "./ui";
import { InventoryThumbnail, type InventoryPreview } from "./InventoryThumbnail";
import { ImageLightbox } from "./ImageLightbox";

export type InventoryEntry = {
  id: string; game: CardGameDetection["game"]; language: string; name: string;
  setName?: string; setKey: string; number?: string; status: "catalogue" | "awaitingCatalogue" | "unresolved";
  quantity: number; confirmedQuantity: number; preview?: InventoryPreview;
  copies: { batchID: string; batchNumber?: number; cardNumber: number }[];
};
export type InventorySet = { id: string; name: string; game: CardGameDetection["game"]; quantity: number; entries: number; previews: InventoryPreview[] };
type InventoryPage = {
  entries: InventoryEntry[]; sets: InventorySet[]; totalCopies: number; catalogueCopies: number;
  awaitingCatalogueCopies: number; unresolvedCopies: number; excludedCodeCards: number;
  matchingEntries: number; nextOffset?: number;
};
export function InventoryCard({ entry, open }: { entry: InventoryEntry; open?: () => void }) {
  const status = entry.status === "awaitingCatalogue" ? "Awaiting catalogue data"
    : entry.status === "unresolved" ? "Needs identification"
    : entry.confirmedQuantity === entry.quantity ? "Confirmed" : "Suggested match";
  return <article className="inventory-card">
    <button className="inventory-card-image" onClick={open} disabled={!entry.preview} aria-label={`Enlarge ${entry.name}`}>
      {entry.preview ? <InventoryThumbnail preview={entry.preview} /> : <ImageIcon size={36} aria-hidden="true" />}
      <span className="inventory-copy-badge" aria-label={`${entry.quantity} ${entry.quantity === 1 ? "copy" : "copies"}`}>×{entry.quantity.toLocaleString()}</span>
    </button>
    <div className="inventory-card-caption">
      <h2 title={entry.name}>{entry.name}</h2>
      <p className="inventory-set-name" title={entry.setName}>{entry.setName || "Unidentified set"}</p>
      <div className="inventory-card-foot"><span>{entry.number || gameLabel(entry.game)}</span><span title={status} aria-label={status} className={`inventory-identity-dot ${entry.status === "catalogue" ? "matched" : "pending"}`} /><span className="inventory-language" title={entry.language === "en" ? "English" : "Korean"}>{entry.language.toUpperCase()}</span></div>
      <details className="inventory-locations">
        <summary>Locations</summary>
        <div>{entry.copies.map(copy => <Link key={`${copy.batchID}:${copy.cardNumber}`} href={`/batches/${copy.batchID}?card=${copy.cardNumber}`}>
          {batchLabel({ id: copy.batchID, batchNumber: copy.batchNumber })} / {captureLabel(copy.cardNumber)}
        </Link>)}</div>
      </details>
    </div>
  </article>;
}
export function InventorySetTile({ set, select }: { set: InventorySet; select: () => void }) {
  return <button className="inventory-set-tile" onClick={select} aria-label={`View ${set.name}`}>
    <span className="inventory-set-covers">{set.previews.length ? set.previews.map((preview, index) => <span key={index} style={{ "--cover-ratio": preview.image.width / preview.image.height } as CSSProperties}><InventoryThumbnail preview={preview} /></span>) : <span className="inventory-set-placeholder" style={{ "--cover-ratio": 63 / 88 } as CSSProperties}><ImageIcon size={38} aria-hidden="true" /></span>}</span>
    <strong>{set.name}</strong><span>{gameLabel(set.game)} · {set.quantity.toLocaleString()} copies</span>
  </button>;
}
export function Inventory() {
  const { api } = useAuth();
  const [text, setText] = useState("");
  const [search, setSearch] = useState("");
  const [offset, setOffset] = useState(0);
  const [view, setView] = useState<"cards" | "sets">("cards");
  const [selectedSet, setSelectedSet] = useState("");
  const [lightbox, setLightbox] = useState<InventoryEntry>();
  useEffect(() => {
    const timer = setTimeout(() => { setSearch(text.trim()); setOffset(0); }, 250);
    return () => clearTimeout(timer);
  }, [text]);
  const inventory = useQuery({
    queryKey: ["inventory", search, offset, selectedSet],
    queryFn: ({ signal }) => api.get<InventoryPage>(`inventory?limit=48&offset=${offset}&search=${encodeURIComponent(search)}&set=${encodeURIComponent(selectedSet)}`, signal),
    placeholderData: keepPreviousData,
    refetchInterval: 30_000,
  });
  const data = inventory.data;
  const sets = data?.sets.filter(set => `${set.name} ${gameLabel(set.game)}`.toLowerCase().includes(search.toLowerCase())) || [];
  function chooseSet(id: string) { setSelectedSet(id); setOffset(0); setText(""); setSearch(""); setView("cards"); }
  return <>
    <header className="inventory-heading"><h1>Inventory</h1>{data && <span>{data.totalCopies.toLocaleString()} scanned copies <span title="Rescans count again; all finishes are combined.">ⓘ</span></span>}</header>
    <div className="inventory-gallery-toolbar">
      <div className="inventory-view-switch" role="group" aria-label="Inventory view"><button aria-pressed={view === "cards"} onClick={() => setView("cards")}>Cards</button><button aria-pressed={view === "sets"} onClick={() => { setView("sets"); setSelectedSet(""); setOffset(0); }}>Sets</button></div>
      <label className="field inventory-search"><span className="sr-only">Search {view}</span><input type="search" maxLength={100} value={text} onChange={e => setText(e.target.value)} placeholder={view === "sets" ? "Find a set…" : "Find a card…"} /></label>
      {view === "cards" && <label className="field inventory-set-filter"><span className="sr-only">Set</span><select value={selectedSet} onChange={e => { setSelectedSet(e.target.value); setOffset(0); }}><option value="">All sets</option>{data?.sets.map(set => <option key={set.id} value={set.id}>{set.name} ({set.quantity})</option>)}</select></label>}
    </div>
    <ErrorNote error={inventory.error} />
    {inventory.isPending ? <Loading /> : view === "sets" ? sets.length ? <div className="inventory-set-grid">{sets.map(set => <InventorySetTile key={set.id} set={set} select={() => chooseSet(set.id)} />)}</div> : <Empty title="No matching sets">Try another set name.</Empty>
      : data && !data.entries.length ? <Empty title={search || selectedSet ? "No matching cards" : "No scanned cards yet"}>{search || selectedSet ? "Try another name or set." : "Completed card batches will appear here."}</Empty>
      : <div className="inventory-card-grid">{data?.entries.map(entry => <InventoryCard key={entry.id} entry={entry} open={() => setLightbox(entry)} />)}</div>}
    {view === "cards" && data && (offset > 0 || data.nextOffset != null) && <nav className="inventory-pagination" aria-label="Inventory pages"><button className="button secondary" disabled={offset === 0 || inventory.isFetching} onClick={() => setOffset(Math.max(0, offset - 48))}>Previous</button><span className="small">{offset + 1}–{offset + data.entries.length} of {data.matchingEntries}</span><button className="button secondary" disabled={data.nextOffset == null || inventory.isFetching} onClick={() => setOffset(data.nextOffset!)}>Next</button></nav>}
    {lightbox?.preview && <ImageLightbox batchID={lightbox.preview.batchID} images={[{ ...lightbox.preview.image, editID: lightbox.preview.editID, label: lightbox.name }]} initial={0} close={() => setLightbox(undefined)} />}
  </>;
}
