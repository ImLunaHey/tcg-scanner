import type { Batch } from "./models";
import type { LightboxImage } from "./ImageLightbox";

// Overlays must follow the exact analyzed source, even after a later crop/rotation.
export function conditionImages(
  batch: Batch,
  sides: { sequence: number; label: string }[],
): LightboxImage[] {
  return sides.flatMap((side) => {
    const identity = batch.cardIdentifications?.find(
      (i) => i.request.sequence === side.sequence,
    );
    if (!identity?.conditionCheck) return [];
    const source = identity.sourceEditID
      ? batch.processedImages?.find(
          (e) =>
            e.request.id === identity.sourceEditID &&
            e.request.sequence === side.sequence,
        )?.image
      : batch.images.find((i) => i.sequence === side.sequence);
    if (!source || source.sha256 !== identity.sourceSHA256) return [];
    return [
      {
        ...source,
        editID: identity.sourceEditID,
        label: `${side.label} · condition check`,
        conditionCheck: identity.conditionCheck,
      },
    ];
  });
}
