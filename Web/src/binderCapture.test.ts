import { expect, it } from "vitest";
import { pocketGrid, cropBounds } from "./binderCapture";
it("covers every pocket once without overlapping neighbours", () => {
  const boxes = pocketGrid(3, 3, .05);
  expect(boxes).toHaveLength(9);
  expect(boxes[0].x).toBe(.05);
  expect(boxes[8].x + boxes[8].width).toBeCloseTo(.95);
  expect(boxes[3].y).toBeCloseTo(.35);
});
it("keeps border padding within the source photo", () => {
  expect(cropBounds({ x: 0, y: 0, width: 1, height: 1 }, 1000, 1500)).toEqual({ x: 0, y: 0, width: 1000, height: 1500 });
});
