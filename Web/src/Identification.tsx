import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { useAuth } from "./auth";
import { Modal } from "./Modal";
import { ErrorNote, Field, Loading } from "./ui";
import { captureLabel, cardLanguages, uuid, type CardLanguage } from "./models";

export type CardCandidate = {
  id: string;
  name: string;
  localID: string;
  setID: string;
  setName: string;
  officialSetCount?: number;
  evidence: string[];
  availableFinishes: string[];
  provider?: string;
  catalogueID?: string;
  rarities?: string[];
  category?: string;
  types?: string[];
};
export type EditionEvidence = {
  state: "notDetected" | "matched" | "uncertain" | "unlisted" | "unavailable";
  candidateID?: string;
  name?: string;
  pricingKey: string;
  stamp: {
    state: "present" | "absent" | "uncertain" | "unsupported";
    kind: string;
    method: string;
    evidence: string[];
  };
  variants: {
    id: string;
    source: string;
    finish: string;
    stamps: string[];
    years: number[];
  }[];
  evidence: string[];
};
export type FinishEstimate = {
  state: "estimated" | "uncertain" | "unsupported";
  suggestedFinish?: string;
  candidateID?: string;
  evidence: string[];
  limitations: string[];
  method: string;
  uncertaintyReason?: string;
  edition?: EditionEvidence;
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
  sourceEditID?: string;
  conditionCheck?: import("./ConditionReview").ConditionCheck;
  evidence?: {
    candidates: CardCandidate[];
    energySymbol?: { type: string; similarity: number; method: string };
    lines: { text: string }[];
    warnings: string[];
  };
  error?: { message: string };
  selectedCardID?: string;
  reviewedAt?: string;
  finishEstimate?: FinishEstimate;
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

export type FinishPresentation = {
  label: string;
  state: "resolved" | "reviewed" | "uncertain" | "pending" | "unavailable";
  detail: string;
};

export function editionPresentation(
  value?: Identification,
  selection?: string,
):
  | { label: string; state: "resolved" | "uncertain"; detail: string }
  | undefined {
  const edition = value?.finishEstimate?.edition;
  if (!edition || !["present", "uncertain"].includes(edition.stamp.state))
    return undefined;
  const card =
    selection && value && ["confirmed", "reviewRequired"].includes(value.state)
      ? value.evidence?.candidates.find(
          (candidate) => candidate.id === selection,
        )
      : identityCard(value);
  const matched =
    edition.stamp.state === "present" &&
    edition.state === "matched" &&
    !!edition.name?.trim() &&
    !!card &&
    edition.candidateID === card.id;
  const detail = [
    ...edition.stamp.evidence,
    ...edition.evidence,
    matched
      ? "The stamp and catalogue support this edition label. The stamp alone does not establish a printing year or finish."
      : "The stamped edition is unresolved; its ordinary-printing finish and price cannot establish this variant.",
  ].join(" ");
  return {
    label: matched
      ? edition.name!
      : edition.stamp.state === "uncertain"
        ? "Possible pumpkin stamp"
        : "Pumpkin stamp · edition unresolved",
    state: matched ? "resolved" : "uncertain",
    detail,
  };
}

export function finishPresentation(
  value?: Identification,
  pending = false,
): FinishPresentation {
  if (value && value.finish !== "unverified")
    return {
      label: finishes[value.finish] || value.finish,
      state: "reviewed",
      detail: "Physically reviewed finish.",
    };
  if ((!value && pending) || value?.state === "processing")
    return {
      label: "Checking…",
      state: "pending",
      detail: "Automatic finish analysis is pending.",
    };
  const estimate = value?.finishEstimate;
  if (!estimate)
    return {
      label: "Not checked",
      state: "unavailable",
      detail: "No automatic finish result is available.",
    };
  const cardID =
    value?.selectedCardID ||
    (value?.evidence?.candidates.length === 1
      ? value.evidence.candidates[0].id
      : undefined);
  const current =
    !!cardID &&
    estimate.candidateID === cardID &&
    (value?.state === "reviewRequired" || value?.state === "confirmed");
  if (current && estimate.state === "estimated" && estimate.suggestedFinish)
    return {
      label: finishes[estimate.suggestedFinish] || estimate.suggestedFinish,
      state: "resolved",
      detail:
        "Automatic result from local image, printed-marker and catalogue evidence; not physically confirmed.",
    };
  if (current && estimate.state === "unsupported")
    return {
      label: "Not assessed",
      state: "unavailable",
      detail: "This printing is outside the automatic finish check's scope.",
    };
  if (
    current &&
    estimate.state === "uncertain" &&
    estimate.uncertaintyReason === "multipleRegularFinishes"
  )
    return {
      label: "Non-holo or holo",
      state: "uncertain",
      detail:
        "Both variants share this printed code; available image evidence has not resolved the finish.",
    };
  const editionReasons: Record<string, string> = {
    stampUncertain:
      "A possible pumpkin stamp needs confirmation before the finish can be resolved.",
    editionUnavailable:
      "A pumpkin stamp was detected, but its catalogue edition is unresolved.",
    multipleEditionFinishes:
      "The possible stamped editions have different finishes. The physical finish needs review.",
  };
  return {
    label: "Uncertain",
    state: "uncertain",
    detail: current
      ? editionReasons[estimate.uncertaintyReason || ""] ||
        "The automatic check could not resolve the finish."
      : "A finish result for the current card identification is not available.",
  };
}

export function finishDescription(value: Identification): string {
  const result = finishPresentation(value);
  if (result.state === "resolved") return `${result.label} (estimated)`;
  if (result.state === "reviewed") return `${result.label} (reviewed)`;
  if (result.state === "uncertain" && result.label !== "Uncertain")
    return result.label;
  return value.finishEstimate ? "Needs review" : "Not verified";
}

export function IdentificationReview({
  batchID,
  sequence,
  cardLanguage,
  close,
}: {
  batchID: string;
  sequence: number;
  cardLanguage: CardLanguage;
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
        language: cardLanguage,
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
  const estimateFinish = useMutation({
    mutationFn: () =>
      api.send<Identification>(
        `images/${batchID}:${sequence}/finish-estimate`,
        "POST",
        { identificationID: result!.request.id },
      ),
    onSuccess: update,
  });
  const candidates = result?.evidence?.candidates || [];
  const chosen = candidates.find((candidate) => candidate.id === selected);
  const edition = editionPresentation(
    result ?? undefined,
    selected || undefined,
  );
  const busy =
    identify.isPending ||
    review.isPending ||
    estimateFinish.isPending ||
    result?.state === "processing";
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
          {cardLanguages[cardLanguage]} card fronts · text recognition runs on
          your Mac. TCGdex supplies the catalogue; your scan is not uploaded.
        </p>
        <ErrorNote
          error={
            current.error ||
            identify.error ||
            review.error ||
            estimateFinish.error ||
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
                {energyIdentityName(result)
                  ? `${energyIdentityName(result)} · Exact printing unknown.`
                  : "No matching card found. Check that this is a clear front image; a back cannot identify the card."}
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
            {edition && (
              <p className="small" title={edition.detail}>
                {edition.label}
              </p>
            )}
            {result?.state === "noMatch" && result.evidence && (
              <button
                type="button"
                className="text-button"
                disabled={busy}
                onClick={() => estimateFinish.mutate()}
              >
                Refresh local stamp check
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
                {result?.finishEstimate && (
                  <div className="small">
                    <p>
                      Finish: {finishDescription(result)}. Estimates require
                      physical confirmation.
                    </p>
                    <details>
                      <summary>Why this finish estimate</summary>
                      {[
                        ...result.finishEstimate.evidence,
                        ...result.finishEstimate.limitations,
                      ].map((text, index) => (
                        <p key={index}>{text}</p>
                      ))}
                    </details>
                    {result.finishEstimate.state === "estimated" &&
                      result.finishEstimate.suggestedFinish &&
                      selected === result.finishEstimate.candidateID && (
                        <button
                          type="button"
                          className="text-button"
                          onClick={() =>
                            setFinish(result.finishEstimate!.suggestedFinish!)
                          }
                        >
                          Use finish estimate in review
                        </button>
                      )}
                  </div>
                )}
                <button
                  type="button"
                  className="text-button"
                  disabled={busy || candidates.length !== 1}
                  onClick={() => estimateFinish.mutate()}
                >
                  {estimateFinish.isPending
                    ? "Estimating finish locally…"
                    : result?.finishEstimate
                      ? "Refresh finish estimate"
                      : "Estimate finish locally"}
                </button>
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

export function identityCard(
  identification?: Identification,
): CardCandidate | undefined {
  if (!identification) return undefined;
  return identification.state === "confirmed"
    ? identification.evidence?.candidates.find(
        (value) => value.id === identification.selectedCardID,
      )
    : identification.state === "reviewRequired" &&
        identification.evidence?.candidates.length === 1
      ? identification.evidence.candidates[0]
      : undefined;
}

export function energyIdentityName(value?: Identification): string | undefined {
  const symbol = value?.evidence?.energySymbol;
  return value?.state === "noMatch" && symbol
    ? `${symbol.type} Energy`
    : undefined;
}

export function IdentitySummary({
  identification,
  pending = false,
}: {
  identification?: Identification;
  pending?: boolean;
}) {
  if (!identification)
    return (
      <p className="small muted">
        {pending
          ? "Waiting for automatic identification…"
          : "Not identified yet"}
      </p>
    );
  const card = identityCard(identification);
  const edition = editionPresentation(identification);
  const energyName = energyIdentityName(identification);
  if (energyName)
    return <p className="small"><strong>{energyName}</strong><br />Exact printing unknown · type recognised from the corner symbol.</p>;
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
        {edition && (
          <>
            <span title={edition.detail}>{edition.label}</span>
            <br />
          </>
        )}
        {[
          card.category === "Pokemon" ? "Pokémon" : card.category,
          ...(card.types || []),
        ]
          .filter(Boolean)
          .join(" · ")}
        <br />
        Finish: {finishDescription(identification)} · Condition:{" "}
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
      {edition && (
        <>
          <br />
          <span title={edition.detail}>{edition.label}</span>
        </>
      )}
    </p>
  );
}
