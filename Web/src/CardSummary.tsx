import { gameLabel, type CardGameDetection } from "./models";
import { ArrowUpRight, Check, CircleHelp, Clock3, Minus } from "lucide-react";
import {
  conditions,
  editionPresentation,
  energyIdentityName,
  finishPresentation,
  identityCard,
  IdentitySummary,
  type Identification,
} from "./Identification";
import type { ConditionCheck } from "./ConditionReview";
import { CardPrice, matchingCardPrice, type CardValuation } from "./Pricing";

/** Compact gallery presentation. Detailed evidence stays in the review dialogs. */
export function CardSummary({
  label,
  game,
  contentKind,
  identification,
  check,
  pending,
  incomplete,
  openMarks,
  price,
  pricingLoading,
}: {
  label: string;
  game: CardGameDetection["game"];
  contentKind?: CardGameDetection["contentKind"];
  identification?: Identification;
  check?: ConditionCheck;
  pending: boolean;
  incomplete: boolean;
  openMarks: () => void;
  price?: CardValuation;
  pricingLoading?: boolean;
}) {
  const card = identityCard(identification);
  const energyName = energyIdentityName(identification);
  if (game !== "pokemon") {
    return (
      <>
        <div className="card-summary-top">
          <span className="card-reference">{label}</span>
          <span className="badge">{contentKind === "pokemonCodeCard" ? "Pokémon code card" : gameLabel(game)}</span>
        </div>
        <div className="card-summary-identity">
          {contentKind === "pokemonCodeCard" ? (
            <>
              <h2>Pokémon online/code card</h2>
              <p>Redemption card · not a playable card.</p>
              <p className="small muted">Identity, finish and condition checks do not apply.</p>
            </>
          ) : game === "yugioh" ? (
            <>
              {card ? (
                <>
                  <h2>{card.name}</h2>
                  <p>
                    {card.setName}{" "}
                    <span className="card-number">{card.localID}</span>
                  </p>
                  {!!card.rarities?.length && (
                    <p>
                      Catalogue{" "}
                      {card.rarities.length === 1 ? "rarity" : "rarities"}:{" "}
                      {card.rarities.join(" / ")}
                    </p>
                  )}
                  {card.category && <p>{card.category}</p>}
                </>
              ) : (
                <IdentitySummary
                  identification={identification}
                  pending={pending}
                />
              )}
              <p className="small muted">Finish and condition not assessed.</p>
            </>
          ) : (
            <p>
              {pending
                ? "Checking card backs…"
                : "No unique supported card back detected. Identification is skipped."}
            </p>
          )}
        </div>
      </>
    );
  }
  const finish = finishPresentation(identification, pending);
  const edition = editionPresentation(identification);
  const FinishIcon =
    finish.state === "resolved" || finish.state === "reviewed"
      ? Check
      : finish.state === "uncertain"
        ? CircleHelp
        : finish.state === "pending"
          ? Clock3
          : Minus;
  const finishBasis =
    finish.state === "reviewed"
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
      <div className="card-summary-top">
        <div className="card-summary-meta">
          <span className="card-reference">{label}</span>
          <span className="badge">{gameLabel(game)}</span>
          {card && identification?.state === "confirmed" && (
            <span className="card-match-state">Confirmed</span>
          )}
          {incomplete && <span className="badge">Incomplete pair</span>}
        </div>
        <CardPrice
          value={matchingCardPrice(
            price,
            card?.id,
            identification?.finishEstimate?.edition?.pricingKey,
          )}
          loading={pricingLoading}
        />
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
            {!!card.rarities?.length && (
              <span title="Catalogue rarity; this does not identify the foil finish">
                {" · "}{card.rarities.join(" / ")}
              </span>
            )}
          </p>
          {edition && <p title={edition.detail}>{edition.label}</p>}
        </div>
      ) : energyName ? (
        <div className="card-summary-identity">
          <h2>{energyName}</h2>
          <p>Exact printing unknown</p>
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
