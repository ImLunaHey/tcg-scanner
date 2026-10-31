import { describe, expect, it } from "vitest";
import { conditionSummary, type ConditionCheck } from "./ConditionReview";
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
      "Visible marks: needs manual inspection",
    );
    expect(conditionSummary({ ...check, state: "inconclusive" })).toBe(
      "Visible marks: needs manual inspection",
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
