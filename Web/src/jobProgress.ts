import type { Batch } from "./models";

export function batchProgressLabel(record: Batch): string {
  const received = record.images.length;
  if (record.state === "scanning") {
    return `Receiving images · ${received} saved. Image transfer can continue after the feeder stops.`;
  }
  if (record.state === "processing") {
    const prepared = new Set((record.processedImages ?? []).map((image) => image.request.sequence)).size;
    const checked = (record.cardGames ?? []).filter((card) =>
      card.frontSequence !== undefined && record.cardIdentifications?.some((identity) =>
        identity.request.sequence === card.frontSequence && identity.state !== "processing",
      ),
    ).length;
    return `Processing · ${received} images received · ${prepared}/${received} images prepared · ${checked} cards checked`;
  }
  return record.state === "preparing" ? "Preparing scanner…" : record.state === "queued" ? "Queued…" : `${record.state} · ${received} images saved`;
}
