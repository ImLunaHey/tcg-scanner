import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { CardSummary } from "./CardSummary";
import {
  groupCardCaptures,
  presentCardPair,
  type OriginalImage,
} from "./models";
import type { Identification } from "./Identification";

const identity: Identification = {
  request: { id: "request", sequence: 2, language: "en" },
  state: "reviewRequired",
  sourceSHA256: "source",
  finish: "unverified",
  condition: "unassessed",
  evidence: {
    lines: [],
    warnings: [],
    candidates: [
      {
        id: "ygoprodeck:39706423:BLZD-EN091",
        provider: "ygoprodeck",
        catalogueID: "39706423",
        name: "GMX Suppression Squad",
        localID: "BLZD-EN091",
        setID: "BLZD-EN091",
        setName: "Blazing Dominion",
        rarities: ["Common"],
        evidence: [],
        availableFinishes: [],
      },
    ],
  },
};
it("shows the Yu-Gi-Oh front identity and printing without Pokemon finish or price claims", () => {
  const pair = groupCardCaptures(
    [1, 2].map((sequence) => ({ sequence }) as OriginalImage),
  )[0];
  const shown = presentCardPair(pair, [identity], {
    number: 1,
    game: "yugioh",
    frontSequence: 2,
    backSequence: 1,
    reason: "Back pattern",
  });
  expect(shown.identity).toBe(identity);
  expect(shown.sides[0].image?.sequence).toBe(2);
  const html = renderToStaticMarkup(
    createElement(CardSummary, {
      label: "C-001",
      game: "yugioh",
      identification: shown.identity,
      pending: false,
      incomplete: false,
      openMarks: () => {},
    }),
  );
  for (const value of [
    "Yu-Gi-Oh!",
    "GMX Suppression Squad",
    "BLZD-EN091",
    "Blazing Dominion",
    "Catalogue rarity",
    "Common",
    "Finish and condition not assessed",
  ])
    expect(html).toContain(value);
  expect(html).not.toContain("Non-holo");
  expect(html).not.toContain("Not identified yet");
});
it("keeps multiple listed rarities explicit", () => {
  const copy = structuredClone(identity);
  copy.evidence!.candidates[0].rarities = ["Common", "Ultra Rare"];
  const html = renderToStaticMarkup(
    createElement(CardSummary, {
      label: "C-001",
      game: "yugioh",
      identification: copy,
      pending: false,
      incomplete: false,
      openMarks: () => {},
    }),
  );
  expect(html).toContain("Common / Ultra Rare");
  expect(html).toContain("Catalogue rarities");
});

it("shows SWU names and keeps double-sided content numbered", () => {
  const swu = structuredClone(identity);
  swu.evidence!.candidates[0] = {
    ...swu.evidence!.candidates[0], id: "swu-db:ASH-025", provider: "swu-db",
    name: "Emperor's Observatory", localID: "025", setID: "ASH", setName: "Ashes of the Empire",
  };
  const pair = groupCardCaptures([1, 2].map((sequence) => ({ sequence }) as OriginalImage))[0];
  const shown = presentCardPair(pair, [swu], { number: 1, game: "starwarsUnlimited", reason: "Two-sided content" });
  expect(shown.identity).toBe(swu);
  expect(shown.sides.map((s) => s.label)).toEqual(["Side 1", "Side 2"]);
  const html = renderToStaticMarkup(createElement(CardSummary, {
    label: "C-001", game: "starwarsUnlimited", identification: shown.identity,
    pending: false, incomplete: false, openMarks: () => {},
  }));
  expect(html).toContain("Star Wars: Unlimited");
  expect(html).toContain("Ashes of the Empire");
  expect(html).toContain("Finish and condition not assessed");
});
