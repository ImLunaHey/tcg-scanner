import type { Identification } from "./Identification";
import { describe, it, expect } from "vitest";
import {
  batchLabel,
  controls,
  uuid,
  groupCardCaptures,
  presentCardPair,
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
      resolutions: [600],
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

const sideIdentity = (sequence: number, state: Identification["state"]) =>
  ({
    request: { sequence },
    state,
    evidence: {
      candidates:
        state === "reviewRequired" || state === "confirmed"
          ? [{ id: "card" }]
          : [],
    },
  }) as unknown as Identification;

it.each([1, 2])(
  "shows the recognized front in either capture direction (%i)",
  (front) => {
    const images = [1, 2].map((sequence) => ({ sequence }) as OriginalImage);
    const pair = groupCardCaptures(images)[0];
    const result = presentCardPair(
      pair,
      [
        sideIdentity(1, front === 1 ? "reviewRequired" : "noMatch"),
        sideIdentity(2, front === 2 ? "reviewRequired" : "noMatch"),
      ],
      {
        number: 1,
        game: "pokemon",
        frontSequence: front,
        backSequence: 3 - front,
        reason: "Detected back",
      },
    );
    expect(
      result.sides.map((side) => [side.label, side.image?.sequence]),
    ).toEqual([
      ["Front", front],
      ["Back", 3 - front],
    ]);
    expect(result.identity?.request.sequence).toBe(front);
    expect(images.map((image) => image.sequence)).toEqual([1, 2]);
  },
);

it("does not guess sides for ambiguous, unmatched, rejected or incomplete pairs", () => {
  const pair = groupCardCaptures(
    [1, 2].map((sequence) => ({ sequence }) as OriginalImage),
  )[0];
  for (const state of [
    "reviewRequired",
    "noMatch",
    "rejected",
    "processing",
  ] as const) {
    const result = presentCardPair(pair, [
      sideIdentity(1, state),
      sideIdentity(2, state),
    ]);
    expect(result.sides.map((side) => side.label)).toEqual([
      "Side 1",
      "Side 2",
    ]);
    expect(result.identity).toBeUndefined();
  }
  expect(
    presentCardPair({ number: 1, second: pair.second }, [
      sideIdentity(2, "reviewRequired"),
    ]).sides.map((side) => side.label),
  ).toEqual(["Side 1", "Side 2"]);
});

it("uses detected backs to order Yu-Gi-Oh pairs without Pokémon identity", () => {
  const images = [1, 2].map((sequence) => ({ sequence })) as OriginalImage[];
  const pair = groupCardCaptures(images)[0];
  const result = presentCardPair(pair, [], {
    number: 1,
    game: "yugioh",
    frontSequence: 2,
    backSequence: 1,
    reason: "Detected back",
  });
  expect(
    result.sides.map((side) => [side.image?.sequence, side.label]),
  ).toEqual([
    [2, "Front"],
    [1, "Back"],
  ]);
  expect(result.identity).toBeUndefined();
});
