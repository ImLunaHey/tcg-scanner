import { useEffect, useRef, useState } from "react";
import { ImageIcon } from "lucide-react";
import { useAuth } from "./auth";
import { imageBlob } from "./api";

export type MissingCard = { id: string; name: string; number: string; language: string; thumbnailKey?: string };
export type MissingCards = { cards: MissingCard[]; catalogueCount?: number; note?: string };

export function MissingInventoryCard({ card, setName }: { card: MissingCard; setName: string }) {
  const { api } = useAuth();
  const node = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [url, setURL] = useState("");
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!node.current) return;
    if (typeof IntersectionObserver === "undefined") { setVisible(true); return; }
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) { setVisible(true); observer.disconnect(); }
    }, { rootMargin: "240px" });
    observer.observe(node.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    setURL(""); setFailed(false);
    if (!visible || !card.thumbnailKey) return;
    const controller = new AbortController();
    let objectURL = "";
    void imageBlob(api, `inventory/references/${card.thumbnailKey}/thumbnail`, controller.signal).then(blob => {
      if (!controller.signal.aborted) { objectURL = URL.createObjectURL(blob); setURL(objectURL); }
    }).catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => { controller.abort(); if (objectURL) URL.revokeObjectURL(objectURL); };
  }, [api, card.thumbnailKey, visible]);
  return <article className="inventory-card inventory-card-missing" aria-label={`${card.name}, missing`}>
    <div ref={node} className="inventory-card-image">
      <div className="inventory-thumbnail"><div className="thumbnail">
        {url ? <img src={url} alt={card.name} /> : failed || !card.thumbnailKey ? <span className="small muted">Artwork unavailable</span> : <ImageIcon size={28} aria-hidden="true" />}
      </div></div>
      <span className="inventory-copy-badge">Missing</span>
    </div>
    <div className="inventory-card-caption"><h2 title={card.name}>{card.name}</h2>
      <p className="inventory-set-name">{setName}</p>
      <div className="inventory-card-foot"><span>{card.number}</span><span className="inventory-language">{card.language.toUpperCase()}</span></div>
    </div>
  </article>;
}
