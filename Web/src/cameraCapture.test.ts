import { describe, expect, it } from "vitest";
import { cameraViewport, SteadyCard } from "./cameraCapture";
const card = Uint8Array.from({ length: 64 }, (_, i) => (i % 2 ? 220 : 20));
describe("camera capture", () => {
  it("starts on the first frame and throttles retries", () => {
    const gate = new SteadyCard();
    expect(gate.ready(card, 0)).toBe(true);
    gate.accept(card, 0);
    expect(gate.ready(card, 1100)).toBe(false);
    expect(gate.ready(card, 1200)).toBe(true);
  });
  it("does not require stillness, contrast or a previous frame", () => {
    for (const frames of [
      [card, card.map((x) => 255 - x)],
      [new Uint8Array(64).fill(100), new Uint8Array(64).fill(120)],
    ]) {
      const gate = new SteadyCard();
      const submissions: number[] = [];
      for (let now = 0; now <= 3600; now += 100) {
        const frame = frames[(now / 100) % 2];
        if (gate.ready(frame, now)) {
          submissions.push(now);
          gate.accept(frame, now);
        }
      }
      expect(submissions).toEqual([0, 1200, 2400, 3600]);
    }
  });
  it("suppresses an unchanged match and immediately tries a replacement when due", () => {
    const gate = new SteadyCard();
    gate.accept(card, 0);
    gate.matched = true;
    expect(gate.ready(card, 2000)).toBe(false);
    expect(gate.observation).toBe("unchanged");
    expect(gate.ready(card.map((x) => 255 - x), 2100)).toBe(true);
  });
  it("keeps the retry limit when a matched card changes", () => {
    const gate = new SteadyCard();
    gate.accept(card, 0);
    gate.matched = true;
    const next = card.map((x) => 255 - x);
    expect(gate.ready(next, 800)).toBe(false);
    expect(gate.ready(next, 1200)).toBe(true);
  });
  it("maps a centered region through landscape video cover scaling", () => {
    const area = cameraViewport(
      1920,
      1080,
      { width: 390, height: 844 },
      { x: 30, y: 202, width: 330, height: 440 },
    );
    expect(area.x).toBeCloseTo(748.8626, 3);
    expect(area.y).toBeCloseTo(258.4834, 3);
    expect(area.width).toBeCloseTo(422.2749, 3);
    expect(area.height).toBeCloseTo(563.0332, 3);
    expect(area.width / area.height).toBeCloseTo(0.75);
  });
  it("maps a landscape screen to portrait video without shifting the guide", () => {
    const area = cameraViewport(
      900,
      1600,
      { width: 1200, height: 800 },
      { x: 450, y: 200, width: 300, height: 400 },
    );
    expect(area.x).toBeCloseTo(337.5);
    expect(area.y).toBeCloseTo(650);
    expect(area.width).toBeCloseTo(225);
    expect(area.height).toBeCloseTo(300);
  });
});
