import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { CardSummary } from "./CardSummary";
import type { Identification } from "./Identification";

it("shows Pokemon catalogue rarity without treating it as a finish", () => {
  const identification: Identification = {
    request: { id: "request", sequence: 1, language: "en" },
    state: "reviewRequired", sourceSHA256: "source", finish: "unverified", condition: "unassessed",
    evidence: { lines: [], warnings: [], candidates: [{
      id: "test-1", name: "Example", localID: "001", officialSetCount: 100,
      setID: "test", setName: "Test", rarities: ["Rare"], evidence: [], availableFinishes: [],
    }] },
  };
  const html = renderToStaticMarkup(createElement(CardSummary, {
    label: "C-001", game: "pokemon", identification, pending: false, incomplete: false, openMarks: () => {},
  }));
  expect(html).toContain("Rare");
  expect(html).toContain("Catalogue rarity; this does not identify the foil finish");
  expect(html).not.toContain("Confirmed");
  expect(identification.finish).toBe("unverified");
});
