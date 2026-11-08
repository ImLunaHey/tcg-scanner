import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Router } from "wouter";
import { expect, it } from "vitest";
import { InventoryCard, type InventoryEntry } from "./Inventory";
const entry: InventoryEntry = {
  id: "pending", setKey: "pokemon:Delta Reign", game: "pokemon", language: "en", name: "Kyogre", setName: "Delta Reign", number: "026/103",
  status: "awaitingCatalogue", quantity: 2, confirmedQuantity: 0,
  copies: [{ batchID: "batch", batchNumber: 51, cardNumber: 1 }, { batchID: "other", batchNumber: 52, cardNumber: 3 }],
};
it("shows quantities, pending status and both physical locations", () => {
  const html = renderToStaticMarkup(createElement(Router, { ssrPath: "/inventory", children: createElement(InventoryCard, { entry }) }));
  for (const text of ["Kyogre", "026/103", "Awaiting catalogue data", "B-051 / C-001", "B-052 / C-003", "/batches/batch?card=1", "copies"])
    expect(html).toContain(text);
  expect(html).not.toContain("Confirmed");
});
it("does not label suggested catalogue matches as confirmed", () => {
  const html = renderToStaticMarkup(createElement(Router, { ssrPath: "/inventory", children: createElement(InventoryCard, { entry: { ...entry, status: "catalogue" } }) }));
  expect(html).toContain("Suggested match");
  expect(html).not.toContain("Awaiting catalogue data");
});

it("renders a visual set tile with a cover area and quantity", async () => {
  const { InventorySetTile } = await import("./Inventory");
  const html = renderToStaticMarkup(createElement(InventorySetTile, { set: {
    id: "pokemon:Delta Reign", name: "Delta Reign", game: "pokemon", quantity: 20, entries: 19, previews: [],
  }, select: () => {} }));
  expect(html).toContain("inventory-set-covers");
  expect(html).toContain("View Delta Reign");
  expect(html).toContain("20 copies");
});
