import { describe, it, expect } from "vitest";
import {
  batchLabel,
  controls,
  uuid,
  groupCardCaptures,
  type OriginalImage,
  type Inspection,
} from "./models";
import { parseSSE } from "./events";

describe("shared contracts", () => {
  it("uses permanent labels without truncating large batch numbers", () => {
    expect(batchLabel({ id: "id", batchNumber: 1 })).toBe("B-001");
    expect(batchLabel({ id: "id", batchNumber: 1000 })).toBe("B-1000");
  });
  it("only offers discovered resolutions supported by the service engine", () => {
    const report = {
      units: [
        {
          type: 3,
          settings: {
            resolutionDPI: { allowed: [75, 300, 600, 1200, "400", 300.5] },
            supportsDuplexScanning: { current: false },
          },
        },
      ],
    } as unknown as Inspection;
    expect(controls(report)).toEqual({
      resolutions: [300, 600],
      duplex: false,
    });
    expect(controls()).toEqual({ resolutions: [], duplex: false });
  });
  it("creates UUID request IDs even without secure-context randomUUID", () => {
    const ids = new Set(Array.from({ length: 100 }, uuid));
    expect(ids.size).toBe(100);
    for (const id of ids)
      expect(id).toMatch(
        /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/,
      );
  });
  it("preserves event IDs and multiline payloads while ignoring heartbeat comments", () => {
    expect(parseSSE(": heartbeat")).toEqual({ event: "message", data: "" });
    expect(
      parseSSE("id: 42\nevent: scanner\ndata: first\ndata: second"),
    ).toEqual({ id: "42", event: "scanner", data: "first\nsecond" });
  });
});

it("keeps missing duplex sides in their sequence slots", () => {
  const image = (sequence: number) => ({ sequence }) as OriginalImage;
  const groups = groupCardCaptures([image(6), image(1), image(2), image(4)]);
  expect(
    groups.map((pair) => [
      pair.number,
      pair.first?.sequence,
      pair.second?.sequence,
    ]),
  ).toEqual([
    [1, 1, 2],
    [2, undefined, 4],
    [3, undefined, 6],
  ]);
});
