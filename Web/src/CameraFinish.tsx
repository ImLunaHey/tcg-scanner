import type { FinishEstimate } from "./Identification";

export function cameraFinishLabel(estimate?: FinishEstimate) {
  const labels: Record<string, string> = {
    normal: "Non-holo",
    holo: "Holo",
    reverseHolo: "Reverse holo",
  };
  if (
    estimate?.state === "estimated" &&
    estimate.suggestedFinish &&
    labels[estimate.suggestedFinish]
  )
    return labels[estimate.suggestedFinish];
  return estimate?.state === "unsupported" || !estimate
    ? "Finish unavailable"
    : "Finish unclear";
}
export function CameraFinish({
  estimate,
  compact = false,
}: {
  estimate?: FinishEstimate;
  compact?: boolean;
}) {
  const label = cameraFinishLabel(estimate);
  const pill = (
    <span
      className="camera-finish-pill"
      data-state={estimate?.state || "unsupported"}
      data-finish={estimate?.suggestedFinish}
    >
      {label}
    </span>
  );
  if (compact) return pill;
  return (
    <section className="camera-finish-details" aria-label="Finish check">
      <h2>Finish</h2>
      {pill}
      <p>
        {estimate?.state === "estimated"
          ? "Camera estimate from printed markers and catalogue evidence. Check the card under light to confirm."
          : estimate?.state === "unsupported" || !estimate
            ? "This card’s finish is outside the supported camera checks."
            : "The photo does not distinguish the finish reliably. Try a closer, sharply focused view without glare."}
      </p>
      {estimate && (
        <details>
          <summary>What the check found</summary>
          <ul>
            {estimate.evidence.map((line, i) => (
              <li key={i}>{line}</li>
            ))}
          </ul>
          <ul>
            {estimate.limitations.map((line, i) => (
              <li key={i}>{line}</li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
