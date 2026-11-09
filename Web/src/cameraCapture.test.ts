import { describe, expect, it } from "vitest";
import { cameraViewport, SteadyCard } from "./cameraCapture";
const card = Uint8Array.from({ length: 64 }, (_, i) => (i % 2 ? 220 : 20));
describe("camera capture", () => {
  it("waits for steady textured frames and throttles retries", () => {
    const gate = new SteadyCard();
    expect(gate.ready(card, 0)).toBe(false);
    expect(gate.ready(card, 500)).toBe(false);
    expect(gate.ready(card, 1000)).toBe(true);
    gate.accept(card, 1000);
    expect(gate.ready(card, 3400)).toBe(false);
    expect(gate.ready(card, 3500)).toBe(true);
  });
  it("does not keep submitting a matched card, but detects a replacement", () => {
    const gate = new SteadyCard();
    gate.accept(card, 0);
    gate.matched = true;
    for (let i = 0; i < 10; i++)
      expect(gate.ready(card, 10000 + i * 500)).toBe(false);
    const next = card.map((x) => 255 - x);
    expect(gate.ready(next, 20000)).toBe(false);
    expect(gate.ready(next, 20500)).toBe(false);
    expect(gate.ready(next, 21000)).toBe(true);
  });
  it("accepts a changed matched card without waiting for the unclear-frame retry", () => {
    const gate = new SteadyCard();
    gate.accept(card, 0);
    gate.matched = true;
    const next = card.map((x) => 255 - x);
    expect(gate.ready(next, 800)).toBe(false);
    expect(gate.ready(next, 1000)).toBe(false);
    expect(gate.ready(next, 1200)).toBe(true);
  });
  it("ignores uniform backgrounds and movement", () => {
    const gate = new SteadyCard();
    for (let i = 0; i < 8; i++)
      expect(gate.ready(new Uint8Array(64).fill(100), i * 5000)).toBe(false);
    for (let i = 0; i < 8; i++)
      expect(
        gate.ready(
          card.map((x) => (i % 2 ? 255 - x : x)),
          50000 + i * 5000,
        ),
      ).toBe(false);
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
