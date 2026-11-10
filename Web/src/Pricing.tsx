import type { Batch } from "./models";

export type CardValuation = {
  number: number;
  cardID?: string;
  editionKey?: string;
  state: "priced" | "range" | "pending" | "unavailable";
  lowPence?: number;
  highPence?: number;
  stale: boolean;
  reason: string;
  quotes: { updated?: string }[];
};
export function matchingCardPrice(
  value: CardValuation | undefined,
  cardID: string | undefined,
  editionKey: string | undefined,
): CardValuation | undefined {
  return cardID && value?.cardID === cardID && value.editionKey === editionKey
    ? value
    : undefined;
}

/** Any edition change must invalidate prices even when base ID and finish agree. */
export function pricingRevision(
  batch?: Pick<Batch, "images" | "cardIdentifications">,
): string {
  return JSON.stringify([
    batch?.images.map((image) => image.sequence),
    batch?.cardIdentifications?.map((value) => [
      value.request.id,
      value.state,
      value.selectedCardID,
      value.finish,
      value.finishEstimate?.candidateID,
      value.finishEstimate?.state,
      value.finishEstimate?.suggestedFinish,
      value.finishEstimate?.uncertaintyReason,
      value.finishEstimate?.edition,
    ]),
  ]);
}
export type BatchValuation = {
  batchID: string;
  cards: CardValuation[];
  pricedCount: number;
  totalCount: number;
  lowPence?: number;
  highPence?: number;
  refreshing: boolean;
  stale: boolean;
  exchangeRate?: { poundsPerEuro: number; date: string };
  warning: string;
};
const sterling = new Intl.NumberFormat("en-GB", {
  style: "currency",
  currency: "GBP",
});
export function priceLabel(low?: number, high?: number): string {
  if (low === undefined || high === undefined) return "—";
  return low === high
    ? sterling.format(low / 100)
    : `${sterling.format(low / 100)}–${sterling.format(high / 100)}`;
}

export function CardPrice({
  value,
  loading,
}: {
  value?: CardValuation;
  loading?: boolean;
}) {
  const pending = value?.state === "pending" || (!value && loading);
  const dates = [
    ...new Set(
      value?.quotes.flatMap((q) =>
        q.updated ? [q.updated.slice(0, 10)] : [],
      ) || [],
    ),
  ];
  const detail = [
    "Cardmarket trend · GBP",
    value?.reason,
    dates.length ? `Prices dated ${dates.join(", ")}.` : undefined,
    value?.stale
      ? "Cached price; refresh unavailable or source data is older."
      : undefined,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <div
      className={`card-price${value?.state === "range" ? " price-range" : ""}`}
      title={detail}
      aria-label={
        pending
          ? "Loading card price"
          : `${priceLabel(value?.lowPence, value?.highPence)}. ${detail}`
      }
    >
      <strong>
        {pending ? "…" : priceLabel(value?.lowPence, value?.highPence)}
      </strong>
      <span>
        {pending
          ? "Pricing"
          : value?.lowPence === undefined
            ? "No price"
            : value.stale
              ? "Cached guide"
              : value.state === "range"
                ? "Guide range"
                : "Price guide"}
      </span>
    </div>
  );
}

export function BatchPrice({
  value,
  loading,
}: {
  value?: BatchValuation;
  loading: boolean;
}) {
  const partial = value && value.pricedCount < value.totalCount;
  const dates = [
    ...new Set(
      value?.cards.flatMap((c) =>
        c.quotes.flatMap((q) => (q.updated ? [q.updated.slice(0, 10)] : [])),
      ) || [],
    ),
  ].sort();
  const sourceDate =
    dates.length > 1 ? `${dates[0]}–${dates.at(-1)}` : dates[0];
  return (
    <div className="batch-price" aria-label="Batch price guide">
      <span className="batch-price-label">
        {partial ? "Priced subtotal" : "Batch value"}
      </span>
      <strong>
        {loading && !value
          ? "…"
          : priceLabel(value?.lowPence, value?.highPence)}
      </strong>
      <span className="batch-price-coverage">
        {value
          ? `${value.pricedCount}/${value.totalCount} cards priced${value.refreshing ? " · updating" : value.stale ? " · cached" : ""}`
          : loading
            ? "Loading prices"
            : "Prices unavailable"}
      </span>
      <details className="price-source">
        <summary>Cardmarket · GBP</summary>
        <div>
          <p>
            Cardmarket trend via TCGdex{sourceDate ? ` · ${sourceDate}` : ""}.
          </p>
          {value?.exchangeRate && (
            <p>
              ECB · {value.exchangeRate.date} · €1 = £
              {value.exchangeRate.poundsPerEuro.toFixed(5)}
            </p>
          )}
          <p>
            {value?.warning ||
              "Market guide, not a condition-adjusted selling price."}
          </p>
          {partial && <p>Unpriced cards are excluded from this subtotal.</p>}
          {value?.cards.some((c) => c.state === "range") && (
            <p>Ranges include unresolved finishes or foil patterns.</p>
          )}
        </div>
      </details>
    </div>
  );
}
