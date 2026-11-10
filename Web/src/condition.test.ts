import { describe, expect, it } from "vitest";
import {
  conditionSummary,
  conditionDetail,
  type ConditionCheck,
} from "./ConditionReview";
const check: ConditionCheck = {
  state: "checked",
  findings: [],
  limitations: [],
  method: "fixture",
};
describe("visible condition observations", () => {
  it("never interprets no marks as a grade", () =>
    expect(conditionSummary(check)).toBe(
      "No border marks flagged · not a grade",
    ));
  it("distinguishes an unavailable check from no marks", () => {
    expect(conditionSummary()).toBe("Visible marks: not checked");
    expect(conditionSummary({ ...check, state: "unsupported" })).toBe(
      "Visible marks: not assessed",
    );
    expect(conditionSummary({ ...check, state: "inconclusive" })).toBe(
      "Visible marks: analysis inconclusive",
    );
  });
  it("labels observations as candidates", () =>
    expect(
      conditionSummary({
        ...check,
        findings: [
          {
            kind: "edgeWhitening",
            region: { x: 0, y: 0, width: 0.1, height: 0.1 },
            explanation: "Inspect",
          },
        ],
      }),
    ).toBe("Visible marks: 1 candidate area"));
});

it("explains why an assessed or skipped image has no red boxes", () => {
  expect(conditionDetail(check)).toContain(
    "checked automatically; no candidate marks",
  );
  expect(
    conditionDetail({
      ...check,
      state: "unsupported",
      limitations: ["Front images are outside scope."],
    }),
  ).toContain("Front images are outside scope. No red boxes");
  expect(
    conditionDetail({
      ...check,
      state: "inconclusive",
      limitations: ["Image too small."],
    }),
  ).toContain("Image too small.");
});
