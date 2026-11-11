import { expect, it } from "vitest";
import { cameraFinishLabel } from "./CameraFinish";
import type { FinishEstimate } from "./Identification";
const estimate: FinishEstimate = {
  state: "estimated",
  suggestedFinish: "holo",
  evidence: [],
  limitations: [],
  method: "fixture",
};
it("shows only supported positive finish estimates", () => {
  expect(cameraFinishLabel(estimate)).toBe("Holo");
  expect(
    cameraFinishLabel({ ...estimate, suggestedFinish: "reverseHolo" }),
  ).toBe("Reverse holo");
  expect(cameraFinishLabel({ ...estimate, suggestedFinish: "normal" })).toBe(
    "Non-holo",
  );
  expect(cameraFinishLabel({ ...estimate, state: "uncertain" })).toBe(
    "Finish unclear",
  );
  expect(cameraFinishLabel({ ...estimate, state: "unsupported" })).toBe(
    "Finish unavailable",
  );
});
