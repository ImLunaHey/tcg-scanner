import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link } from "wouter";
import { useAuth } from "./auth";
import { batchLabel, captureLabel, gameLabel, type CardGameDetection } from "./models";
import { Empty, ErrorNote, Loading } from "./ui";

export type InventoryEntry = {
  id: string; game: CardGameDetection["game"]; language: string; name: string;
  setName?: string; number?: string; status: "catalogue" | "awaitingCatalogue" | "unresolved";
  quantity: number; confirmedQuantity: number;
  copies: { batchID: string; batchNumber?: number; cardNumber: number }[];
};
type InventoryPage = {
  entries: InventoryEntry[]; totalCopies: number; catalogueCopies: number;
  awaitingCatalogueCopies: number; unresolvedCopies: number; excludedCodeCards: number;
  matchingEntries: number; nextOffset?: number;
};
export function InventoryRow({ entry }: { entry: InventoryEntry }) {
  return <article className="inventory-row">
    <div className="inventory-identity">
      <h2>{entry.name}</h2>
      <p>{[entry.setName, entry.number].filter(Boolean).join(" · ") || "Check the saved batch"}</p>
      <div className="inventory-meta">
        <span>{gameLabel(entry.game)} · {entry.language === "en" ? "English" : entry.language === "ko" ? "Korean" : entry.language}</span>
        {entry.status === "awaitingCatalogue" ? <span className="finish-pill finish-uncertain">Awaiting catalogue data</span>
          : entry.status === "unresolved" ? <span className="finish-pill finish-uncertain">Needs review</span>
          : <span>{entry.confirmedQuantity === entry.quantity ? "Confirmed" : "Includes suggested matches"}</span>}
      </div>
      <details className="inventory-locations">
        <summary>Batch locations</summary>
        <div>{entry.copies.map(copy => <Link key={`${copy.batchID}:${copy.cardNumber}`} href={`/batches/${copy.batchID}?card=${copy.cardNumber}`}>
          {batchLabel({ id: copy.batchID, batchNumber: copy.batchNumber })} / {captureLabel(copy.cardNumber)}
        </Link>)}</div>
      </details>
    </div>
    <div className="inventory-quantity"><strong>{entry.quantity.toLocaleString()}</strong><span>{entry.quantity === 1 ? "copy" : "copies"}</span></div>
  </article>;
}
export function Inventory() {
  const { api } = useAuth();
  const [text, setText] = useState("");
  const [search, setSearch] = useState("");
  const [offset, setOffset] = useState(0);
  useEffect(() => {
    const timer = setTimeout(() => { setSearch(text.trim()); setOffset(0); }, 250);
    return () => clearTimeout(timer);
  }, [text]);
  const inventory = useQuery({
    queryKey: ["inventory", search, offset],
    queryFn: ({ signal }) => api.get<InventoryPage>(`inventory?limit=50&offset=${offset}&search=${encodeURIComponent(search)}`, signal),
    refetchInterval: 30_000,
  });
  const data = inventory.data;
  return <>
    <header className="page-heading"><div><h1>Inventory</h1><p>Scanned copies across all batches. Rescans count again; all finishes are combined.</p></div></header>
    {data && <dl className="inventory-totals">
      {[ ["Scanned copies", data.totalCopies], ["Catalogue matched", data.catalogueCopies], ["Awaiting catalogue", data.awaitingCatalogueCopies], ["Needs identification", data.unresolvedCopies] ].map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{Number(value).toLocaleString()}</dd></div>)}
    </dl>}
    <section className="panel">
      <div className="inventory-toolbar"><label className="field"><span>Find a card</span><input type="search" maxLength={100} value={text} onChange={e => setText(e.target.value)} placeholder="Name, set or card number" /></label>{data && <span className="small muted">{data.matchingEntries.toLocaleString()} entries</span>}</div>
      <ErrorNote error={inventory.error} />
      {inventory.isPending ? <Loading /> : data && !data.entries.length ? <Empty title={search ? "No matching cards" : "No scanned cards yet"}>{search ? "Try another name, set or number." : "Card batches will appear here after processing."}</Empty> : data?.entries.map(entry => <InventoryRow key={entry.id} entry={entry} />)}
      {data && (offset > 0 || data.nextOffset != null) && <nav className="inventory-pagination" aria-label="Inventory pages">
        <button className="button secondary" disabled={offset === 0 || inventory.isFetching} onClick={() => setOffset(Math.max(0, offset - 50))}>Previous</button>
        <span className="small">{offset + 1}–{offset + data.entries.length} of {data.matchingEntries}</span>
        <button className="button secondary" disabled={data.nextOffset == null || inventory.isFetching} onClick={() => setOffset(data.nextOffset!)}>Next</button>
      </nav>}
      {!!data?.excludedCodeCards && <p className="small muted">{data.excludedCodeCards} online/code cards excluded.</p>}
    </section>
  </>;
}
