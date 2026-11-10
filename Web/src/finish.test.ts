import { describe, expect, it } from "vitest";
import {
  finishDescription,
  finishPresentation,
  type Identification,
} from "./Identification";
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

it("explains shared-code ambiguity without assigning a finish or overriding review", () => {
  const ambiguous = {
    ...result,
    finishEstimate: {
      ...result.finishEstimate!,
      state: "uncertain" as const,
      suggestedFinish: undefined,
      uncertaintyReason: "multipleRegularFinishes",
    },
  };
  expect(finishDescription(ambiguous)).toBe("Non-holo or holo");
  expect(finishDescription({ ...ambiguous, finish: "normal" })).toBe(
    "Non-holo (reviewed)",
  );
  expect(finishDescription({ ...ambiguous, selectedCardID: "other" })).toBe(
    "Needs review",
  );
});

it("uses resolved styling only for a current result and distinguishes human review", () => {
  expect(finishPresentation(result)).toMatchObject({
    label: "Reverse holo",
    state: "resolved",
  });
  expect(finishPresentation({ ...result, finish: "normal" })).toMatchObject({
    label: "Non-holo",
    state: "reviewed",
  });
  expect(finishPresentation({ ...result, selectedCardID: "other" }).state).toBe(
    "uncertain",
  );
  expect(finishPresentation({ ...result, state: "rejected" }).state).toBe(
    "uncertain",
  );
});
it("keeps pending and unsupported checks separate from unresolved finishes", () => {
  expect(finishPresentation(undefined, true).state).toBe("pending");
  expect(finishPresentation({ ...result, state: "processing" }).state).toBe(
    "pending",
  );
  expect(finishPresentation(undefined).state).toBe("unavailable");
  expect(
    finishPresentation({
      ...result,
      finishEstimate: { ...result.finishEstimate!, state: "unsupported" },
    }).state,
  ).toBe("unavailable");
  expect(
    finishPresentation({
      ...result,
      finishEstimate: {
        ...result.finishEstimate!,
        state: "uncertain",
        suggestedFinish: undefined,
      },
    }),
  ).toMatchObject({ label: "Uncertain", state: "uncertain" });
});
