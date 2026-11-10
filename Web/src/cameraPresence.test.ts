import { describe, expect, it } from "vitest";
import { hasCameraCard } from "./cameraPresence";

describe("live card presence", () => {
  it("rejects empty scenes and the weak furniture box from the reported room", () => {
    expect(hasCameraCard([])).toBe(false);
    expect(hasCameraCard([{ x: .433, y: .004, width: .567, height: .244, confidence: .3988 }])).toBe(false);
  });
  it("accepts the Drowzee and low-confidence Salamence saved-card detections", () => {
    for (const confidence of [.883, .504])
      expect(hasCameraCard([{ x: 0, y: .03, width: .92, height: .8, confidence }])).toBe(true);
  });
  it("allows off-centre, rotated and partially clipped cards without tight alignment", () => {
    expect(hasCameraCard([{ x: .65, y: 0, width: .35, height: .3, confidence: .8 }])).toBe(true);
  });
  it("rejects small texture boxes and malformed geometry", () => {
    for (const width of [.01, -1, NaN, Infinity, 2])
      expect(hasCameraCard([{ x: 0, y: 0, width, height: .1, confidence: .9 }])).toBe(false);
  });
});
