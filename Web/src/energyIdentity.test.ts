import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { CardSummary } from "./CardSummary";
import { energyIdentityName, identityCard, type Identification } from "./Identification";
const energy: Identification = {
  request: { id: "test", sequence: 18, language: "en" }, state: "noMatch",
  sourceSHA256: "current", finish: "unverified", condition: "unassessed",
  evidence: { candidates: [], lines: [], warnings: [], energySymbol: {
    type: "Psychic", similarity: 0.8, method: "energy-corner-shape-v1",
  } },
};
it("shows symbol identity while leaving catalogue identity and price unresolved", () => {
  expect(energyIdentityName(energy)).toBe("Psychic Energy");
  expect(identityCard(energy)).toBeUndefined();
  const html = renderToStaticMarkup(createElement(CardSummary, {
    label: "C-009", game: "pokemon", identification: energy, pending: false,
    incomplete: false, openMarks: () => {},
  }));
  expect(html).toContain("<h2>Psychic Energy</h2>");
  expect(html).toContain("Exact printing unknown");
  expect(html).not.toContain("No card match found");
  expect(html).not.toContain("Non-holo");
});
it("does not reuse partial Energy identity after rejection or during a retry", () => {
  expect(energyIdentityName({ ...energy, state: "rejected" })).toBeUndefined();
  expect(energyIdentityName({ ...energy, state: "processing" })).toBeUndefined();
  expect(energyIdentityName({ ...energy, evidence: undefined })).toBeUndefined();
});
