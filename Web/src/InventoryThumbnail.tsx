import { useEffect, useRef, useState } from "react";
import { ImageIcon } from "lucide-react";
import { Thumbnail } from "./Library";
import type { OriginalImage } from "./models";

export type InventoryPreview = { batchID: string; image: OriginalImage; editID?: string };
/** Mount the shared authenticated, cached thumbnail loader only near the viewport. */
export function InventoryThumbnail({ preview }: { preview: InventoryPreview }) {
  const node = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    if (!node.current) return;
    if (typeof IntersectionObserver === "undefined") { setVisible(true); return; }
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) { setVisible(true); observer.disconnect(); }
    }, { rootMargin: "240px" });
    observer.observe(node.current);
    return () => observer.disconnect();
  }, []);
  return <div ref={node} className="inventory-thumbnail">
    {visible ? <Thumbnail id={preview.batchID} image={preview.image} editID={preview.editID} /> : <ImageIcon size={28} aria-hidden="true" />}
  </div>;
}
