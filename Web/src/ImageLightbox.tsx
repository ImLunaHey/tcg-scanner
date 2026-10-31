import { useEffect, useRef, useState } from "react";
import { X, ZoomIn, ZoomOut } from "lucide-react";
import { Modal } from "./Modal";
import { useAuth } from "./auth";
import { imageBlob } from "./api";
import { ErrorNote } from "./ui";

export type LightboxImage = {
  sequence: number;
  editID?: string;
  label: string;
  width: number;
  height: number;
};
export function ImageLightbox({
  batchID,
  images,
  initial,
  close,
}: {
  batchID: string;
  images: LightboxImage[];
  initial: number;
  close: () => void;
}) {
  const { api } = useAuth();
  const [selected, setSelected] = useState(initial);
  const [zoom, setZoom] = useState(1);
  const [url, setURL] = useState("");
  const [error, setError] = useState<unknown>();
  const [size, setSize] = useState({ width: 800, height: 600 });
  const viewport = useRef<HTMLDivElement>(null);
  const drag = useRef<{
    x: number;
    y: number;
    left: number;
    top: number;
  } | null>(null);
  const current = images[selected];
  const fit = Math.min(
    size.width / current.width,
    size.height / current.height,
    1,
  );
  useEffect(() => {
    const element = viewport.current!;
    const observer = new ResizeObserver(() =>
      setSize({ width: element.clientWidth, height: element.clientHeight }),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const controller = new AbortController();
    let objectURL = "";
    setURL("");
    setError(undefined);
    setZoom(1);
    if (viewport.current) viewport.current.scrollTo(0, 0);
    void imageBlob(
      api,
      `images/${batchID}:${current.sequence}${current.editID ? `?editID=${current.editID}` : ""}`,
      controller.signal,
    )
      .then((blob) => {
        if (!controller.signal.aborted) {
          objectURL = URL.createObjectURL(blob);
          setURL(objectURL);
        }
      })
      .catch((error) => {
        if (!controller.signal.aborted) setError(error);
      });
    return () => {
      controller.abort();
      if (objectURL) URL.revokeObjectURL(objectURL);
    };
  }, [api, batchID, current.sequence, current.editID]);
  const resize = (value: number) =>
    setZoom(Math.max(0.5, Math.min(Math.max(8, 1 / fit), value)));
  return (
    <Modal close={close}>
      <section
        className="image-lightbox"
        role="dialog"
        aria-modal="true"
        aria-labelledby="lightbox-title"
      >
        <header className="lightbox-toolbar">
          <h2 id="lightbox-title">{current.label}</h2>
          <div className="lightbox-controls">
            {images.map((image, index) => (
              <button
                key={image.sequence}
                className="text-button"
                aria-pressed={index === selected}
                onClick={() => setSelected(index)}
              >
                {image.label}
              </button>
            ))}
            <button
              className="icon-button"
              aria-label="Zoom out"
              onClick={() => resize(zoom / 1.5)}
            >
              <ZoomOut size={20} />
            </button>
            <output aria-label="Zoom level">
              {Math.round(fit * zoom * 100)}%
            </output>
            <button
              className="icon-button"
              aria-label="Zoom in"
              onClick={() => resize(zoom * 1.5)}
            >
              <ZoomIn size={20} />
            </button>
            <button className="text-button" onClick={() => setZoom(1)}>
              Fit
            </button>
            <button className="text-button" onClick={() => resize(1 / fit)}>
              100%
            </button>
            <button
              autoFocus
              className="icon-button"
              aria-label="Close image"
              onClick={close}
            >
              <X size={22} />
            </button>
          </div>
        </header>
        <ErrorNote error={error} />
        <div
          ref={viewport}
          className="lightbox-viewport"
          tabIndex={0}
          aria-label="Zoomed image; scroll or drag to pan"
          onPointerDown={(e) => {
            if (e.pointerType !== "mouse" || e.button !== 0) return;
            const el = e.currentTarget;
            drag.current = {
              x: e.clientX,
              y: e.clientY,
              left: el.scrollLeft,
              top: el.scrollTop,
            };
            el.setPointerCapture(e.pointerId);
          }}
          onPointerMove={(e) => {
            const d = drag.current;
            if (d) {
              e.currentTarget.scrollLeft = d.left - (e.clientX - d.x);
              e.currentTarget.scrollTop = d.top - (e.clientY - d.y);
            }
          }}
          onPointerUp={() => {
            drag.current = null;
          }}
          onPointerCancel={() => {
            drag.current = null;
          }}
        >
          {url ? (
            <img
              src={url}
              alt={current.label}
              draggable={false}
              style={{
                width: current.width * fit * zoom,
                height: current.height * fit * zoom,
              }}
              onError={() =>
                setError(
                  new Error(
                    "This image could not be displayed. Download the original or prepare a PNG copy.",
                  ),
                )
              }
            />
          ) : (
            !error && <p role="status">Loading full-resolution image…</p>
          )}
        </div>
      </section>
    </Modal>
  );
}
