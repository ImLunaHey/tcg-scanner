import { useEffect, useRef, useState } from "react";
import { X, ZoomIn, ZoomOut } from "lucide-react";
import { Modal } from "./Modal";
import { useAuth } from "./auth";
import { cachedImageBlob, imageBlob } from "./api";
import { ErrorNote } from "./ui";
import { conditionSummary, type ConditionCheck } from "./ConditionReview";

export type LightboxImage = {
  sequence: number;
  sha256: string;
  editID?: string;
  label: string;
  width: number;
  height: number;
  conditionCheck?: ConditionCheck;
};
export function ImageLightbox({
  batchID,
  images,
  initial,
  initialFinding,
  close,
}: {
  batchID: string;
  images: LightboxImage[];
  initial: number;
  initialFinding?: number;
  close: () => void;
}) {
  const { api } = useAuth();
  const [selected, setSelected] = useState(initial);
  const [zoom, setZoom] = useState(1);
  const [focusedFinding, setFocusedFinding] = useState<number>();
  const [focusSerial, setFocusSerial] = useState(0);
  const pendingFocus = useRef<
    ConditionCheck["findings"][number]["region"] | null
  >(null);
  const [display, setDisplay] = useState<{
    key: string;
    url: string;
    full: boolean;
  }>();
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
  const path = `images/${batchID}:${current.sequence}${current.editID ? `?editID=${current.editID}` : ""}`;
  const key = `${path}:${current.sha256}`;
  const visible = display?.key === key ? display : undefined;
  const fit = Math.min(
    size.width / current.width,
    size.height / current.height,
    1,
  );
  const findings = current.conditionCheck?.findings || [];
  function focusFinding(index: number) {
    const region = findings[index]?.region;
    if (!region) return;
    setFocusedFinding(index);
    pendingFocus.current = region;
    setZoom(
      Math.max(1, Math.min(8, 0.55 / Math.max(region.width, region.height))),
    );
    setFocusSerial((value) => value + 1);
  }
  useEffect(() => {
    const region = pendingFocus.current;
    if (!region || !visible || !viewport.current) return;
    const width = current.width * fit * zoom,
      height = current.height * fit * zoom;
    viewport.current.scrollTo({
      left: Math.max(0, width * (region.x + region.width / 2) - size.width / 2),
      top: Math.max(
        0,
        height * (region.y + region.height / 2) - size.height / 2,
      ),
    });
    pendingFocus.current = null;
  }, [
    focusSerial,
    zoom,
    visible,
    current.width,
    current.height,
    fit,
    size.width,
    size.height,
  ]);
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
    const objectURLs: string[] = [];
    const show = (blob: Blob, full: boolean) => {
      const url = URL.createObjectURL(blob);
      objectURLs.push(url);
      setDisplay({ key, url, full });
    };
    setDisplay(undefined);
    const full = cachedImageBlob(api, path, current.sha256);
    const preview = cachedImageBlob(
      api,
      `images/${batchID}:${current.sequence}/thumbnail?format=jpeg${current.editID ? `&editID=${current.editID}` : ""}`,
      current.sha256,
    );
    if (full || preview) show((full || preview)!, !!full);
    setError(undefined);
    setZoom(1);
    setFocusedFinding(undefined);
    pendingFocus.current = null;
    if (findings.length)
      focusFinding(selected === initial ? (initialFinding ?? 0) : 0);
    if (viewport.current) viewport.current.scrollTo(0, 0);
    void imageBlob(api, path, controller.signal, current.sha256)
      .then((blob) => {
        if (!controller.signal.aborted) {
          show(blob, true);
        }
      })
      .catch((error) => {
        if (!controller.signal.aborted) setError(error);
      });
    return () => {
      controller.abort();
      objectURLs.forEach(URL.revokeObjectURL);
    };
  }, [
    api,
    batchID,
    current.sequence,
    current.editID,
    current.sha256,
    path,
    key,
  ]);
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
        {current.conditionCheck && (
          <div className="lightbox-condition">
            <p>{conditionSummary(current.conditionCheck)}</p>
            {findings.length > 0 && (
              <div className="lightbox-mark-links" aria-label="Candidate marks">
                {findings.map((finding, index) => (
                  <button
                    key={index}
                    className="text-button"
                    aria-pressed={focusedFinding === index}
                    onClick={() => focusFinding(index)}
                    title={finding.explanation}
                  >
                    Mark {index + 1}
                  </button>
                ))}
              </div>
            )}
            {focusedFinding !== undefined && findings[focusedFinding] && (
              <p className="small">{findings[focusedFinding].explanation}</p>
            )}
            <details>
              <summary>Scope and limitations</summary>
              <ul>
                {current.conditionCheck.limitations.map((text, index) => (
                  <li key={index}>{text}</li>
                ))}
              </ul>
            </details>
          </div>
        )}
        {visible && !visible.full && !error && (
          <p className="small muted" role="status">
            Preview · loading full-resolution PNG…
          </p>
        )}
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
          {visible ? (
            <div
              className="lightbox-canvas"
              style={{
                width: current.width * fit * zoom,
                height: current.height * fit * zoom,
              }}
            >
              <img
                src={visible.url}
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
              {current.conditionCheck && (
                <svg
                  className="lightbox-mark-overlay"
                  viewBox="0 0 1 1"
                  preserveAspectRatio="none"
                  aria-label="Candidate condition mark locations"
                >
                  {findings.map((finding, index) => (
                    <g key={index}>
                      <rect
                        x={finding.region.x}
                        y={finding.region.y}
                        width={finding.region.width}
                        height={finding.region.height}
                        vectorEffect="non-scaling-stroke"
                        className={
                          index === focusedFinding ? "focused" : undefined
                        }
                      />
                      <text
                        x={Math.min(0.95, finding.region.x + 0.005)}
                        y={Math.max(0.025, finding.region.y)}
                      >
                        {index + 1}
                      </text>
                    </g>
                  ))}
                </svg>
              )}
            </div>
          ) : (
            !error && <p role="status">Loading full-resolution image…</p>
          )}
        </div>
      </section>
    </Modal>
  );
}
