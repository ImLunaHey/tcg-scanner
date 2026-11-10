import { describe, expect, it } from "vitest";
import { conditionImages } from "./conditionLightbox";
import type { Batch, OriginalImage } from "./models";
const original: OriginalImage = {
  sequence: 1,
  relativePath: "original.png",
  width: 700,
  height: 1000,
  bytes: 10,
  sha256: "original",
  side: "unknown",
  uniformType: "public.png",
};
const batch: Batch = {
  id: "fixture",
  state: "completed",
  createdAt: "",
  outputDirectory: "",
  errors: [],
  warnings: [],
  device: { id: "fixture", name: "fixture" },
  request: { dpi: 300, duplex: true, format: "png" },
  images: [original],
  processedImages: ["checked", "newer"].map((id) => ({
    request: {
      id,
      sequence: 1,
      crop: { x: 0, y: 0, width: 1, height: 1 },
      quarterTurns: 0,
    },
    image: { ...original, sha256: id },
    createdAt: "",
  })),
  cardIdentifications: [
    {
      request: { id: "identity", sequence: 1, language: "en" },
      state: "noMatch",
      finish: "unverified",
      condition: "unassessed",
      sourceEditID: "checked",
      sourceSHA256: "checked",
      conditionCheck: {
        state: "checked",
        method: "fixture",
        findings: [],
        limitations: [],
      },
    },
  ],
};
describe("condition lightbox provenance", () => {
  it("opens the analyzed source even when a newer crop exists", () => {
    const images = conditionImages(batch, [{ sequence: 1, label: "Back" }]);
    expect(images[0].editID).toBe("checked");
    expect(images[0].sha256).toBe("checked");
    expect(images[0].conditionCheck?.state).toBe("checked");
  });
  it("never overlays marks on changed or missing source data", () => {
    const changed = structuredClone(batch);
    changed.cardIdentifications![0].sourceSHA256 = "other";
    expect(conditionImages(changed, [{ sequence: 1, label: "Back" }])).toEqual(
      [],
    );
  });
  it("does not present an unchecked source as clear", () => {
    const unchecked = structuredClone(batch);
    delete unchecked.cardIdentifications![0].conditionCheck;
    expect(
      conditionImages(unchecked, [{ sequence: 1, label: "Back" }]),
    ).toEqual([]);
  });
});
