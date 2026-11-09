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
    expect(gate.ready(card, 5000)).toBe(false);
    expect(gate.ready(card, 5500)).toBe(true);
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
  it("crops landscape and portrait video exactly like the 3:4 viewport", () => {
    expect(cameraViewport(1920, 1080)).toEqual({
      x: 555,
      y: 0,
      width: 810,
      height: 1080,
    });
    expect(cameraViewport(900, 1600)).toEqual({
      x: 0,
      y: 200,
      width: 900,
      height: 1200,
    });
  });
});
