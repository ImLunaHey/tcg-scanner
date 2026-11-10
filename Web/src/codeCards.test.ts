import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { CardSummary } from "./CardSummary";
it("labels detected redemption cards without playable identity or finish controls", () => {
  const html = renderToStaticMarkup(createElement(CardSummary, {
    label: "C-016", game: "unknown", contentKind: "pokemonCodeCard", pending: false,
    incomplete: false, openMarks: () => {},
  }));
  expect(html).toContain("Pokémon online/code card");
  expect(html).toContain("not a playable card");
  expect(html).not.toContain("Unknown game");
  expect(html).not.toContain("Not identified yet");
  expect(html).not.toContain("Non-holo");
});
