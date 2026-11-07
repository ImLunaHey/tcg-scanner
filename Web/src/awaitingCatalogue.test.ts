import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { CardSummary } from "./CardSummary";
import { identityCard, type Identification } from "./Identification";
it("shows transcribed printing details without inventing a database identity", () => {
  const value: Identification = {
    request: { id: "pending", sequence: 2, language: "en" }, state: "awaitingCatalogue",
    sourceSHA256: "source", finish: "unverified", condition: "unassessed",
    evidence: { candidates: [], lines: [], warnings: [], uncataloguedCard: {
      name: "Kyogre", setCode: "DLR", setName: "Delta Reign", collectorNumber: "026/103",
    } },
  };
  expect(identityCard(value)).toBeUndefined();
  const html = renderToStaticMarkup(createElement(CardSummary, {
    label: "C-001", game: "pokemon", identification: value, pending: false,
    incomplete: false, openMarks: () => {},
  }));
  for (const text of ["Kyogre", "Delta Reign", "DLR", "026/103", "Awaiting catalogue data", "not yet verified"])
    expect(html).toContain(text);
  expect(html).not.toContain("possible matches");
});
