import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { useAuth } from "./auth";
import { imageBlob } from "./api";
import { Modal } from "./Modal";
import { ImageLightbox } from "./ImageLightbox";
import { ErrorNote, Loading } from "./ui";
import type { Identification } from "./Identification";
import type { Batch } from "./models";

export type ConditionCheck = {
  state: "checked" | "unsupported" | "inconclusive";
  method: string;
  findings: {
    kind: string;
    region: { x: number; y: number; width: number; height: number };
    explanation: string;
  }[];
  limitations: string[];
};
export function conditionSummary(check?: ConditionCheck): string {
  if (!check) return "Visible marks: not checked";
  if (check.state !== "checked")
    return "Visible marks: needs manual inspection";
  return check.findings.length
    ? `Visible marks: ${check.findings.length} candidate area${check.findings.length === 1 ? "" : "s"}`
    : "No border marks flagged · not a grade";
}
export function ConditionReview({
  batch,
  sides,
  initial,
  close,
}: {
  batch: Batch;
  sides: { sequence: number; label: string }[];
  initial: number;
  close: () => void;
}) {
  const { api } = useAuth();
  const queries = useQueryClient();
  const [sequence, setSequence] = useState(initial);
  const [lightbox, setLightbox] = useState<{ finding?: number }>();
  const key = ["identification", batch.id, sequence];
  const current = useQuery({
    queryKey: key,
    queryFn: ({ signal }) =>
      api.get<{ identification: Identification | null }>(
        `images/${batch.id}:${sequence}/identification`,
        signal,
      ),
  });
  const identity = current.data?.identification;
  const check = identity?.conditionCheck;
  const inspect = useMutation({
    mutationFn: () =>
      api.send<Identification>(
        `images/${batch.id}:${sequence}/condition-check`,
        "POST",
        { identificationID: identity!.request.id },
      ),
    onSuccess: (value) => {
      queries.setQueryData(key, { identification: value });
      void queries.invalidateQueries({ queryKey: ["batch", batch.id] });
    },
  });
  const source = identity?.sourceEditID
    ? batch.processedImages?.find((e) => e.request.id === identity.sourceEditID)
        ?.image
    : batch.images.find((i) => i.sequence === sequence);
  const validSource = source?.sha256 === identity?.sourceSHA256;
  const [url, setURL] = useState("");
  const [imageError, setImageError] = useState<unknown>();
  useEffect(() => {
    const abort = new AbortController();
    let objectURL = "";
    setURL("");
    setImageError(undefined);
    if (identity && source && validSource) {
      const path = `images/${batch.id}:${sequence}${identity.sourceEditID ? `?editID=${encodeURIComponent(identity.sourceEditID)}` : ""}`;
      void imageBlob(api, path, abort.signal, source.sha256)
        .then((blob) => {
          if (!abort.signal.aborted) {
            objectURL = URL.createObjectURL(blob);
            setURL(objectURL);
          }
        })
        .catch((e) => {
          if (!abort.signal.aborted) setImageError(e);
        });
    }
    return () => {
      abort.abort();
      if (objectURL) URL.revokeObjectURL(objectURL);
    };
  }, [
    api,
    batch.id,
    sequence,
    identity?.sourceEditID,
    source?.sha256,
    validSource,
  ]);
  if (lightbox && source && identity && validSource)
    return (
      <ImageLightbox
        batchID={batch.id}
        images={[
          {
            ...source,
            editID: identity.sourceEditID,
            label: `${sides.find((s) => s.sequence === sequence)?.label || "Card"} · condition check`,
            conditionCheck: check,
          },
        ]}
        initial={0}
        initialFinding={lightbox.finding}
        close={() => setLightbox(undefined)}
      />
    );
  return (
    <Modal close={close}>
      <section
        className="modal condition-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="condition-title"
      >
        <div className="section-heading">
          <h2 id="condition-title">Visible condition checks</h2>
          <button
            autoFocus
            className="icon-button"
            aria-label="Close condition checks"
            onClick={close}
          >
            <X />
          </button>
        </div>
        <p className="muted small">
          Local checks for possible whitening and elongated marks in the blue
          back border. Inspect each marked area on the physical card. This does
          not assign a selling grade.
        </p>
        <label>
          Card side{" "}
          <select
            aria-label="Condition check side"
            value={sequence}
            disabled={inspect.isPending}
            onChange={(e) => setSequence(Number(e.target.value))}
          >
            {sides.map((s) => (
              <option key={s.sequence} value={s.sequence}>
                {s.label}
              </option>
            ))}
          </select>
        </label>
        <ErrorNote error={current.error || inspect.error || imageError} />
        {current.isPending ? (
          <Loading />
        ) : (
          <>
            <p role="status">{conditionSummary(check)}</p>
            <button
              className="button secondary"
              disabled={
                !identity ||
                identity.state === "processing" ||
                inspect.isPending
              }
              onClick={() => inspect.mutate()}
            >
              {inspect.isPending
                ? "Checking…"
                : check
                  ? "Refresh visible checks"
                  : "Check visible marks locally"}
            </button>
            {!identity && (
              <p>
                Card preparation and identification must finish before checking
                this side.
              </p>
            )}
            <button
              className="text-button"
              disabled={!url}
              onClick={() => setLightbox({})}
            >
              Open marked image in lightbox
            </button>
            <div className="condition-content">
              <div>
                {url ? (
                  <div className="condition-image">
                    <img
                      src={url}
                      alt="Card side with candidate condition areas"
                    />
                    <svg
                      viewBox="0 0 1 1"
                      preserveAspectRatio="none"
                      aria-label="Candidate mark locations"
                    >
                      {check?.findings.map((f, i) => (
                        <g key={i}>
                          <rect
                            x={f.region.x}
                            y={f.region.y}
                            width={f.region.width}
                            height={f.region.height}
                          />
                          <text
                            x={Math.min(0.93, f.region.x + 0.01)}
                            y={Math.max(0.035, f.region.y)}
                          >
                            {i + 1}
                          </text>
                        </g>
                      ))}
                    </svg>
                  </div>
                ) : identity && validSource ? (
                  <Loading label="Loading inspected image…" />
                ) : (
                  <p className="muted">The inspected source is unavailable.</p>
                )}
              </div>
              <div>
                {check?.findings.length ? (
                  <ol className="condition-findings">
                    {check.findings.map((f, i) => (
                      <li key={i}>
                        <button
                          className="text-button"
                          onClick={() => setLightbox({ finding: i })}
                        >
                          {f.explanation}
                        </button>
                      </li>
                    ))}
                  </ol>
                ) : null}
                <ul className="condition-limitations">
                  {check?.limitations.map((text, i) => (
                    <li key={i}>{text}</li>
                  ))}
                </ul>
                <p className="small muted">
                  Condition remains your manual assessment. No flagged marks
                  does not mean Near Mint.
                </p>
              </div>
            </div>
          </>
        )}
      </section>
    </Modal>
  );
}
