import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { useAuth } from "./auth";
import { Modal } from "./Modal";
import { ErrorNote, Field, Loading } from "./ui";
import { captureLabel, uuid } from "./models";

export type CardCandidate = {
  id: string;
  name: string;
  localID: string;
  setID: string;
  setName: string;
  officialSetCount?: number;
  evidence: string[];
  availableFinishes: string[];
  category?: string;
  types?: string[];
};
export type Identification = {
  request: { id: string; sequence: number; language: string };
  state:
    | "processing"
    | "reviewRequired"
    | "noMatch"
    | "failed"
    | "confirmed"
    | "rejected";
  sourceSHA256: string;
  evidence?: {
    candidates: CardCandidate[];
    lines: { text: string }[];
    warnings: string[];
  };
  error?: { message: string };
  selectedCardID?: string;
  reviewedAt?: string;
  finish: string;
  condition: string;
};
type IdentificationResponse = { identification: Identification | null };
export const finishes: Record<string, string> = {
  unverified: "Not verified",
  normal: "Non-holo",
  holo: "Holo",
  reverseHolo: "Reverse holo",
  other: "Other finish",
};
export const conditions: Record<string, string> = {
  unassessed: "Not assessed",
  nearMint: "Near mint",
  lightlyPlayed: "Lightly played",
  moderatelyPlayed: "Moderately played",
  heavilyPlayed: "Heavily played",
  damaged: "Damaged",
};

export function IdentificationReview({
  batchID,
  sequence,
  close,
}: {
  batchID: string;
  sequence: number;
  close: () => void;
}) {
  const { api } = useAuth();
  const queries = useQueryClient();
  const path = `images/${batchID}:${sequence}/identification`;
  const queryKey = ["identification", batchID, sequence];
  const current = useQuery({
    queryKey,
    queryFn: ({ signal }) => api.get<IdentificationResponse>(path, signal),
    refetchInterval: (query) =>
      query.state.data?.identification?.state === "processing" ? 1500 : false,
  });
  const result = current.data?.identification;
  const requestID = useRef(uuid());
  const [selected, setSelected] = useState("");
  const [finish, setFinish] = useState("unverified");
  const [condition, setCondition] = useState("unassessed");
  useEffect(() => {
    setSelected(result?.selectedCardID || "");
    setFinish(result?.finish || "unverified");
    setCondition(result?.condition || "unassessed");
  }, [
    result?.request.id,
    result?.selectedCardID,
    result?.finish,
    result?.condition,
  ]);
  function update(value: Identification) {
    queries.setQueryData(queryKey, { identification: value });
    void queries.invalidateQueries({ queryKey: ["batch", batchID] });
  }
  const identify = useMutation({
    mutationFn: () =>
      api.send<Identification>(path, "POST", {
        id: requestID.current,
        language: "en",
      }),
    onSuccess: (value) => {
      update(value);
      requestID.current = uuid();
    },
    onError: () => {
      void current.refetch();
    },
  });
  const review = useMutation({
    mutationFn: (reject: boolean) =>
      api.send<Identification>(path, "PATCH", {
        identificationID: result!.request.id,
        ...(reject ? {} : { cardID: selected }),
        finish,
        condition,
      }),
    onSuccess: update,
  });
  const candidates = result?.evidence?.candidates || [];
  const chosen = candidates.find((candidate) => candidate.id === selected);
  const busy =
    identify.isPending || review.isPending || result?.state === "processing";
  const mayIdentify =
    !result || ["failed", "noMatch", "rejected"].includes(result.state);
  return (
    <Modal close={close}>
      <section
        className="modal identification-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="identification-title"
      >
        <div className="section-heading">
          <h2 id="identification-title">Identify {captureLabel(sequence)}</h2>
          <button
            autoFocus
            className="icon-button"
            aria-label="Close identification"
            onClick={close}
          >
            <X />
          </button>
        </div>
        <p className="muted small">
          English card fronts · text recognition runs on your Mac. TCGdex
          supplies the catalogue; your scan is not uploaded.
        </p>
        <ErrorNote
          error={
            current.error ||
            identify.error ||
            review.error ||
            result?.error?.message
          }
        />
        {current.isPending ? (
          <Loading />
        ) : (
          <>
            {result?.state === "processing" && (
              <Loading label="Reading text and checking the catalogue… You can close this panel while it works." />
            )}
            {result?.state === "noMatch" && (
              <p>
                No matching card found. Check that this is a clear front image;
                a back cannot identify the card.
              </p>
            )}
            {result?.state === "confirmed" && (
              <p className="success-note" role="status">
                Identity confirmed by review. Finish: {finishes[result.finish]}.
                Condition: {conditions[result.condition]}.
              </p>
            )}
            {result?.state === "rejected" && (
              <p>Suggestions rejected. No card identity is assigned.</p>
            )}
            {mayIdentify && (
              <button
                className="button primary"
                disabled={busy || current.isError}
                onClick={() => identify.mutate()}
              >
                {identify.isPending
                  ? "Starting…"
                  : identify.isError
                    ? "Retry identification request"
                    : result
                      ? "Try identification again"
                      : "Identify this image"}
              </button>
            )}
            {!!candidates.length && result?.state !== "rejected" && (
              <fieldset disabled={busy} className="form-stack">
                <legend>Choose a matching card and set</legend>
                <p className="small muted">
                  These are suggestions, not confirmed identities. Compare the
                  printed number and set before choosing.
                </p>
                {candidates.map((candidate) => (
                  <label key={candidate.id} className="candidate-option">
                    <input
                      type="radio"
                      name="card-candidate"
                      value={candidate.id}
                      checked={selected === candidate.id}
                      onChange={() => {
                        setSelected(candidate.id);
                        setFinish("unverified");
                        setCondition("unassessed");
                      }}
                    />
                    <span>
                      <strong>{candidate.name}</strong>
                      <span>
                        {[
                          candidate.category === "Pokemon"
                            ? "Pokémon"
                            : candidate.category,
                          ...(candidate.types || []),
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                      </span>
                      <span className="candidate-set">
                        {candidate.setName} · {candidate.localID}
                        {candidate.officialSetCount
                          ? `/${candidate.officialSetCount}`
                          : ""}
                      </span>
                      <span className="small muted">
                        {candidate.evidence.join(" · ")}
                      </span>
                    </span>
                  </label>
                ))}
                <div className="form-row">
                  <Field label="Finish · manual">
                    <select
                      value={finish}
                      onChange={(event) => setFinish(event.target.value)}
                    >
                      {Object.entries(finishes).map(([value, label]) => (
                        <option value={value} key={value}>
                          {label}
                        </option>
                      ))}
                    </select>
                  </Field>
                  <Field label="Condition · manual">
                    <select
                      value={condition}
                      onChange={(event) => setCondition(event.target.value)}
                    >
                      {Object.entries(conditions).map(([value, label]) => (
                        <option value={value} key={value}>
                          {label}
                        </option>
                      ))}
                    </select>
                  </Field>
                </div>
                {chosen && (
                  <p className="small muted">
                    Catalogue finishes:{" "}
                    {chosen.availableFinishes
                      .map((finish) => finishes[finish])
                      .join(", ") || "Not listed"}
                    . This describes available versions, not the finish of your
                    card.
                  </p>
                )}
                <p className="small muted">
                  Inspect the physical card for finish, dents and scratches.
                  Condition is your assessment; it is not graded automatically.
                </p>
                <div className="toolbar">
                  <button
                    className="button primary"
                    disabled={!selected || busy}
                    onClick={() => review.mutate(false)}
                  >
                    {review.isPending ? "Saving…" : "Save reviewed identity"}
                  </button>
                  <button
                    className="button secondary"
                    disabled={busy}
                    onClick={() => review.mutate(true)}
                  >
                    Reject suggestions
                  </button>
                </div>
              </fieldset>
            )}
            {result?.evidence && (
              <details className="details-panel">
                <summary>OCR evidence and limitations</summary>
                {result.evidence.warnings.map((warning, index) => (
                  <p key={index} className="small muted">
                    {warning}
                  </p>
                ))}
                <pre className="ocr-text">
                  {result.evidence.lines.map((line) => line.text).join("\n") ||
                    "No text found."}
                </pre>
              </details>
            )}
          </>
        )}
      </section>
    </Modal>
  );
}

export function IdentitySummary({
  identification,
}: {
  identification?: Identification;
}) {
  if (!identification) return <p className="small muted">Not identified yet</p>;
  const card =
    identification.state === "confirmed"
      ? identification.evidence?.candidates.find(
          (value) => value.id === identification.selectedCardID,
        )
      : identification.state === "reviewRequired" &&
          identification.evidence?.candidates.length === 1
        ? identification.evidence.candidates[0]
        : undefined;
  if (card)
    return (
      <p className="small">
        <span>
          {identification.state === "confirmed"
            ? "Confirmed"
            : "Suggested · review required"}
        </span>
        <br />
        <strong>{card.name}</strong> · {card.setName} · {card.localID}
        {card.officialSetCount ? `/${card.officialSetCount}` : ""}
        <br />
        {[
          card.category === "Pokemon" ? "Pokémon" : card.category,
          ...(card.types || []),
        ]
          .filter(Boolean)
          .join(" · ")}
        <br />
        Finish: {finishes[identification.finish]} · Condition:{" "}
        {conditions[identification.condition]} (manual)
      </p>
    );
  return (
    <p className="small muted">
      {identification.state === "processing"
        ? "Identifying card…"
        : identification.state === "reviewRequired"
          ? `${identification.evidence?.candidates.length || 0} possible matches · review identity`
          : identification.state === "noMatch"
            ? "No card match found"
            : identification.state === "failed"
              ? "Identification failed · retry in review"
              : identification.state === "rejected"
                ? "Suggestions rejected"
                : "Identity unavailable"}
    </p>
  );
}
