import { describe, expect, it } from "vitest";
import { finishDescription, type Identification } from "./Identification";
const result: Identification = {
  request: { id: "fixture", sequence: 1, language: "en" },
  sourceSHA256: "fixture",
  state: "reviewRequired",
  finish: "unverified",
  condition: "unassessed",
  selectedCardID: "sv09-028",
  finishEstimate: {
    state: "estimated",
    candidateID: "sv09-028",
    suggestedFinish: "reverseHolo",
    evidence: [],
    limitations: [],
    method: "fixture",
  },
};
describe("finish estimates", () => {
  it("clearly labels an unconfirmed estimate", () =>
    expect(finishDescription(result)).toBe("Reverse holo (estimated)"));
  it("keeps a reviewed finish authoritative", () =>
    expect(finishDescription({ ...result, finish: "normal" })).toBe(
      "Non-holo (reviewed)",
    ));
  it("does not apply an estimate to another printing", () =>
    expect(finishDescription({ ...result, selectedCardID: "different" })).toBe(
      "Needs review",
    ));
  it("shows abstention without guessing", () =>
    expect(
      finishDescription({
        ...result,
        finishEstimate: {
          ...result.finishEstimate!,
          state: "uncertain",
          suggestedFinish: undefined,
        },
      }),
    ).toBe("Needs review"));
});
