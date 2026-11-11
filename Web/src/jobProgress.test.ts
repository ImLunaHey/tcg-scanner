import { expect, it } from "vitest";
import { batchProgressLabel } from "./jobProgress";
import type { Batch } from "./models";
it("describes image transfer independently of physical feeder completion", () => {
  const record = { state: "scanning", images: Array(17) } as Batch;
  expect(batchProgressLabel(record)).toContain("Receiving images · 17 saved");
  expect(batchProgressLabel(record)).toContain("after the feeder stops");
  expect(batchProgressLabel(record)).not.toContain("Total count is unknown");
});
it("shows the known received total and unique prepared images during processing", () => {
  const record = { state: "processing", images: Array(46), processedImages: [
    { request: { sequence: 1 } }, { request: { sequence: 1 } }, { request: { sequence: 2 } },
  ], cardGames: [{ number: 1, frontSequence: 2 }], cardIdentifications: [
    { request: { sequence: 2 }, state: "reviewRequired" },
  ] } as Batch;
  expect(batchProgressLabel(record)).toBe("Processing · 46 images received · 2/46 images prepared · 1 card checked");
});
it("counts classified code cards without waiting for a playable identity", () => {
  const record = { state: "processing", images: Array(6), cardGames: [
    { number: 1, contentKind: "pokemonCodeCard" },
    { number: 2, contentKind: "pokemonCodeCard" },
    { number: 3, game: "unknown" },
  ] } as Batch;
  expect(batchProgressLabel(record)).toContain("2 cards checked");
});
