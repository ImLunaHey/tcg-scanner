import { ArrowUpRight, Check, CircleHelp, Clock3, Minus } from "lucide-react";
import {
  conditions,
  finishPresentation,
  identityCard,
  IdentitySummary,
  type Identification,
} from "./Identification";
import type { ConditionCheck } from "./ConditionReview";

/** Compact gallery presentation. Detailed evidence stays in the review dialogs. */
export function CardSummary({
  label,
  identification,
  check,
  pending,
  incomplete,
  openMarks,
}: {
  label: string;
  identification?: Identification;
  check?: ConditionCheck;
  pending: boolean;
  incomplete: boolean;
  openMarks: () => void;
}) {
  const card = identityCard(identification);
  const finish = finishPresentation(identification, pending);
  const FinishIcon =
    finish.state === "resolved" || finish.state === "reviewed"
      ? Check
      : finish.state === "uncertain"
        ? CircleHelp
        : finish.state === "pending"
          ? Clock3
          : Minus;
  const finishBasis =
    finish.state === "resolved"
      ? "Auto"
      : finish.state === "reviewed"
        ? "Confirmed"
        : finish.state === "uncertain"
          ? "Unresolved"
          : undefined;
  const marks = check?.state === "checked" ? check.findings.length : 0;
  const borderStatus =
    check?.state === "checked"
      ? "No marks flagged"
      : check?.state === "inconclusive"
        ? "Inconclusive"
        : check?.state === "unsupported"
          ? "Not assessed"
          : pending
            ? "Checking…"
            : "Not checked";
  const reviewedGrade =
    identification && identification.condition !== "unassessed"
      ? conditions[identification.condition] || identification.condition
      : undefined;
  return (
    <>
      <div className="card-summary-meta">
        <span className="card-reference">{label}</span>
        {card && (
          <span className="card-match-state">
            {identification?.state === "confirmed"
              ? "Confirmed"
              : "Suggested match"}
          </span>
        )}
        {incomplete && <span className="badge">Incomplete pair</span>}
      </div>
      {card ? (
        <div className="card-summary-identity">
          <h2>{card.name}</h2>
          <p>
            {card.setName}{" "}
            <span className="card-number">
              {card.localID}
              {card.officialSetCount ? `/${card.officialSetCount}` : ""}
            </span>
          </p>
        </div>
      ) : (
        <div className="card-summary-identity">
          <IdentitySummary identification={identification} pending={pending} />
        </div>
      )}
      <dl className="card-facts">
        <div>
          <dt>Finish</dt>
          <dd>
            <span
              className={`finish-pill finish-${finish.state}`}
              title={finish.detail}
            >
              <FinishIcon size={13} aria-hidden="true" />
              <span>{finish.label}</span>
              <span className="sr-only">. {finish.detail}</span>
            </span>
            {finishBasis && <span className="finish-basis">{finishBasis}</span>}
          </dd>
        </div>
        <div>
          <dt>Back border</dt>
          <dd>
            {marks ? (
              <button className="card-mark-link" onClick={openMarks}>
                {marks} possible {marks === 1 ? "mark" : "marks"}
                <ArrowUpRight size={14} aria-hidden="true" />
              </button>
            ) : (
              <span>{borderStatus}</span>
            )}
          </dd>
        </div>
      </dl>
      <p className="card-grade-note">
        {reviewedGrade
          ? `Grade: ${reviewedGrade} · reviewed`
          : "Border check only · condition not graded"}
      </p>
    </>
  );
}
